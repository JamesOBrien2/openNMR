//! Bruker JCAMP-style parameter files (`acqus`, `acqu2s`, `procs`).
//!
//! Follows the parsing rules of nmrglue's `bruker.parse_jcamp_line`: strings in
//! `<...>` may span lines, and `(0..N)` arrays continue on following lines until
//! N + 1 values are read.

use std::collections::HashMap;

use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Scalar(String),
    Array(Vec<String>),
}

#[derive(Debug, Clone, Default)]
pub struct Parameters(HashMap<String, Value>);

impl Parameters {
    /// Parses the `##$KEY= value` entries of a parameter file, ignoring
    /// core headers (`##TITLE=`) and comments (`$$`).
    pub fn parse(text: &str) -> Self {
        let mut map = HashMap::new();
        let mut lines = text.lines().map(str::trim_end);
        while let Some(line) = lines.next() {
            if line.starts_with("##END=") {
                break;
            }
            let Some((key, text)) = line.strip_prefix("##$").and_then(|l| l.split_once('=')) else {
                continue;
            };
            let text = text.trim_start();
            let value = if text.starts_with('<') {
                Value::Scalar(parse_string(text, &mut lines))
            } else if let Some(len) = array_len(text) {
                Value::Array(parse_array(text, len, &mut lines))
            } else {
                Value::Scalar(text.to_string())
            };
            map.insert(key.to_string(), value);
        }
        Self(map)
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    /// Every entry as text, for showing a dataset's parameters as they stand.
    /// Arrays are joined, since a table wants one line for each key.
    pub fn entries(&self) -> Vec<(String, String)> {
        let mut rows: Vec<(String, String)> = self
            .0
            .iter()
            .map(|(key, value)| {
                let text = match value {
                    Value::Scalar(one) => one.clone(),
                    Value::Array(many) => many.join(" "),
                };
                (key.clone(), text)
            })
            .collect();
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        rows
    }

    pub fn str(&self, key: &str) -> Result<&str> {
        match self.get(key) {
            Some(Value::Scalar(s)) => Ok(s),
            Some(Value::Array(_)) => Err(invalid(key, "array")),
            None => Err(Error::MissingParameter(key.to_string())),
        }
    }

    pub fn f64(&self, key: &str) -> Result<f64> {
        let s = self.str(key)?;
        s.parse().map_err(|_| invalid(key, s))
    }

    pub fn i64(&self, key: &str) -> Result<i64> {
        let s = self.str(key)?;
        s.parse().map_err(|_| invalid(key, s))
    }

    /// Like [`Self::f64`], but a missing key gives `default`.
    pub fn f64_or(&self, key: &str, default: f64) -> Result<f64> {
        or_default(self.f64(key), default)
    }

    /// Like [`Self::i64`], but a missing key gives `default`.
    pub fn i64_or(&self, key: &str, default: i64) -> Result<i64> {
        or_default(self.i64(key), default)
    }
}

fn or_default<T>(value: Result<T>, default: T) -> Result<T> {
    match value {
        Err(Error::MissingParameter(_)) => Ok(default),
        other => other,
    }
}

fn invalid(key: &str, value: &str) -> Error {
    Error::InvalidParameter {
        key: key.to_string(),
        value: value.to_string(),
    }
}

fn parse_string<'a>(first: &str, lines: &mut impl Iterator<Item = &'a str>) -> String {
    let mut text = first.to_string();
    while !text[1..].contains('>') {
        let Some(next) = lines.next() else { break };
        text.push('\n');
        text.push_str(next);
    }
    let end = text.rfind('>').unwrap_or(text.len());
    text[1..end].to_string()
}

/// Returns N + 1 for a value starting with `(0..N)`.
fn array_len(text: &str) -> Option<usize> {
    let inner = text.strip_prefix('(')?.split_once(')')?.0;
    let (_, last) = inner.split_once("..")?;
    last.trim().parse::<usize>().ok().map(|n| n + 1)
}

fn parse_array<'a>(
    first: &str,
    len: usize,
    lines: &mut impl Iterator<Item = &'a str>,
) -> Vec<String> {
    let rest = first.split_once(')').map_or("", |(_, r)| r);
    let mut values: Vec<String> = rest.split_whitespace().map(unquote).collect();
    while values.len() < len {
        let Some(next) = lines.next() else { break };
        values.extend(next.split_whitespace().map(unquote));
    }
    values
}

fn unquote(token: &str) -> String {
    token
        .strip_prefix('<')
        .and_then(|t| t.strip_suffix('>'))
        .unwrap_or(token)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACQUS: &str = "##TITLE= Parameter file, Bruker 4.1.4
##JCAMPDX= 5.0
$$ /opt/data/1/acqus
##$TD= 65536
##$SW_h= 8012.82051282051
##$NUC1= <1H>
##$D= (0..63)
0 0.1 0.03 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
##$GRPDLY= 67.9841766357422
##$AUNM= <au_zg>
##$TITLE_LONG= <first line
second line>
##$BYTORDA= 0
##END=
##$AFTER= 1
";

    #[test]
    fn parses_scalars_and_strings() {
        let p = Parameters::parse(ACQUS);
        assert_eq!(p.i64("TD").unwrap(), 65536);
        assert_eq!(p.f64("SW_h").unwrap(), 8012.82051282051);
        assert_eq!(p.str("NUC1").unwrap(), "1H");
        assert_eq!(p.f64("GRPDLY").unwrap(), 67.9841766357422);
        assert_eq!(p.i64("BYTORDA").unwrap(), 0);
    }

    #[test]
    fn multi_line_array_does_not_swallow_next_key() {
        let p = Parameters::parse(ACQUS);
        let Some(Value::Array(d)) = p.get("D") else {
            panic!("D is not an array")
        };
        assert_eq!(d.len(), 64);
        assert_eq!(d[1], "0.1");
        assert!(p.get("GRPDLY").is_some());
    }

    #[test]
    fn multi_line_string() {
        let p = Parameters::parse(ACQUS);
        assert_eq!(p.str("TITLE_LONG").unwrap(), "first line\nsecond line");
    }

    #[test]
    fn stops_at_end_and_ignores_headers() {
        let p = Parameters::parse(ACQUS);
        assert!(p.get("AFTER").is_none());
        assert!(p.get("TITLE").is_none());
    }

    #[test]
    fn missing_and_invalid_values_are_errors() {
        let p = Parameters::parse(ACQUS);
        assert!(matches!(p.f64("SFO1"), Err(Error::MissingParameter(_))));
        assert!(matches!(p.f64("NUC1"), Err(Error::InvalidParameter { .. })));
        assert!(matches!(p.str("D"), Err(Error::InvalidParameter { .. })));
    }

    #[test]
    fn string_array_values_are_unquoted() {
        let p = Parameters::parse("##$NUC= (0..1)\n<1H> <13C>\n");
        assert_eq!(
            p.get("NUC"),
            Some(&Value::Array(vec!["1H".into(), "13C".into()]))
        );
    }
}
