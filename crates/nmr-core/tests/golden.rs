//! Compares nmr-core with reference output from `tools/reference/golden_1d.py`
//! and with stored processed spectra. Datasets are not committed
//! (`tools/reference/fetch_testdata.sh` downloads public ones), so this checks
//! whatever is present locally.

use std::fs;
use std::path::{Path, PathBuf};

use nmr_core::process::{self, Operation};
use nmr_core::{Complex64, Error, Spectrum2D, analysis, baseline, bruker, phase, process2d};

/// Reference file, operation appended to `default_1d`, tolerance relative to the
/// largest intensity. airPLS reweights every point by `exp(|d|)` and drops points
/// that land above the baseline, so the round-off of each linear solve grows over
/// its iterations: on a real spectrum, where the loop runs all 20 iterations
/// without converging, perturbing the input by 1e-12 already moves the baseline
/// by 1e-5 of the largest intensity (2e-8 on the synthetic dataset, which
/// converges in 6). `whittaker_solver_matches_scipy` checks the solve itself
/// tightly instead.
const CASES: &[(&str, Option<Operation>, f64)] = &[
    ("default_1d.c128", None, 1e-9),
    (
        "airpls.c128",
        Some(Operation::BaselineAirpls { lambda: 1e7 }),
        5e-3,
    ),
    (
        "polynomial.c128",
        Some(Operation::BaselinePolynomial { order: 5 }),
        1e-6,
    ),
];

#[test]
fn processing_matches_reference() {
    let testdata = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let golden = testdata.join("golden");
    let files = find(&golden, "default_1d.c128");
    for file in &files {
        let dir = file.parent().unwrap();
        let rel = dir.strip_prefix(&golden).unwrap();
        let raw = bruker::read_fid(&testdata.join("bruker").join(rel)).unwrap();
        let default = process::default_1d(&raw);

        for (name, extra, tolerance) in CASES {
            let mut ops = default.clone();
            ops.extend(extra.iter().cloned());
            let s = process::process(&raw, &ops).unwrap();
            let expected = read_complex(&dir.join(name));
            assert_eq!(s.data.len(), expected.len(), "{}/{name}", rel.display());
            let scale = expected.iter().map(|z| z.norm()).fold(0.0, f64::max);
            let worst = s
                .data
                .iter()
                .zip(&expected)
                .map(|(a, b)| (a - b).norm())
                .fold(0.0, f64::max);
            assert!(
                worst <= tolerance * scale,
                "{}/{name}: max deviation {worst:e}, max intensity {scale:e}",
                rel.display()
            );
        }

        let axis = read_f64(&dir.join("axis.f64"));
        let ours = process::process(&raw, &default).unwrap().ppm_axis();
        assert!(
            (ours.first - axis[0]).abs() < 1e-9,
            "{}: first ppm",
            rel.display()
        );
        assert!(
            (ours.delta - axis[1]).abs() < 1e-9,
            "{}: ppm step",
            rel.display()
        );
    }
    eprintln!("compared {} datasets with reference output", files.len());
}

/// The airPLS tolerance above is loose by necessity, so the linear algebra it
/// rests on is compared with scipy's sparse factorisation separately.
#[test]
fn whittaker_solver_matches_scipy() {
    let testdata = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let golden = testdata.join("golden");
    for file in find(&golden, "whittaker.f64") {
        let rel = file.parent().unwrap().strip_prefix(&golden).unwrap();
        let raw = bruker::read_fid(&testdata.join("bruker").join(rel)).unwrap();
        let s = process::process(&raw, &process::default_1d(&raw)).unwrap();
        let real: Vec<f64> = s.data.iter().map(|z| z.re).collect();
        let ours = baseline::smooth(&real, 1e7);
        let expected = read_f64(&file);
        assert_eq!(ours.len(), expected.len(), "{}", rel.display());
        let scale = real.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let worst = ours
            .iter()
            .zip(&expected)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        // The smoothing matrix is ill-conditioned at this lambda (1.8e10 for
        // 65 536 points), so the banded LDL' and scipy's sparse LU differ by
        // ~1.6e-7 of the largest intensity.
        assert!(
            worst <= 1e-6 * scale,
            "{}: max deviation {worst:e}, max intensity {scale:e}",
            rel.display()
        );
    }
}

#[test]
fn processed_data_matches_nmrglue() {
    let testdata = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let golden = testdata.join("golden");
    let files = find(&golden, "processed.c128");
    for file in &files {
        let dir = file.parent().unwrap();
        let rel = dir.strip_prefix(&golden).unwrap();
        let s = bruker::read_processed(&testdata.join("bruker").join(rel)).unwrap();
        let expected = read_complex(file);
        assert_eq!(s.data, expected, "{}", rel.display());
        let axis = read_f64(&dir.join("processed_axis.f64"));
        let ours = s.ppm_axis();
        assert!(
            (ours.first - axis[0]).abs() < 1e-9,
            "{}: first ppm",
            rel.display()
        );
        assert!(
            (ours.delta - axis[1]).abs() < 1e-9,
            "{}: ppm step",
            rel.display()
        );
    }
    eprintln!("compared {} processed spectra with nmrglue", files.len());
}

/// Auto-phase must undo a known extra phase error on top of whatever phase the
/// dataset already has, so this works for real data of unknown phase.
#[test]
fn auto_phase_undoes_applied_error() {
    let testdata = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let (e0, e1) = (50.0, -80.0);
    for file in find(&testdata.join("golden"), "default_1d.c128") {
        let rel = file
            .parent()
            .unwrap()
            .strip_prefix(testdata.join("golden"))
            .unwrap();
        let raw = bruker::read_fid(&testdata.join("bruker").join(rel)).unwrap();
        let spectrum = process::process(&raw, &process::default_1d(&raw)).unwrap();
        let (a0, a1) = phase::auto_phase(&spectrum.data, &raw.meta);
        let error = Operation::Phase {
            p0: e0,
            p1: e1,
            pivot: 0.0,
        };
        let dephased = process::process(&spectrum, &[error]).unwrap();
        let (b0, b1) = phase::auto_phase(&dephased.data, &raw.meta);
        let d0 = (b0 - (a0 - e0) + 180.0).rem_euclid(360.0) - 180.0;
        assert!(d0.abs() < 2.0, "{}: p0 {b0} vs {}", rel.display(), a0 - e0);
        assert!(
            (b1 - (a1 - e1)).abs() < 3.0,
            "{}: p1 {b1} vs {}",
            rel.display(),
            a1 - e1
        );
    }
}

#[test]
fn analysis_matches_nmrglue() {
    let testdata = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let golden = testdata.join("golden");
    for file in find(&golden, "integrals.f64") {
        let dir = file.parent().unwrap();
        let rel = dir.strip_prefix(&golden).unwrap();
        let raw = bruker::read_fid(&testdata.join("bruker").join(rel)).unwrap();
        let s = process::process(&raw, &process::default_1d(&raw)).unwrap();
        let real: Vec<f64> = s.data.iter().map(|z| z.re).collect();
        let axis = s.ppm_axis();

        let limits = [(1.35, 1.10), (3.85, 3.55), (2.70, 2.50)];
        for ((a, b), expected) in limits.iter().zip(read_f64(&file)) {
            let value = analysis::integrate(&real, &axis, *a, *b);
            assert!(
                (value - expected).abs() <= 1e-9 * expected.abs(),
                "{}: integral {a}..{b} = {value}, nmrglue {expected}",
                rel.display()
            );
        }

        let threshold = 0.05 * real.iter().copied().fold(f64::MIN, f64::max);
        let peaks: Vec<f64> = analysis::local_maxima(&real, 0..real.len(), threshold, 3)
            .into_iter()
            .map(|i| i as f64)
            .collect();
        assert_eq!(
            peaks,
            read_f64(&dir.join("peaks.f64")),
            "{}: peaks",
            rel.display()
        );
    }
}

/// Every FID in `testdata/bruker` that has a stored processed spectrum, with
/// that spectrum: (path, FID, `pdata/1`).
fn processed_pairs() -> Vec<(String, nmr_core::Spectrum, nmr_core::Spectrum)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/bruker");
    let mut pairs = Vec::new();
    for acqus in find(&root, "acqus") {
        let exp = acqus.parent().unwrap();
        let pdata = exp.join("pdata/1");
        if exp.join("fid").is_file() && pdata.join("1r").is_file() {
            let rel = exp.strip_prefix(&root).unwrap().display().to_string();
            let raw = bruker::read_fid(exp).unwrap();
            pairs.push((rel, raw, bruker::read_processed(&pdata).unwrap()));
        }
    }
    // Stored apart from its FID. Its delay, 61.02 points, is the one in
    // testdata that turns the recorded phase half a turn.
    let (fid, stored) = (
        root.join("cheminfo/aspirin-1h/1"),
        root.join("cheminfo/aspirin-1h-processed/1/pdata/1"),
    );
    if fid.join("fid").is_file() && stored.join("1r").is_file() {
        let raw = bruker::read_fid(&fid).unwrap();
        let stored = bruker::read_processed(&stored).unwrap();
        pairs.push(("cheminfo/aspirin-1h/1".into(), raw, stored));
    }
    pairs
}

/// Opened as the app opens it (recorded window, zero filling and phase), every
/// FID must match the stored `1r`. This catches a reader convention
/// (quadrature sign, digital filter, zero filling) that holds on one
/// spectrometer and not another: a wrong one scores below 0.1. The rest of the
/// gap is the stored spectrum's own treatment of the group delay and of its
/// baseline; an older ¹³C dataset is lowest at 0.98, and 0.9995 or more with
/// the phase fitted freely.
#[test]
fn matches_processed() {
    const MIN_CORRELATION: f64 = 0.97;
    let mut failures = Vec::new();
    let pairs = processed_pairs();
    for (rel, raw, stored) in &pairs {
        let ops = process::auto_1d(raw, false, false).unwrap();
        let ours = process::process(raw, &ops).unwrap();
        if ours.data.len() != stored.data.len() {
            failures.push(format!(
                "{rel}: {} points vs {} stored",
                ours.data.len(),
                stored.data.len()
            ));
            continue;
        }
        let c = correlation(&ours.data, &stored.data);
        if c < MIN_CORRELATION {
            failures.push(format!("{rel}: correlation {c:.4} with the stored 1r"));
        }
    }
    eprintln!("compared {} datasets with stored spectra", pairs.len());
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// Auto phase, run without the recorded phase, must land within 7° of it
/// wherever there is signal on ¹H, 16° on other nuclei. With the digital
/// filter taken off as a first-order phase, every ¹H is within 6.4° (most
/// within 2.3°) and the ⁸⁷Rb 1.3°; an older ¹³C dataset is 15.1° out, its
/// regions scattering by ±80° and its spectrum 1.5 % from the stored one
/// whichever way the filter comes off.
#[test]
fn auto_phase_matches_recorded() {
    let mut failures = Vec::new();
    for (rel, raw, _) in processed_pairs() {
        let Some([r0, r1]) = raw.meta.phase else {
            continue;
        };
        let ops = process::auto_1d(&raw, false, false).unwrap();
        let unphased: Vec<_> = ops
            .into_iter()
            .filter(|op| !matches!(op, Operation::Phase { .. }))
            .collect();
        let spectrum = process::process(&raw, &unphased).unwrap();
        let (a0, a1) = phase::auto_phase(&spectrum.data, &raw.meta);
        let top = spectrum.data.iter().map(|z| z.norm()).fold(0.0, f64::max);
        let n = spectrum.data.len() as f64;
        let worst = spectrum
            .data
            .iter()
            .enumerate()
            .filter(|(_, z)| z.norm() > 0.1 * top)
            .map(|(k, _)| {
                let d = (a0 - r0) + (a1 - r1) * k as f64 / n;
                ((d + 180.0).rem_euclid(360.0) - 180.0).abs()
            })
            .fold(0.0, f64::max);
        let tolerance = if raw.meta.nucleus == "1H" { 7.0 } else { 16.0 };
        if worst > tolerance {
            failures.push(format!(
                "{rel}: auto phase {a0:.1}/{a1:.1} is {worst:.1}° from recorded {r0:.1}/{r1:.1}"
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// Every uniformly sampled 2D in `testdata/bruker` that has a stored `2rr` must
/// match it when opened with the recorded parameters: same size, same axes, a
/// correlation of at least 0.97, and the ten tallest stored peaks where ours
/// are in F1, to 0.6 of a point on average.
///
/// The stored spectra also carried F1 on by linear prediction, which is ported, and took
/// a polynomial baseline off in both dimensions (`abs1`, `abs2`) and, on the
/// HSQC, phased automatically (`apkm`), which are not: the COSY reaches 0.998,
/// the HMBC 0.979 and the HSQC 0.992, against 0.88 for all three before the
/// prediction. A wrong quadrature convention scores below 0.06, and on the HSQC
/// a phase half a turn out scores −0.87. But F1 mirrored instead of
/// conjugated, one point off, scored 0.87 on the COSY before the prediction,
/// so the peaks are placed to a fraction of a point too: 0.03, 0.51 and 0.03
/// points on the COSY, HMBC and HSQC, against 1.0 to 1.3 mirrored. F2's
/// orientation is the 1D's, tested above.
#[test]
fn matches_processed_2d() {
    compare_2d(false, 0.97, 0.6);
}

/// The same for the 2Ds sampled non-uniformly, filled in by IST before F1 is
/// transformed. The stored spectra were reconstructed with IST as well, but its
/// implementation is not published, and it mirrored each FID first (virtual echo), which is left
/// out. The ibuprofen COSY reaches 0.938, its HMBC 0.970 and its HSQC 0.945,
/// the COSY and HSQC of `nonUniformSampling` 0.976 and 0.983. The ibuprofen
/// HSQC is sensitivity-improved (`hsqcedetgpsisp2.3`): its match is the second
/// pulse program the echo-antiecho pairing holds on.
#[test]
fn matches_processed_nus() {
    compare_2d(true, 0.93, 0.6);
}

/// Compares every 2D with the stored `2rr`, uniformly sampled or not (`nus`).
fn compare_2d(nus: bool, min_correlation: f64, max_f1_offset: f64) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/bruker");
    let mut failures = Vec::new();
    let mut compared = 0;
    for acqu2s in find(&root, "acqu2s") {
        let exp = acqu2s.parent().unwrap();
        let pdata = exp.join("pdata/1");
        if !exp.join("ser").is_file() || !pdata.join("2rr").is_file() {
            continue;
        }
        let rel = exp.strip_prefix(&root).unwrap().display().to_string();
        let raw = match bruker::read_ser(exp) {
            Err(Error::Unsupported(_)) => continue,
            other => other.unwrap(),
        };
        if raw.nus.is_some() != nus {
            continue;
        }
        let stored = bruker::read_processed_2d(&pdata).unwrap();
        let ours = process2d::process(&raw, &process2d::default_2d(&raw)).unwrap();
        compared += 1;
        let size = |s: &Spectrum2D| (s.rows.len(), s.rows[0].len());
        if size(&ours) != size(&stored) {
            failures.push(format!(
                "{rel}: {:?} points vs {:?} stored",
                size(&ours),
                size(&stored)
            ));
            continue;
        }
        for (dim, (a, b)) in ["F2", "F1"]
            .iter()
            .zip(ours.ppm_axes().iter().zip(stored.ppm_axes()))
        {
            // The step to a millionth: some acqus files round SW_h (8012.82 where
            // the stored axis is 8012.8205), which is 6e-8 of it.
            if (a.first - b.first).abs() > 0.5 * b.delta.abs()
                || (a.delta - b.delta).abs() > 1e-6 * b.delta.abs()
            {
                failures.push(format!("{rel}: {dim} axis {a:?} vs {b:?}"));
            }
        }
        let flat = |s: &Spectrum2D| s.rows.iter().flatten().copied().collect::<Vec<_>>();
        let c = correlation(&flat(&ours), &flat(&stored));
        // The COSY's sine bell tells where the window starts: at the first
        // point of the FID it scores 0.989, at the end of the filter's delay
        // 0.998, as the stored spectrum starts it.
        let min_correlation = if rel.contains("cyclosporin_cosy") {
            0.995
        } else {
            min_correlation
        };
        if c < min_correlation {
            failures.push(format!("{rel}: correlation {c:.4} with the stored 2rr"));
        }
        let peaks = tallest_peaks(&stored, 10);
        let offset = peaks
            .iter()
            .map(|&(r, k)| (f1_position(&ours, r, k) - f1_position(&stored, r, k)).abs())
            .sum::<f64>()
            / peaks.len() as f64;
        eprintln!("{rel}: correlation {c:.4}, F1 peaks {offset:.2} points off");
        if offset > max_f1_offset {
            failures.push(format!(
                "{rel}: the stored tallest peaks are {offset:.2} points from ours in F1"
            ));
        }
    }
    eprintln!("compared {compared} 2D datasets with stored spectra");
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// The `n` largest local maxima of the absolute real part, away from the edges.
fn tallest_peaks(s: &Spectrum2D, n: usize) -> Vec<(usize, usize)> {
    let height = |r: usize, k: usize| s.rows[r][k].re.abs();
    let (rows, cols) = (s.rows.len(), s.rows[0].len());
    let mut peaks: Vec<(usize, usize)> = (4..rows - 4)
        .flat_map(|r| (4..cols - 4).map(move |k| (r, k)))
        .filter(|&(r, k)| {
            (r - 1..=r + 1).all(|i| (k - 1..=k + 1).all(|j| height(i, j) <= height(r, k)))
        })
        .collect();
    peaks.sort_by(|a, b| height(b.0, b.1).total_cmp(&height(a.0, a.1)));
    peaks.truncate(n);
    peaks
}

/// F1 position, to a fraction of a point, of the tallest point within two rows
/// and a column of `(r, k)`: the vertex of a parabola through it and the rows
/// either side.
fn f1_position(s: &Spectrum2D, r: usize, k: usize) -> f64 {
    let height = |r: usize, k: usize| s.rows[r][k].re.abs();
    let (r, k) = (r - 2..=r + 2)
        .flat_map(|i| (k - 1..=k + 1).map(move |j| (i, j)))
        .max_by(|a, b| height(a.0, a.1).total_cmp(&height(b.0, b.1)))
        .unwrap_or((r, k));
    let (a, b, c) = (height(r - 1, k), height(r, k), height(r + 1, k));
    r as f64 + 0.5 * (a - c) / (a - 2.0 * b + c)
}

/// An acquisition stopped early is read as far as it got: the recorded `TD` says
/// 101 increments, the file holds 256 rows, of which only 100 were recorded
/// (`TDeff`), and echo-antiecho rows come in pairs.
#[test]
fn incomplete_2d_reads_what_was_recorded() {
    let exp = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/bruker/cheminfo/hsqc-incompleted-acquisition/5");
    if !exp.is_dir() {
        return;
    }
    let raw = bruker::read_ser(&exp).unwrap();
    assert_eq!(raw.rows.len(), 100);
    let s = process2d::process(&raw, &process2d::default_2d(&raw)).unwrap();
    assert_eq!((s.rows.len(), s.rows[0].len()), (1024, 1024));
}

/// The stored `2rr` is kept in tiles (`XDIM`); this COSY has 128-point tiles in a
/// 2048-point square. A COSY is near symmetric about its diagonal: read as
/// nmrglue's `read_pdata` reorders it, it correlates 0.758 with its own
/// transpose, and the file read straight through −0.001.
#[test]
fn processed_2d_tiles_are_put_back_in_place() {
    let pdata = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/bruker/cheminfo/nonUniformSampling/cosy-nus/2/pdata/1");
    if !pdata.is_dir() {
        return;
    }
    let s = bruker::read_processed_2d(&pdata).unwrap();
    let n = s.rows.len();
    assert_eq!(n, s.rows[0].len());
    let transposed: Vec<Complex64> = (0..n)
        .flat_map(|k| s.rows.iter().map(move |row| row[k]))
        .collect();
    let c = correlation(&s.rows.concat(), &transposed);
    assert!(c > 0.5, "correlation with its own transpose {c:.4}");
}

fn correlation(a: &[Complex64], b: &[Complex64]) -> f64 {
    let n = a.len() as f64;
    let (ma, mb) = (
        a.iter().map(|z| z.re).sum::<f64>() / n,
        b.iter().map(|z| z.re).sum::<f64>() / n,
    );
    let (mut ab, mut aa, mut bb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        let (x, y) = (x.re - ma, y.re - mb);
        ab += x * y;
        aa += x * x;
        bb += y * y;
    }
    ab / (aa * bb).sqrt()
}

fn read_complex(path: &Path) -> Vec<Complex64> {
    read_f64(path)
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&[re, im]| Complex64::new(re, im))
        .collect()
}

fn find(dir: &Path, name: &str) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for path in entries.filter_map(|e| e.ok().map(|e| e.path())) {
        if path.is_dir() {
            found.extend(find(&path, name));
        } else if path.file_name().is_some_and(|n| n == name) {
            found.push(path);
        }
    }
    found.sort();
    found
}

fn read_f64(path: &Path) -> Vec<f64> {
    fs::read(path)
        .unwrap()
        .as_chunks::<8>()
        .0
        .iter()
        .map(|&b| f64::from_le_bytes(b))
        .collect()
}

/// How far the spectrum as opened is from the stored one, as a fraction of its
/// tallest line, once a complex scale and a first order within ±30° are fitted:
/// what a correlation near 1 cannot see, the feet of the lines.
fn residual_from_processed(ours: &[Complex64], stored: &[f64]) -> f64 {
    let n = ours.len() as f64;
    let top = stored.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    (-60..=60)
        .map(|half_degrees| {
            let p1 = f64::from(half_degrees) * 0.5;
            let z: Vec<Complex64> = ours
                .iter()
                .enumerate()
                .map(|(k, z)| z * Complex64::from_polar(1.0, (p1 * k as f64 / n).to_radians()))
                .collect();
            // stored ≈ a·re + b·im, by least squares.
            let (mut rr, mut ii, mut ri, mut ry, mut iy) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for (z, y) in z.iter().zip(stored) {
                (rr, ii, ri) = (rr + z.re * z.re, ii + z.im * z.im, ri + z.re * z.im);
                (ry, iy) = (ry + z.re * y, iy + z.im * y);
            }
            let det = rr * ii - ri * ri;
            let (a, b) = ((ry * ii - iy * ri) / det, (iy * rr - ry * ri) / det);
            let sum: f64 = z
                .iter()
                .zip(stored)
                .map(|(z, y)| (y - a * z.re - b * z.im).powi(2))
                .sum();
            (sum / n).sqrt() / top
        })
        .fold(f64::INFINITY, f64::min)
}

/// The digital filter taken off as a first-order phase: a delay that is not a
/// whole number of points, shifted out of the FID instead, bent the feet of the
/// strong lines (aspirin 6.6e-4, strychnine 4.7e-3 of the tallest line).
#[test]
fn feet_of_the_lines_match_processed() {
    for (rel, limit) in [
        ("cheminfo/aspirin-1h/1", 2e-4),
        ("cheminfo/strychnine-1h/strychnine/10", 1e-3),
    ] {
        let Some((_, raw, stored)) = processed_pairs().into_iter().find(|p| p.0 == rel) else {
            continue;
        };
        let ops = process::auto_1d(&raw, false, false).unwrap();
        let ours = process::process(&raw, &ops).unwrap();
        let stored: Vec<f64> = stored.data.iter().map(|z| z.re).collect();
        let residual = residual_from_processed(&ours.data, &stored);
        assert!(
            residual < limit,
            "{rel}: {residual:.2e} of the tallest line"
        );
    }
}
