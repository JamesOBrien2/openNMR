//! A molecule read from an MDL molfile (V2000): what is bonded to what, how many
//! hydrogens each atom carries, and which of them are equivalent by topology.
//! The drawing's coordinates are kept as they are, so a structure is shown as it
//! was drawn.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Atom {
    pub symbol: String,
    pub x: f64,
    pub y: f64,
    pub charge: i32,
    /// Hydrogens on this atom, drawn or implied.
    pub h: u32,
    /// Atoms of one class cannot be told apart by their bonds alone (a methyl's
    /// three hydrogens, the two ortho positions of a phenyl).
    pub class: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bond {
    pub a: usize,
    pub b: usize,
    /// 1, 2, 3, or 4 for aromatic.
    pub order: u8,
    /// The molfile's wedge code: 0 plain, 1 up, 6 down.
    pub stereo: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Molecule {
    pub atoms: Vec<Atom>,
    pub bonds: Vec<Bond>,
    /// Hill order, hydrogens counted.
    pub formula: String,
}

/// A fixed-width field, trimmed; a short line simply has less in it.
fn field(line: &str, from: usize, to: usize) -> &str {
    line.get(from.min(line.len())..to.min(line.len()))
        .unwrap_or("")
        .trim()
}

fn bad(what: impl Into<String>) -> Error {
    Error::Molfile(what.into())
}

fn number<T: std::str::FromStr>(line: &str, from: usize, to: usize, what: &str) -> Result<T> {
    field(line, from, to)
        .parse()
        .map_err(|_| bad(format!("{what} is not a number: {line:?}")))
}

/// Valences an element may take, smallest first, for its charge; hydrogens are
/// filled up to the first that is not below what the bonds already use. An
/// ion has the valence of the element that has its electron count.
fn valences(symbol: &str, charge: i32) -> &'static [u32] {
    match (symbol, charge) {
        ("C", 0) | ("N", 1) | ("B", -1) | ("Si", 0) => &[4],
        ("C", 1) | ("C", -1) | ("B", 0) => &[3],
        ("N", 0) | ("O", 1) => &[3],
        ("P", 0) => &[3, 5],
        ("N", -1) | ("O", 0) => &[2],
        ("O", -1) | ("F", 0) | ("Cl", 0) | ("Br", 0) | ("I", 0) => &[1],
        ("S", 0) => &[2, 4, 6],
        _ => &[],
    }
}

/// `text` is one molecule; anything after `M  END` (an SDF's properties) is
/// ignored.
pub fn parse_molfile(text: &str) -> Result<Molecule> {
    // Old Mac programs end their lines with a bare carriage return.
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = text.lines().collect();
    let counts = lines.get(3).ok_or_else(|| bad("the file is too short"))?;
    if counts.contains("V3000") {
        return Err(Error::Unsupported("molfile V3000; save it as V2000".into()));
    }
    // Fixed columns: on a large molecule the two counts run together (`100101`).
    let n_atoms: usize = number(counts, 0, 3, "the atom count")?;
    let n_bonds: usize = number(counts, 3, 6, "the bond count")?;
    if lines.len() < 4 + n_atoms + n_bonds {
        return Err(bad("the file ends before its atoms and bonds do"));
    }

    let mut symbols = Vec::new();
    let mut xy = Vec::new();
    let mut charges = Vec::new();
    for line in &lines[4..4 + n_atoms] {
        symbols.push(field(line, 31, 34).to_string());
        xy.push((
            number::<f64>(line, 0, 10, "x")?,
            number::<f64>(line, 10, 20, "y")?,
        ));
        charges.push(match field(line, 36, 39) {
            "1" => 3,
            "2" => 2,
            "3" => 1,
            "5" => -1,
            "6" => -2,
            "7" => -3,
            _ => 0,
        });
    }
    let mut raw_bonds = Vec::new();
    for line in &lines[4 + n_atoms..4 + n_atoms + n_bonds] {
        let a: usize = number(line, 0, 3, "a bond's first atom")?;
        let b: usize = number(line, 3, 6, "a bond's second atom")?;
        if a == 0 || b == 0 || a > n_atoms || b > n_atoms {
            return Err(bad(format!("a bond joins atom {a} to {b}")));
        }
        let order: u8 = number(line, 6, 9, "a bond's order")?;
        raw_bonds.push((a - 1, b - 1, order, field(line, 9, 12).parse().unwrap_or(0)));
    }

    // `M  CHG` replaces the charges of the atom block entirely, as the format says.
    let mut chg = false;
    for line in lines[4 + n_atoms + n_bonds..]
        .iter()
        .take_while(|l| !l.starts_with("M  END"))
    {
        if let Some(rest) = line.strip_prefix("M  CHG") {
            if !chg {
                charges.iter_mut().for_each(|c| *c = 0);
                chg = true;
            }
            let v: Vec<i32> = rest
                .split_whitespace()
                .filter_map(|t| t.parse().ok())
                .collect();
            for &pair in v.get(1..).unwrap_or(&[]).as_chunks::<2>().0 {
                if let Some(c) = usize::try_from(pair[0] - 1)
                    .ok()
                    .and_then(|i| charges.get_mut(i))
                {
                    *c = pair[1];
                }
            }
        }
    }

    // A hydrogen drawn as an atom belongs to the atom it hangs on: it is counted
    // there and left out, so the atoms shown are the heavy ones.
    let is_h: Vec<bool> = symbols.iter().map(|s| s == "H").collect();
    let mut keep = vec![usize::MAX; n_atoms];
    let mut next = 0;
    for i in 0..n_atoms {
        if !is_h[i] {
            keep[i] = next;
            next += 1;
        }
    }
    let mut drawn_h = vec![0u32; next];
    let mut bond_sum = vec![0u32; next];
    let mut aromatic = vec![false; next];
    let mut bonds = Vec::new();
    for &(a, b, order, stereo) in &raw_bonds {
        let (ka, kb) = (keep[a], keep[b]);
        for (k, other) in [(ka, kb), (kb, ka)] {
            if k == usize::MAX {
                continue;
            }
            if other == usize::MAX {
                drawn_h[k] += 1;
            }
            // Aromatic bonds count once each, and the ring's one double bond is
            // added below. ponytail: pyrrole's N-H reads as no H; a drawn one is kept.
            bond_sum[k] += if order == 4 { 1 } else { u32::from(order) };
            aromatic[k] |= order == 4;
        }
        if ka != usize::MAX && kb != usize::MAX {
            bonds.push(Bond {
                a: ka,
                b: kb,
                order,
                stereo,
            });
        }
    }

    let mut atoms = Vec::new();
    for i in (0..n_atoms).filter(|&i| !is_h[i]) {
        let k = keep[i];
        let used = bond_sum[k] + u32::from(aromatic[k]);
        let implied = valences(&symbols[i], charges[i])
            .iter()
            .find(|&&v| v >= used)
            .map_or(0, |v| v - used);
        atoms.push(Atom {
            symbol: symbols[i].clone(),
            x: xy[i].0,
            y: xy[i].1,
            charge: charges[i],
            h: implied,
            class: 0,
        });
    }
    // A drawn hydrogen is among the bonds, so the implied ones are what is left.
    for (atom, drawn) in atoms.iter_mut().zip(&drawn_h) {
        atom.h += drawn;
    }

    let mut molecule = Molecule {
        atoms,
        bonds,
        formula: String::new(),
    };
    molecule.classify();
    molecule.formula = molecule.hill();
    Ok(molecule)
}

impl Molecule {
    /// Morgan-style refinement: atoms start together by element, charge, degree
    /// and hydrogens, and are parted by the classes of their neighbours until no
    /// class splits any more. Topology only, so it cannot part diastereotopic
    /// hydrogens; that is for the person assigning to say. Bond orders enter as
    /// each atom's sum and not bond by bond: a Kekulé ring alternates them, and
    /// its two ortho carbons would otherwise never match.
    fn classify(&mut self) {
        let n = self.atoms.len();
        let mut near: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut order_sum = vec![0u32; n];
        for b in &self.bonds {
            near[b.a].push(b.b);
            near[b.b].push(b.a);
            order_sum[b.a] += u32::from(b.order);
            order_sum[b.b] += u32::from(b.order);
        }
        let rank = |keys: Vec<String>| -> Vec<usize> {
            let mut order: Vec<&String> = keys.iter().collect();
            order.sort();
            order.dedup();
            keys.iter()
                .map(|k| order.binary_search(&k).unwrap_or(0))
                .collect()
        };
        let mut classes = rank(
            self.atoms
                .iter()
                .zip(near.iter().zip(&order_sum))
                .map(|(a, (nb, o))| format!("{}|{}|{}|{}|{o}", a.symbol, a.charge, a.h, nb.len()))
                .collect(),
        );
        let count = |c: &[usize]| c.iter().max().map_or(0, |m| m + 1);
        loop {
            let next = rank(
                (0..n)
                    .map(|i| {
                        let mut around: Vec<usize> = near[i].iter().map(|&j| classes[j]).collect();
                        around.sort_unstable();
                        format!("{}|{around:?}", classes[i])
                    })
                    .collect(),
            );
            let done = count(&next) == count(&classes);
            classes = next;
            if done {
                break;
            }
        }
        for (atom, class) in self.atoms.iter_mut().zip(classes) {
            atom.class = class;
        }
    }

    /// Carbon first, then hydrogen, then the rest alphabetically (Hill).
    fn hill(&self) -> String {
        let mut count: BTreeMap<&str, u32> = BTreeMap::new();
        for a in &self.atoms {
            *count.entry(&a.symbol).or_default() += 1;
        }
        let h: u32 = self.atoms.iter().map(|a| a.h).sum();
        if h > 0 {
            count.insert("H", h);
        }
        let mut parts: Vec<(&str, u32)> = Vec::new();
        let has_c = count.contains_key("C");
        for first in if has_c { vec!["C", "H"] } else { vec![] } {
            if let Some(n) = count.remove(first) {
                parts.push((first, n));
            }
        }
        parts.extend(count);
        parts
            .iter()
            .map(|(s, n)| {
                if *n == 1 {
                    s.to_string()
                } else {
                    format!("{s}{n}")
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A molfile from atoms `(symbol, charge code)` and bonds `(a, b, order)`,
    /// laid out on a line: only the connectivity is under test.
    fn molfile(atoms: &[(&str, u8)], bonds: &[(usize, usize, u8)]) -> String {
        let mut s = format!(
            "\n  test\n\n{:>3}{:>3}  0  0  0  0  0  0  0  0999 V2000\n",
            atoms.len(),
            bonds.len()
        );
        for (i, (sym, chg)) in atoms.iter().enumerate() {
            s += &format!(
                "{:>10.4}{:>10.4}{:>10.4} {:<3} 0{:>3}  0  0  0  0  0  0  0  0  0  0\n",
                i as f64, 0.0, 0.0, sym, chg
            );
        }
        for (a, b, o) in bonds {
            s += &format!("{a:>3}{b:>3}{o:>3}  0\n");
        }
        s + "M  END\n"
    }

    /// The hydrogens of each class, largest first, as `(atoms, hydrogens)`.
    fn groups(m: &Molecule) -> Vec<(usize, u32)> {
        let mut by: BTreeMap<usize, (usize, u32)> = BTreeMap::new();
        for a in m.atoms.iter().filter(|a| a.h > 0) {
            let e = by.entry(a.class).or_default();
            e.0 += 1;
            e.1 += a.h;
        }
        let mut v: Vec<_> = by.into_values().collect();
        v.sort_by_key(|a| std::cmp::Reverse(a.1));
        v
    }

    #[test]
    fn ethanol_has_three_groups() {
        let m = parse_molfile(&molfile(
            &[("C", 0), ("C", 0), ("O", 0)],
            &[(1, 2, 1), (2, 3, 1)],
        ))
        .unwrap();
        assert_eq!(m.formula, "C2H6O");
        assert_eq!(groups(&m), vec![(1, 3), (1, 2), (1, 1)]);
    }

    #[test]
    fn kekule_benzene_is_one_group_of_six() {
        let bonds: Vec<_> = (1..=6)
            .map(|i| (i, i % 6 + 1, if i % 2 == 1 { 2 } else { 1 }))
            .collect();
        let m = parse_molfile(&molfile(&[("C", 0); 6], &bonds)).unwrap();
        assert_eq!(m.formula, "C6H6");
        assert_eq!(groups(&m), vec![(6, 6)]);
    }

    #[test]
    fn toluene_parts_ortho_meta_para() {
        let mut bonds: Vec<_> = (1..=6)
            .map(|i| (i, i % 6 + 1, if i % 2 == 1 { 2 } else { 1 }))
            .collect();
        bonds.push((1, 7, 1));
        let m = parse_molfile(&molfile(&[("C", 0); 7], &bonds)).unwrap();
        assert_eq!(m.formula, "C7H8");
        // methyl, ortho pair, meta pair, para
        assert_eq!(groups(&m), vec![(1, 3), (2, 2), (2, 2), (1, 1)]);
    }

    #[test]
    fn tert_butyl_is_one_group_of_nine() {
        let m = parse_molfile(&molfile(
            &[("C", 0), ("C", 0), ("C", 0), ("C", 0), ("O", 0)],
            &[(1, 2, 1), (1, 3, 1), (1, 4, 1), (1, 5, 1)],
        ))
        .unwrap();
        assert_eq!(groups(&m), vec![(3, 9), (1, 1)]);
    }

    #[test]
    fn nitro_nitrogen_carries_no_hydrogen() {
        // CH3-N(+)(=O)O(-), the charges in the atom block.
        let m = parse_molfile(&molfile(
            &[("C", 0), ("N", 3), ("O", 0), ("O", 5)],
            &[(1, 2, 1), (2, 3, 2), (2, 4, 1)],
        ))
        .unwrap();
        assert_eq!(m.formula, "CH3NO2");
    }

    #[test]
    fn a_charge_line_replaces_the_atom_block() {
        let mut text = molfile(
            &[("C", 0), ("N", 0), ("O", 0), ("O", 0)],
            &[(1, 2, 1), (2, 3, 2), (2, 4, 1)],
        );
        text = text.replace("M  END", "M  CHG  2   2   1   4  -1\nM  END");
        let m = parse_molfile(&text).unwrap();
        assert_eq!((m.atoms[1].charge, m.atoms[3].charge), (1, -1));
        assert_eq!(m.formula, "CH3NO2");
    }

    #[test]
    fn counts_that_run_together_are_read_by_column() {
        let mut atoms = vec![("C", 0)];
        atoms.resize(100, ("C", 0));
        let bonds: Vec<_> = (1..100).map(|i| (i, i + 1, 1)).collect();
        let m = parse_molfile(&molfile(&atoms, &bonds)).unwrap();
        assert_eq!((m.atoms.len(), m.bonds.len()), (100, 99));
    }

    #[test]
    fn a_drawn_hydrogen_is_counted_on_its_atom() {
        // Methane with one H drawn: still four.
        let m = parse_molfile(&molfile(&[("C", 0), ("H", 0)], &[(1, 2, 1)])).unwrap();
        assert_eq!((m.atoms.len(), m.atoms[0].h), (1, 4));
    }

    #[test]
    fn carriage_returns_end_lines_too() {
        let text = molfile(&[("C", 0), ("O", 0)], &[(1, 2, 1)]).replace('\n', "\r");
        assert_eq!(parse_molfile(&text).unwrap().formula, "CH4O");
    }

    #[test]
    fn v3000_is_refused_and_junk_is_an_error() {
        assert!(parse_molfile("\n\n\n  0  0  0     0  0            999 V3000\n").is_err());
        assert!(parse_molfile("not a molfile").is_err());
    }
}
