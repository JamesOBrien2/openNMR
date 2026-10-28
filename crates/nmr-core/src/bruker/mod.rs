//! Bruker datasets.

mod dsp;
pub mod params;

use std::fs;
use std::path::{Path, PathBuf};

use crate::process::Operation;
use crate::{Complex64, Domain, Error, Metadata, Nus, Quadrature, Result, Spectrum, Spectrum2D};
use params::Parameters;

/// Finds experiment folders from a path the user opened: an experiment folder,
/// a file or `pdata/N` folder inside one, or a folder holding numbered experiments.
pub fn find_experiments(path: &Path) -> Result<Vec<PathBuf>> {
    let dir = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    // A pdata folder is opened as the stored processed spectrum, so that it
    // can be compared with what the app computes from the FID.
    if dir.join("1r").is_file() {
        return Ok(vec![dir.to_path_buf()]);
    }
    if let Some(exp) = dir.ancestors().take(3).find(|d| is_experiment(d)) {
        return Ok(vec![exp.to_path_buf()]);
    }
    let entries = fs::read_dir(dir).map_err(|source| io_error(dir, source))?;
    let mut experiments: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| is_experiment(p))
        .collect();
    if experiments.is_empty() {
        return Err(Error::NoExperiment(dir.to_path_buf()));
    }
    experiments.sort_by_key(|p| {
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned());
        let number = name.as_deref().and_then(|n| n.parse::<u64>().ok());
        (number.is_none(), number, name)
    });
    Ok(experiments)
}

fn is_experiment(dir: &Path) -> bool {
    dir.join("acqus").is_file() && (dir.join("fid").is_file() || dir.join("ser").is_file())
}

/// Reads the raw 1D FID of an experiment folder, truncated to `TD`.
pub fn read_fid(dir: &Path) -> Result<Spectrum> {
    if dir.join("acqu2s").is_file() {
        return Err(Error::Unsupported(
            "2D experiments are not supported yet".into(),
        ));
    }
    let acqus = read_parameters(&dir.join("acqus"))?;
    // nmrglue reads processing parameters from pdata/1; they give SF (with SR).
    let procs = read_parameters(&dir.join("pdata/1/procs")).ok();

    let aq_mod = acqus.i64_or("AQ_mod", 0)?;
    if !matches!(aq_mod, 1 | 3) {
        return Err(Error::Unsupported(format!(
            "single-channel acquisition (AQ_mod = {aq_mod})"
        )));
    }
    let td = usize::try_from(acqus.i64("TD")?).map_err(|_| invalid("TD", &acqus))?;
    let path = dir.join("fid");
    let bytes = fs::read(&path).map_err(|source| io_error(&path, source))?;
    let values = decode(&bytes, &acqus, "BYTORDA", "DTYPA")?;
    if values.len() < td {
        return Err(Error::Truncated {
            path,
            expected: td,
            found: values.len(),
        });
    }
    Ok(Spectrum {
        data: complex(&values[..td]),
        meta: metadata(&acqus, procs.as_ref())?,
        domain: Domain::Time,
    })
}

/// Reads the raw `ser` file of a 2D experiment: one FID per F1 increment, each
/// padded to a 1024-byte block as nmrglue's `read` expects. An acquisition that
/// stopped early is read as far as it got.
pub fn read_ser(dir: &Path) -> Result<Spectrum2D> {
    let acqus = read_parameters(&dir.join("acqus"))?;
    let acqu2s = read_parameters(&dir.join("acqu2s"))?;
    let procs = read_parameters(&dir.join("pdata/1/procs")).ok();
    let proc2s = read_parameters(&dir.join("pdata/1/proc2s")).ok();

    // A series of 1D spectra (relaxation, diffusion) has a list of delays or
    // powers beside it, and it is not transformed in F1.
    let series = ["vdlist", "vclist", "vplist", "vtlist", "difflist"]
        .iter()
        .any(|f| dir.join(f).is_file())
        || proc2s
            .as_ref()
            .is_some_and(|p| matches!(p.i64("FT_mod"), Ok(0)));
    if series {
        return Err(Error::Unsupported(
            "pseudo-2D experiments (relaxation and diffusion series)".into(),
        ));
    }
    let quadrature = match acqu2s.i64("FnMODE")? {
        1 => Quadrature::Qf,
        6 => Quadrature::EchoAntiecho,
        mode => {
            return Err(Error::Unsupported(format!(
                "F1 acquisition mode FnMODE = {mode}"
            )));
        }
    };
    let aq_mod = acqus.i64_or("AQ_mod", 0)?;
    if !matches!(aq_mod, 1 | 3) {
        return Err(Error::Unsupported(format!(
            "single-channel acquisition (AQ_mod = {aq_mod})"
        )));
    }

    let td = usize::try_from(acqus.i64("TD")?).map_err(|_| invalid("TD", &acqus))?;
    let td1 = usize::try_from(acqu2s.i64("TD")?).map_err(|_| invalid("TD", &acqu2s))?;
    let path = dir.join("ser");
    let bytes = fs::read(&path).map_err(|source| io_error(&path, source))?;
    let values = decode(&bytes, &acqus, "BYTORDA", "DTYPA")?;
    let width = if acqus.i64_or("DTYPA", 0)? == 2 { 8 } else { 4 };
    let stride = (td * width).div_ceil(1024) * 1024 / width;
    let mut count = td1.min(values.len() / stride.max(1));
    if quadrature == Quadrature::EchoAntiecho {
        count -= count % 2;
    }
    if count == 0 {
        return Err(Error::Truncated {
            path,
            expected: td1 * stride,
            found: values.len(),
        });
    }
    let rows = values
        .chunks_exact(stride)
        .take(count)
        .map(|row| complex(&row[..td]))
        .collect();
    // Echo and antiecho rows make one F1 point between them.
    let per_point = if quadrature == Quadrature::Qf { 1 } else { 2 };
    let nus = sampling(dir, &acqu2s, count / per_point, per_point)?;
    // What linear prediction carries on from: the whole grid when sampled
    // non-uniformly, with `NusTD` the TD it stands for.
    let (full_td, full_points) = match &nus {
        Some(nus) => (nus.grid * per_point, nus.grid),
        None => (td1, count / per_point),
    };

    let f2 = metadata(&acqus, procs.as_ref())?;
    Ok(Spectrum2D {
        rows,
        f1: indirect_metadata(&acqu2s, proc2s.as_ref())?,
        domain: [Domain::Time; 2],
        quadrature,
        along_f1: false,
        windows: match (&procs, &proc2s) {
            (Some(_), _) | (_, Some(_)) => [window(procs.as_ref()), window(proc2s.as_ref())],
            (None, None) => std::array::from_fn(|_| unprocessed_window(quadrature)),
        },
        f2_points: procs
            .as_ref()
            .and_then(|p| p.i64("TDeff").ok())
            .and_then(|n| usize::try_from(n / 2).ok())
            .filter(|&n| n > 0 && n < td / 2),
        magnitude: proc2s
            .as_ref()
            .is_some_and(|p| matches!(p.i64("PH_mod"), Ok(2))),
        f1_prediction: proc2s
            .as_ref()
            .and_then(|p| prediction(p, full_td, full_points)),
        nus,
        f2,
    })
}

/// Where the `points` F1 points acquired belong, from `nuslist`: one entry per
/// point, in the order acquired, on a grid of `NusTD` divided by the rows that
/// make a point (`per_point`). Measured on every non-uniformly sampled 2D in
/// testdata, the entries reach `NusTD − 2` for QF and `NusTD/2 − 1` for
/// echo-antiecho. A list longer than what was acquired is an acquisition that
/// stopped early, and only its start is used.
fn sampling(
    dir: &Path,
    acqu2s: &Parameters,
    points: usize,
    per_point: usize,
) -> Result<Option<Nus>> {
    let path = dir.join("nuslist");
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path).map_err(|source| io_error(&path, source))?;
    let bad = |why: &str| Error::Unsupported(format!("{}: {why}", path.display()));
    let grid =
        usize::try_from(acqu2s.i64("NusTD")?).map_err(|_| invalid("NusTD", acqu2s))? / per_point;
    let all = text
        .split_whitespace()
        .map(|t| {
            t.parse::<usize>()
                .map_err(|_| bad("an entry is not a whole number"))
        })
        .collect::<Result<Vec<_>>>()?;
    if all.len() < points {
        return Err(bad("fewer entries than increments acquired"));
    }
    let used = all[..points].to_vec();
    let mut seen = vec![false; grid];
    for &k in &used {
        if k >= grid || std::mem::replace(&mut seen[k], true) {
            return Err(bad("an entry lies off the grid or is repeated"));
        }
    }
    Ok(Some(Nus { points: used, grid }))
}

/// The recorded forward complex linear prediction in F1 (`ME_mod` 2) as an
/// operation: `NCOEF` coefficients, carrying the F1 series on in proportion
/// to `TDeff` over the acquired `TD`. Every 2D in testdata that uses it asks
/// for twice the increments. Other kinds (`ME_mod` 1, 3 and 4: real, or
/// backward) are not ported yet.
fn prediction(proc2s: &Parameters, td1: usize, points: usize) -> Option<Operation> {
    let order = usize::try_from(proc2s.i64("NCOEF").ok()?).ok()?;
    let target = usize::try_from(proc2s.i64("TDeff").ok()?).ok()?;
    let size = points * target / td1.max(1);
    (proc2s.i64("ME_mod").ok()? == 2 && order > 0 && size > points)
        .then_some(Operation::LinearPrediction { size, order })
}

/// Reads a processed 2D spectrum from a `pdata/N` folder: `2rr`, stored in
/// tiles of `XDIM` points as nmrglue's `read_pdata` reorders them
/// (`reorder_submatrix`), multiplied by `2^NC_proc`.
pub fn read_processed_2d(dir: &Path) -> Result<Spectrum2D> {
    let procs = read_parameters(&dir.join("procs"))?;
    let proc2s = read_parameters(&dir.join("proc2s"))?;
    let size = |p: &Parameters, key: &str| {
        usize::try_from(p.i64(key)?)
            .ok()
            .filter(|&n| n > 0)
            .ok_or_else(|| invalid(key, p))
    };
    let (si2, si1) = (size(&procs, "SI")?, size(&proc2s, "SI")?);
    let (x2, x1) = (size(&procs, "XDIM")?, size(&proc2s, "XDIM")?);
    if si2 % x2 != 0 || si1 % x1 != 0 {
        return Err(invalid("XDIM", &procs));
    }
    let path = dir.join("2rr");
    let bytes = fs::read(&path).map_err(|source| io_error(&path, source))?;
    let values = decode(&bytes, &procs, "BYTORDP", "DTYPP")?;
    if values.len() < si1 * si2 {
        return Err(Error::Truncated {
            path,
            expected: si1 * si2,
            found: values.len(),
        });
    }
    let scale = 2f64.powi(procs.i64_or("NC_proc", 0)? as i32);
    let mut rows = vec![vec![Complex64::ZERO; si2]; si1];
    let mut tile = values.chunks_exact(x2);
    for t1 in 0..si1 / x1 {
        for t2 in 0..si2 / x2 {
            for row in &mut rows[t1 * x1..(t1 + 1) * x1] {
                for (z, v) in row[t2 * x2..(t2 + 1) * x2]
                    .iter_mut()
                    .zip(tile.next().unwrap_or(&[]))
                {
                    *z = Complex64::new(v * scale, 0.0);
                }
            }
        }
    }
    Ok(Spectrum2D {
        rows,
        f2: processed_metadata(&procs)?,
        f1: processed_metadata(&proc2s)?,
        domain: [Domain::Frequency; 2],
        quadrature: Quadrature::Qf,
        along_f1: true,
        windows: [None, None],
        f2_points: None,
        magnitude: false,
        f1_prediction: None,
        nus: None,
    })
}

/// Complex points from interleaved real and imaginary values.
fn complex(values: &[f64]) -> Vec<Complex64> {
    values
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&[re, im]| Complex64::new(re, im))
        .collect()
}

/// Reads a processed 1D spectrum from a `pdata/N` folder: `1r` and, when
/// present, `1i`, multiplied by `2^NC_proc` as nmrglue's `read_pdata` with
/// `scale_pdata` does. The axis comes from `OFFSET` (ppm of the first point),
/// `SW_p` and `SF`.
pub fn read_processed(dir: &Path) -> Result<Spectrum> {
    let procs = read_parameters(&dir.join("procs"))?;
    let si = usize::try_from(procs.i64("SI")?).map_err(|_| invalid("SI", &procs))?;
    let scale = 2f64.powi(procs.i64_or("NC_proc", 0)? as i32);
    let read = |name: &str| -> Result<Option<Vec<f64>>> {
        let path = dir.join(name);
        if !path.is_file() {
            return Ok(None);
        }
        let bytes = fs::read(&path).map_err(|source| io_error(&path, source))?;
        let values = decode(&bytes, &procs, "BYTORDP", "DTYPP")?;
        if values.len() < si {
            return Err(Error::Truncated {
                path,
                expected: si,
                found: values.len(),
            });
        }
        Ok(Some(values[..si].iter().map(|v| v * scale).collect()))
    };
    let real = read("1r")?.ok_or_else(|| Error::NoExperiment(dir.to_path_buf()))?;
    let imag = read("1i")?.unwrap_or_else(|| vec![0.0; si]);

    Ok(Spectrum {
        data: real
            .iter()
            .zip(&imag)
            .map(|(&re, &im)| Complex64::new(re, im))
            .collect(),
        meta: processed_metadata(&procs)?,
        domain: Domain::Frequency,
    })
}

/// The axis of a processed dimension from `OFFSET` (ppm of the first point),
/// `SW_p` and `SF`.
fn processed_metadata(procs: &Parameters) -> Result<Metadata> {
    let sw_hz = procs.f64("SW_p")?;
    let obs_mhz = procs.f64("SF")?;
    Ok(Metadata {
        nucleus: procs.str("AXNUC")?.to_string(),
        sw_hz,
        obs_mhz,
        carrier_hz: procs.f64("OFFSET")? * obs_mhz - sw_hz / 2.0,
        group_delay: 0.0,
        solvent: None,
        phase: None,
        conjugate: false,
        signed: false,
        size: None,
        line_broadening: None,
        delay_at_ft: false,
    })
}

/// What a dataset says about how it was recorded, for the metadata table.
///
/// A curated list rather than every key: a JCAMP file carries several hundred,
/// most of them instrument housekeeping. Anything the file does not have is left
/// out rather than shown empty.
pub fn describe(dir: &Path) -> Result<Vec<(String, String)>> {
    let acqus = read_parameters(&dir.join("acqus"))?;
    let procs = read_parameters(&dir.join("pdata/1/procs")).ok();
    let mut rows: Vec<(String, String)> = Vec::new();
    let mut text = |label: &str, key: &str, unit: &str| {
        if let Ok(value) = acqus.str(key) {
            rows.push((label.to_string(), format!("{value}{unit}")));
        }
    };
    text("Experiment", "PULPROG", "");
    text("Nucleus", "NUC1", "");
    text("Solvent", "SOLVENT", "");
    text("Instrument", "INSTRUM", "");
    text("Probe", "PROBHD", "");

    let mut number = |label: &str, key: &str, unit: &str, decimals: usize| {
        if let Ok(value) = acqus.f64(key) {
            rows.push((label.to_string(), format!("{value:.decimals$}{unit}")));
        }
    };
    number("Frequency", "SFO1", " MHz", 4);
    number("Spectral width", "SW_h", " Hz", 2);
    number("Spectral width", "SW", " ppm", 3);
    number("Acquired points", "TD", "", 0);
    number("Scans", "NS", "", 0);
    number("Dummy scans", "DS", "", 0);
    number("Receiver gain", "RG", "", 0);
    number("Temperature", "TE", " K", 1);
    number("Relaxation delay", "D", " s", 0);

    if let Some(procs) = &procs {
        for (label, key, unit) in [
            ("Processed points", "SI", ""),
            ("Line broadening", "LB", " Hz"),
            ("Phase PH0", "PHC0", "°"),
            ("Phase PH1", "PHC1", "°"),
        ] {
            if let Ok(value) = procs.f64(key) {
                rows.push((label.to_string(), format!("{value}{unit}")));
            }
        }
    }

    // Then everything the files carry, so nothing recorded is hidden. The title
    // comes first: it is what the experiment was called when it was run.
    if let Ok(title) = fs::read_to_string(dir.join("pdata/1/title")) {
        let title = title.trim();
        if !title.is_empty() {
            rows.insert(0, ("Title".to_string(), title.to_string()));
        }
    }
    rows.push((String::new(), String::new()));
    rows.push(("Acquisition".to_string(), "acqus".to_string()));
    rows.extend(acqus.entries());
    if let Some(procs) = &procs {
        rows.push((String::new(), String::new()));
        rows.push(("Processing".to_string(), "procs".to_string()));
        rows.extend(procs.entries());
    }
    Ok(rows)
}

/// Decodes a whole binary file with the byte order and data type named by
/// `order_key` and `type_key`. Bruker pads each FID to a 1024-byte block; callers
/// cut the padding using `TD` or `SI`.
fn decode(bytes: &[u8], params: &Parameters, order_key: &str, type_key: &str) -> Result<Vec<f64>> {
    let big = match params.i64_or(order_key, 0)? {
        0 => false,
        1 => true,
        _ => return Err(invalid(order_key, params)),
    };
    Ok(match params.i64_or(type_key, 0)? {
        0 => bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|&b| {
                f64::from(if big {
                    i32::from_be_bytes(b)
                } else {
                    i32::from_le_bytes(b)
                })
            })
            .collect(),
        2 => bytes
            .as_chunks::<8>()
            .0
            .iter()
            .map(|&b| {
                if big {
                    f64::from_be_bytes(b)
                } else {
                    f64::from_le_bytes(b)
                }
            })
            .collect(),
        _ => return Err(invalid(type_key, params)),
    })
}

/// Axis parameters following nmrglue's `bruker.add_axis_to_udic`.
fn metadata(acqus: &Parameters, procs: Option<&Parameters>) -> Result<Metadata> {
    let sfo1 = acqus.f64("SFO1")?;
    let sf = procs.map(|p| p.f64("SF")).transpose().ok().flatten();
    let (obs_mhz, carrier_hz) = match sf {
        Some(sf) => (sf, (sfo1 - sf) * 1e6),
        None => (sfo1, acqus.f64("O1")?),
    };
    let group_delay = group_delay(acqus)?;
    let recorded = recorded(procs);
    Ok(Metadata {
        nucleus: acqus.str("NUC1")?.to_string(),
        sw_hz: acqus.f64("SW_h")?,
        obs_mhz,
        carrier_hz,
        group_delay,
        solvent: acqus.str("SOLVENT").ok().map(str::to_string),
        // Named for the pulse programs that produce them: deptsp135, jmod, apt...
        signed: acqus.str("PULPROG").is_ok_and(|p| {
            let p = p.to_ascii_lowercase();
            ["dept", "apt", "jmod"].iter().any(|k| p.contains(k))
        }),
        // The digital receiver (DIGMOD 1, or 3 for baseopt) writes the quadrature
        // pair with the opposite sign to nmrglue's transform, so the spectrum
        // comes out mirrored about the carrier unless the FID is conjugated.
        // Against the stored `1r` spectra, 1H, 13C and 87Rb:
        // correlation 0.98-0.999 with, below 0.09 without. Only DIGMOD 0
        // (analog, and nmrglue's synthetic data) is left as it is.
        conjugate: acqus.i64_or("DIGMOD", 0)? != 0,
        ..recorded
    })
}

/// The F1 axis of a 2D experiment. Not [`metadata`] on `acqu2s`: there `SW_h`
/// is left at its 1D value (2000 Hz on the ¹³C HSQC and HMBC in `testdata`)
/// while `SW` in ppm is kept up to date, and `SW × SFO1` equals the stored
/// `SW_p` on every dataset; nmrglue reads `SW_h`. `NUC1` may be missing, and
/// `DIGMOD` describes the receiver, which only F2 goes through.
fn indirect_metadata(acqu2s: &Parameters, proc2s: Option<&Parameters>) -> Result<Metadata> {
    let sfo1 = acqu2s.f64("SFO1")?;
    let sf = proc2s.and_then(|p| p.f64("SF").ok());
    let (obs_mhz, carrier_hz) = match sf {
        Some(sf) => (sf, (sfo1 - sf) * 1e6),
        None => (sfo1, acqu2s.f64("O1")?),
    };
    let nucleus = proc2s
        .and_then(|p| p.str("AXNUC").ok())
        .or_else(|| acqu2s.str("NUC1").ok())
        .unwrap_or("off");
    Ok(Metadata {
        nucleus: nucleus.to_string(),
        sw_hz: acqu2s.f64("SW")? * sfo1,
        obs_mhz,
        carrier_hz,
        group_delay: 0.0,
        solvent: None,
        signed: false,
        conjugate: false,
        ..recorded(proc2s)
    })
}

/// What a processing parameter file records: phase, size and window.
fn recorded(procs: Option<&Parameters>) -> Metadata {
    Metadata {
        nucleus: String::new(),
        sw_hz: 0.0,
        obs_mhz: 0.0,
        carrier_hz: 0.0,
        group_delay: 0.0,
        solvent: None,
        signed: false,
        conjugate: false,
        // The phase the operator settled on, which is what the stored spectrum
        // shows. It applies to the spectrum once the digital filter's delay is
        // taken off as a first-order phase
        // (`Operation::DigitalFilter`); `process::shifted_phase` gives it for
        // the delay shifted out of the FID instead. All-zero means the dataset
        // was never phased.
        phase: procs
            .map(|p| [p.f64_or("PHC0", 0.0), p.f64_or("PHC1", 0.0)])
            .and_then(|pair| match pair {
                [Ok(p0), Ok(p1)] if p0 != 0.0 || p1 != 0.0 => Some([p0, p1]),
                _ => None,
            }),
        size: procs
            .and_then(|p| p.i64("SI").ok())
            .and_then(|si| usize::try_from(si).ok())
            .filter(|si| *si > 0),
        line_broadening: procs.and_then(|p| match p.i64("WDW").ok()? {
            0 => Some(0.0),
            1 => p.f64("LB").ok(),
            _ => None,
        }),
        delay_at_ft: false,
    }
}

/// The window for a 2D that was never processed, so has no `pdata` to say:
/// what the standard parameter sets use, and what every 2D in testdata
/// was processed with - a sine bell for a magnitude COSY or HMBC (`SINE`,
/// `SSB` 0), a squared cosine for a phase-sensitive HSQC (`QSINE`, `SSB` 2).
/// Without one, the truncated FID rings along both axes.
fn unprocessed_window(quadrature: Quadrature) -> Option<Operation> {
    Some(match quadrature {
        Quadrature::Qf => Operation::SineBell {
            offset: 0.0,
            end: 1.0,
            power: 1.0,
        },
        Quadrature::EchoAntiecho => Operation::SineBell {
            offset: 0.5,
            end: 1.0,
            power: 2.0,
        },
    })
}

/// The recorded window (`WDW`) as an operation. Its sine bells run from
/// `π/SSB` to π over the FID (`SSB` below 2 is a plain sine), which is nmrglue's
/// `proc_base.sp` with `off = 1/SSB` and `end = 1`.
fn window(procs: Option<&Parameters>) -> Option<Operation> {
    let p = procs?;
    let ssb = p.f64_or("SSB", 0.0).ok()?;
    let offset = if ssb < 2.0 { 0.0 } else { 1.0 / ssb };
    match p.i64("WDW").ok()? {
        1 => Some(Operation::Exponential {
            lb_hz: p.f64("LB").ok()?,
        }),
        3 | 4 => Some(Operation::SineBell {
            offset,
            end: 1.0,
            power: if p.i64("WDW").ok()? == 4 { 2.0 } else { 1.0 },
        }),
        // ponytail: Gaussian (GM) and the rarer windows give none; port them when a dataset needs one.
        _ => None,
    }
}

fn group_delay(acqus: &Parameters) -> Result<f64> {
    let grpdly = acqus.f64_or("GRPDLY", 0.0)?;
    if grpdly > 0.0 {
        return Ok(grpdly);
    }
    // Datasets from before digital filters have neither DSPFVS nor DECIM.
    if acqus.get("DSPFVS").is_none() {
        return Ok(0.0);
    }
    let dspfvs = acqus.i64("DSPFVS")?;
    let decim = acqus.f64("DECIM")? as i64;
    dsp::group_delay(dspfvs, decim).ok_or_else(|| {
        Error::Unsupported(format!(
            "unknown digital filter (DSPFVS = {dspfvs}, DECIM = {decim})"
        ))
    })
}

fn read_parameters(path: &Path) -> Result<Parameters> {
    let bytes = fs::read(path).map_err(|source| io_error(path, source))?;
    // Parameter files may contain Latin-1 in comments and titles.
    Ok(Parameters::parse(&String::from_utf8_lossy(&bytes)))
}

fn io_error(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn invalid(key: &str, params: &Parameters) -> Error {
    Error::InvalidParameter {
        key: key.to_string(),
        value: params.str(key).unwrap_or_default().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("nmr-core-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn experiment(&self, name: &str, acqus: &str, fid: &[u8]) -> PathBuf {
            let exp = self.0.join(name);
            fs::create_dir_all(exp.join("pdata/1")).unwrap();
            fs::write(exp.join("acqus"), acqus).unwrap();
            fs::write(exp.join("fid"), fid).unwrap();
            exp
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn acqus(extra: &str) -> String {
        format!(
            "##$TD= 6\n##$SW_h= 5000\n##$SFO1= 400.13\n##$O1= 1880\n##$NUC1= <1H>\n\
             ##$AQ_mod= 3\n##$GRPDLY= 67.98\n{extra}##END=\n"
        )
    }

    /// Six points padded to one 1024-byte block, as Bruker stores them.
    fn padded<const N: usize>(values: &[f64], encode: impl Fn(f64) -> [u8; N]) -> Vec<u8> {
        let mut bytes: Vec<u8> = values.iter().flat_map(|&v| encode(v)).collect();
        bytes.resize(1024, 0);
        bytes
    }

    const VALUES: [f64; 6] = [1.0, -2.0, 3.0, -4.0, 5.0, -6.0];

    fn expected() -> Vec<Complex64> {
        vec![
            Complex64::new(1.0, -2.0),
            Complex64::new(3.0, -4.0),
            Complex64::new(5.0, -6.0),
        ]
    }

    #[test]
    fn reads_int32_little_endian() {
        let fx = Fixture::new("i32le");
        let fid = padded(&VALUES, |v| (v as i32).to_le_bytes());
        let exp = fx.experiment("1", &acqus("##$BYTORDA= 0\n##$DTYPA= 0\n"), &fid);
        let s = read_fid(&exp).unwrap();
        assert_eq!(s.data, expected());
        assert_eq!(s.domain, Domain::Time);
        assert_eq!(s.meta.group_delay, 67.98);
        assert_eq!(s.meta.carrier_hz, 1880.0);
    }

    #[test]
    fn reads_int32_big_endian() {
        let fx = Fixture::new("i32be");
        let fid = padded(&VALUES, |v| (v as i32).to_be_bytes());
        let exp = fx.experiment("1", &acqus("##$BYTORDA= 1\n##$DTYPA= 0\n"), &fid);
        assert_eq!(read_fid(&exp).unwrap().data, expected());
    }

    #[test]
    fn reads_float64_both_orders() {
        let fx = Fixture::new("f64");
        let le = fx.experiment(
            "1",
            &acqus("##$BYTORDA= 0\n##$DTYPA= 2\n"),
            &padded(&VALUES, f64::to_le_bytes),
        );
        let be = fx.experiment(
            "2",
            &acqus("##$BYTORDA= 1\n##$DTYPA= 2\n"),
            &padded(&VALUES, f64::to_be_bytes),
        );
        assert_eq!(read_fid(&le).unwrap().data, expected());
        assert_eq!(read_fid(&be).unwrap().data, expected());
    }

    #[test]
    fn short_file_is_an_error() {
        let fx = Fixture::new("short");
        let fid: Vec<u8> = VALUES[..4]
            .iter()
            .flat_map(|&v| (v as i32).to_le_bytes())
            .collect();
        let exp = fx.experiment("1", &acqus(""), &fid);
        assert!(matches!(
            read_fid(&exp),
            Err(Error::Truncated {
                expected: 6,
                found: 4,
                ..
            })
        ));
    }

    #[test]
    fn describes_how_a_dataset_was_recorded() {
        let fx = Fixture::new("describe");
        let exp = fx.experiment(
            "1",
            &acqus("##$PULPROG= <zg30>\n##$SOLVENT= <CDCl3>\n##$NS= 16\n"),
            &padded(&VALUES, |v| (v as i32).to_le_bytes()),
        );
        let rows = describe(&exp).unwrap();
        let find = |label: &str| {
            rows.iter()
                .find(|(name, _)| name == label)
                .map(|(_, value)| value.clone())
        };
        assert_eq!(find("Experiment").as_deref(), Some("zg30"));
        assert_eq!(find("Solvent").as_deref(), Some("CDCl3"));
        assert_eq!(find("Scans").as_deref(), Some("16"));
        assert_eq!(find("Nucleus").as_deref(), Some("1H"));
        // Nothing is invented for what the file does not carry.
        assert!(find("Probe").is_none());
        // And every key of the file is there to be read, under its own heading.
        assert!(rows.iter().any(|(name, _)| name == "Acquisition"));
        assert!(
            rows.iter()
                .any(|(name, value)| name == "PULPROG" && value == "zg30")
        );
    }

    #[test]
    fn digital_receiver_datasets_are_conjugated() {
        let fx = Fixture::new("digmod");
        let fid = padded(&VALUES, |v| (v as i32).to_le_bytes());
        let analog = fx.experiment("1", &acqus(""), &fid);
        let digital = fx.experiment("2", &acqus("##$DIGMOD= 1\n"), &fid);
        let baseopt = fx.experiment("3", &acqus("##$DIGMOD= 3\n"), &fid);
        assert!(!read_fid(&analog).unwrap().meta.conjugate);
        assert!(read_fid(&digital).unwrap().meta.conjugate);
        assert!(read_fid(&baseopt).unwrap().meta.conjugate);
    }

    #[test]
    fn pdata_folder_opens_the_processed_spectrum() {
        let fx = Fixture::new("pdata-open");
        let exp = fx.experiment(
            "1",
            &acqus(""),
            &padded(&VALUES, |v| (v as i32).to_le_bytes()),
        );
        let pdata = exp.join("pdata/1");
        fs::write(
            pdata.join("procs"),
            "##$SF= 400.13\n##$SI= 3\n##$SW_p= 5000\n##$OFFSET= 7\n##$AXNUC= <1H>\n##END=\n",
        )
        .unwrap();
        fs::write(
            pdata.join("1r"),
            padded(&VALUES[..3], |v| (v as i32).to_le_bytes()),
        )
        .unwrap();
        assert_eq!(find_experiments(&pdata).unwrap(), vec![pdata.clone()]);
        assert_eq!(
            find_experiments(&pdata.join("1r")).unwrap(),
            vec![pdata.clone()]
        );
        // Without the processed file the experiment folder is opened instead.
        assert_eq!(find_experiments(&exp.join("pdata")).unwrap(), vec![exp]);
        let s = read_processed(&pdata).unwrap();
        assert_eq!(s.domain, Domain::Frequency);
        assert_eq!(s.data.len(), 3);
    }

    #[test]
    fn procs_sf_sets_axis_reference() {
        let fx = Fixture::new("procs");
        let exp = fx.experiment(
            "1",
            &acqus(""),
            &padded(&VALUES, |v| (v as i32).to_le_bytes()),
        );
        fs::write(exp.join("pdata/1/procs"), "##$SF= 400.13\n##END=\n").unwrap();
        let meta = read_fid(&exp).unwrap().meta;
        assert_eq!(meta.obs_mhz, 400.13);
        assert_eq!(meta.carrier_hz, 0.0);
    }

    #[test]
    fn raw_and_processed_axes_share_the_recorded_reference() {
        let fx = Fixture::new("axis-reference");
        let input = "##$TD= 6\n##$SW_h= 4000\n##$SFO1= 400.002\n##$O1= 2000\n\
                     ##$NUC1= <1H>\n##$AQ_mod= 3\n##$GRPDLY= 0\n##END=\n";
        let exp = fx.experiment("1", input, &padded(&VALUES, |v| (v as i32).to_le_bytes()));
        let pdata = exp.join("pdata/1");
        fs::write(
            pdata.join("procs"),
            "##$SF= 400\n##$SI= 3\n##$SW_p= 4000\n##$OFFSET= 10\n##$AXNUC= <1H>\n##END=\n",
        )
        .unwrap();
        fs::write(pdata.join("1r"), [0u8; 12]).unwrap();
        let raw_axis = read_fid(&exp).unwrap().ppm_axis();
        let processed_axis = read_processed(&pdata).unwrap().ppm_axis();
        assert!((raw_axis.first - 10.0).abs() < 1e-9);
        assert!((raw_axis.first - processed_axis.first).abs() < 1e-9);
        assert!((raw_axis.delta - processed_axis.delta).abs() < 1e-9);
    }

    #[test]
    fn recorded_phase_is_read_as_it_is() {
        let fx = Fixture::new("phc0");
        let exp = fx.experiment(
            "1",
            &acqus(""),
            &padded(&VALUES, |v| (v as i32).to_le_bytes()),
        );
        let procs = "##$SF= 400.13\n##$PHC0= -100\n##$PHC1= 9\n##END=\n";
        fs::write(exp.join("pdata/1/procs"), procs).unwrap();
        assert_eq!(read_fid(&exp).unwrap().meta.phase, Some([-100.0, 9.0]));
    }

    #[test]
    fn group_delay_from_table_when_grpdly_missing() {
        let text = acqus("").replace("##$GRPDLY= 67.98\n", "##$DSPFVS= 10\n##$DECIM= 1024\n");
        assert_eq!(
            group_delay(&Parameters::parse(&text)).unwrap(),
            70.48486328125
        );
        let old = acqus("").replace("##$GRPDLY= 67.98\n", "");
        assert_eq!(group_delay(&Parameters::parse(&old)).unwrap(), 0.0);
    }

    #[test]
    fn finds_experiments_from_any_level() {
        let fx = Fixture::new("find");
        let fid = padded(&VALUES, |v| (v as i32).to_le_bytes());
        let ten = fx.experiment("10", &acqus(""), &fid);
        let two = fx.experiment("2", &acqus(""), &fid);
        fs::create_dir_all(fx.0.join("notes")).unwrap();

        assert_eq!(find_experiments(&fx.0).unwrap(), vec![two.clone(), ten]);
        assert_eq!(find_experiments(&two).unwrap(), vec![two.clone()]);
        assert_eq!(
            find_experiments(&two.join("fid")).unwrap(),
            vec![two.clone()]
        );
        assert_eq!(find_experiments(&two.join("pdata/1")).unwrap(), vec![two]);
        assert!(matches!(
            find_experiments(&fx.0.join("notes")),
            Err(Error::NoExperiment(_))
        ));
    }

    #[test]
    fn reads_processed_spectrum_with_scaling() {
        let fx = Fixture::new("pdata");
        let exp = fx.experiment("1", &acqus(""), &[0; 1024]);
        let pdata = exp.join("pdata/1");
        fs::write(
            pdata.join("procs"),
            "##$SI= 3\n##$NC_proc= 2\n##$BYTORDP= 1\n##$DTYPP= 0\n##$SF= 400.0\n\
             ##$SW_p= 8000.0\n##$OFFSET= 12.0\n##$AXNUC= <1H>\n##END=\n",
        )
        .unwrap();
        let r: Vec<u8> = [1i32, -2, 3, 99]
            .iter()
            .flat_map(|v| v.to_be_bytes())
            .collect();
        fs::write(pdata.join("1r"), r).unwrap();

        let s = read_processed(&pdata).unwrap();
        assert_eq!(s.domain, Domain::Frequency);
        let re: Vec<f64> = s.data.iter().map(|z| z.re).collect();
        assert_eq!(re, vec![4.0, -8.0, 12.0]);
        assert!(s.data.iter().all(|z| z.im == 0.0));
        let axis = s.ppm_axis();
        assert!((axis.first - 12.0).abs() < 1e-12);
        assert!((axis.delta + 20.0 / 3.0).abs() < 1e-12);
    }

    /// A 2D experiment of three increments of six values, each padded to its
    /// own 1024-byte block, with `FnMODE` as given.
    fn ser(fx: &Fixture, fn_mode: i64) -> PathBuf {
        let rows: Vec<u8> = (0..3)
            .flat_map(|r| {
                padded(&VALUES.map(|v| v + 10.0 * f64::from(r)), |v| {
                    (v as i32).to_le_bytes()
                })
            })
            .collect();
        let exp = fx.experiment("1", &acqus(""), &[]);
        fs::remove_file(exp.join("fid")).unwrap();
        fs::write(exp.join("ser"), rows).unwrap();
        fs::write(
            exp.join("acqu2s"),
            format!("##$TD= 3\n##$FnMODE= {fn_mode}\n##$SW= 10\n##$SFO1= 100.6\n##$O1= 0\n##$NUC1= <13C>\n##END=\n"),
        )
        .unwrap();
        exp
    }

    #[test]
    fn reads_2d_rows_past_their_padding() {
        let fx = Fixture::new("ser");
        let s = read_ser(&ser(&fx, 1)).unwrap();
        assert_eq!(s.rows.len(), 3);
        assert_eq!(s.rows[0], expected());
        assert_eq!(s.rows[2][0], Complex64::new(21.0, 18.0));
        assert_eq!(s.quadrature, Quadrature::Qf);
        // Never processed: the window a magnitude 2D is processed with.
        let sine = Operation::SineBell {
            offset: 0.0,
            end: 1.0,
            power: 1.0,
        };
        assert_eq!(s.windows, [Some(sine.clone()), Some(sine)]);
        // The F1 width comes from SW in ppm, not from SW_h.
        assert!((s.f1.sw_hz - 1006.0).abs() < 1e-9);
        assert_eq!(s.f1.nucleus, "13C");

        // Echo and antiecho come in pairs: the odd row out is left.
        let fx = Fixture::new("ser-ea");
        let s = read_ser(&ser(&fx, 6)).unwrap();
        assert_eq!((s.rows.len(), s.quadrature), (2, Quadrature::EchoAntiecho));
        assert!(matches!(
            s.windows[1],
            Some(Operation::SineBell { power: 2.0, .. })
        ));
    }

    #[test]
    fn rejects_2d_it_cannot_process() {
        let fx = Fixture::new("ser-nus-bad");
        let exp = ser(&fx, 1);
        let acqu2s = fs::read_to_string(exp.join("acqu2s")).unwrap();
        fs::write(
            exp.join("acqu2s"),
            acqu2s.replace("##END=", "##$NusTD= 8\n##END="),
        )
        .unwrap();
        // Two entries for three increments, and one repeated.
        fs::write(exp.join("nuslist"), "0\n2\n").unwrap();
        assert!(matches!(read_ser(&exp), Err(Error::Unsupported(_))));
        fs::write(exp.join("nuslist"), "0\n2\n2\n").unwrap();
        assert!(matches!(read_ser(&exp), Err(Error::Unsupported(_))));

        let fx = Fixture::new("ser-series");
        let exp = ser(&fx, 1);
        fs::write(exp.join("vdlist"), "0.1\n1\n").unwrap();
        assert!(matches!(read_ser(&exp), Err(Error::Unsupported(_))));

        let fx = Fixture::new("ser-tppi");
        assert!(matches!(read_ser(&ser(&fx, 2)), Err(Error::Unsupported(_))));
    }

    #[test]
    fn reads_where_non_uniform_increments_belong() {
        let fx = Fixture::new("ser-nus");
        let exp = ser(&fx, 1);
        let acqu2s = fs::read_to_string(exp.join("acqu2s")).unwrap();
        fs::write(
            exp.join("acqu2s"),
            acqu2s.replace("##END=", "##$NusTD= 8\n##END="),
        )
        .unwrap();
        // A list longer than what was acquired: only its start is used.
        fs::write(exp.join("nuslist"), "0\n5\n2\n7\n").unwrap();
        let s = read_ser(&exp).unwrap();
        assert_eq!(
            s.nus,
            Some(Nus {
                points: vec![0, 5, 2],
                grid: 8
            })
        );
    }

    #[test]
    fn processed_2d_tiles_are_reordered() {
        let fx = Fixture::new("2rr");
        let exp = fx.experiment("1", &acqus(""), &[0; 1024]);
        let pdata = exp.join("pdata/1");
        let procs = "##$SI= 4\n##$XDIM= 2\n##$SF= 400.0\n##$SW_p= 4000.0\n##$OFFSET= 10.0\n##$AXNUC= <1H>\n##END=\n";
        fs::write(pdata.join("procs"), procs).unwrap();
        fs::write(pdata.join("proc2s"), procs).unwrap();
        // Four 2×2 tiles, the tiles along F2 first, each row by row.
        let stored = [0, 1, 4, 5, 2, 3, 6, 7, 8, 9, 12, 13, 10, 11, 14, 15];
        fs::write(
            pdata.join("2rr"),
            stored
                .iter()
                .flat_map(|v: &i32| v.to_le_bytes())
                .collect::<Vec<u8>>(),
        )
        .unwrap();
        let s = read_processed_2d(&pdata).unwrap();
        let values: Vec<f64> = s.rows.iter().flatten().map(|z| z.re).collect();
        assert_eq!(values, (0..16).map(f64::from).collect::<Vec<_>>());
    }

    #[test]
    fn rejects_2d() {
        let fx = Fixture::new("2d");
        let exp = fx.experiment("1", &acqus(""), &[0; 1024]);
        fs::write(exp.join("acqu2s"), "##$TD= 128\n").unwrap();
        assert!(matches!(read_fid(&exp), Err(Error::Unsupported(_))));
    }
}
