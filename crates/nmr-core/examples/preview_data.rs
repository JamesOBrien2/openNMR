//! Writes Bruker datasets the way the browser preview of the app reads them, so
//! the interface can be looked at on real spectra without Tauri:
//!
//!     cargo run -p nmr-core --example preview_data -- testdata/bruker/L1C-Cl_00/1 ...
//!
//! Each dataset becomes `apps/desktop/public/preview-data/<n>.json`, with an
//! `index.json` naming them all. The files hold the spectrum as it comes out of
//! the Fourier transform (before phase, which the preview applies itself) and
//! the baseline that the default correction takes off, both from this crate's
//! own processing. The folder is not committed, as the datasets are not.
//!
//! A 2D experiment is written as processed with its recorded parameters: the
//! JSON describes it and `<n>.f32` holds the real part, row by row, as
//! little-endian `f32`, which is a tenth of the size of numbers written out.

use nmr_core::process::{self, Operation};
use nmr_core::{analysis, bruker, process2d};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn numbers(values: impl Iterator<Item = f64>) -> String {
    let mut out = String::from("[");
    for (i, v) in values.enumerate() {
        if i > 0 {
            out.push(',');
        }
        // Six significant figures is far below the noise and a third of the size.
        let _ = write!(out, "{v:.6e}");
    }
    out.push(']');
    out
}

fn text(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// "sample / expno", as the app names an experiment.
fn name(dir: &Path) -> String {
    let part = |p: Option<&Path>| {
        p.and_then(Path::file_name)
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    format!("{} / {}", part(dir.parent()), part(Some(dir)))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/desktop/public/preview-data");
    std::fs::create_dir_all(&out)?;
    let mut index = Vec::new();
    for (k, arg) in std::env::args().skip(1).enumerate() {
        let dir = PathBuf::from(&arg);
        if dir.join("acqu2s").is_file() {
            let raw = bruker::read_ser(&dir)?;
            let s = process2d::process(&raw, &process2d::default_2d(&raw))?;
            let real: Vec<f64> = s.rows.iter().flatten().map(|z| z.re).collect();
            let [f2, f1] = s.ppm_axes();
            let data = format!("{k}.f32");
            let bytes: Vec<u8> = real
                .iter()
                .flat_map(|&v| (v as f32).to_le_bytes())
                .collect();
            std::fs::write(out.join(&data), bytes)?;
            let json = format!(
                "{{\"kind\":\"2d\",\"name\":{},\"path\":{},\"nuclei\":[{},{}],\"solvent\":{},\"mhz\":{},\"mhz1\":{},\"points\":[{},{}],\"first\":[{},{}],\"delta\":[{},{}],\"noise\":{},\"max\":{},\"signed\":{},\"data\":{}}}",
                text(&name(&dir)),
                text(&arg),
                text(&s.f2.nucleus),
                text(&s.f1.nucleus),
                s.f2.solvent.as_deref().map_or("null".into(), text),
                s.f2.obs_mhz,
                s.f1.obs_mhz,
                s.rows[0].len(),
                s.rows.len(),
                f2.first,
                f1.first,
                f2.delta,
                f1.delta,
                analysis::noise_level(&real),
                real.iter().fold(0.0f64, |m, v| m.max(v.abs())),
                !raw.magnitude,
                text(&data),
            );
            let file = format!("{k}.json");
            std::fs::write(out.join(&file), json)?;
            println!("{} -> {file}", name(&dir));
            index.push(text(&file));
            continue;
        }
        let raw = bruker::read_fid(&dir)?;
        let ops = process::auto_1d(&raw, true, true)?;
        let ft = ops
            .iter()
            .position(|op| *op == Operation::Ft)
            .map_or(0, |i| i + 1);
        let base = process::process(&raw, &ops[..ft])?;
        let without: Vec<Operation> = ops
            .iter()
            .filter(|op| !matches!(op, Operation::BaselineWhittaker { .. }))
            .cloned()
            .collect();
        let bare = process::process(&base, &without[ft..])?;
        let done = process::process(&base, &ops[ft..])?;
        let axis = base.ppm_axis();
        let [p0, p1] = ops
            .iter()
            .find_map(|op| match op {
                Operation::Phase { p0, p1, .. } => Some([*p0, *p1]),
                _ => None,
            })
            .unwrap_or([0.0, 0.0]);
        let lb = ops.iter().find_map(|op| match op {
            Operation::Exponential { lb_hz } => Some(*lb_hz),
            _ => None,
        });
        let file = format!("{k}.json");
        let json = format!(
            "{{\"name\":{},\"path\":{},\"nucleus\":{},\"solvent\":{},\"mhz\":{},\"first\":{},\"delta\":{},\"phase\":[{p0},{p1}],\"lb\":{},\"size\":{},\"conjugate\":{},\"re\":{},\"im\":{},\"baseline\":{}}}",
            text(&name(&dir)),
            text(&arg),
            text(&raw.meta.nucleus),
            raw.meta.solvent.as_deref().map_or("null".into(), text),
            raw.meta.obs_mhz,
            axis.first,
            axis.delta,
            lb.map_or("null".into(), |v| v.to_string()),
            base.data.len(),
            raw.meta.conjugate,
            numbers(base.data.iter().map(|z| z.re)),
            numbers(base.data.iter().map(|z| z.im)),
            numbers(bare.data.iter().zip(&done.data).map(|(a, b)| a.re - b.re)),
        );
        std::fs::write(out.join(&file), json)?;
        println!("{} -> {}", name(&dir), file);
        index.push(text(&file));
    }
    std::fs::write(out.join("index.json"), format!("[{}]", index.join(",")))?;
    Ok(())
}
