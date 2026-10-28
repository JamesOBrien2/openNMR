//! Reproducible native measurements, independent of the browser's illustrative backend.
//! cargo run -p nmr-core --example analysis_assessment -- [Bruker experiment folders]
use nmr_core::{PpmAxis, analysis, bruker, deconvolution, process};
use std::{f64::consts::PI, hint::black_box, path::Path, time::Instant};

fn timed<T>(mut run: impl FnMut() -> T) -> (T, f64) {
    let result = run();
    let mut times: Vec<_> = (0..7)
        .map(|_| {
            let start = Instant::now();
            black_box(run());
            start.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    times.sort_by(f64::total_cmp);
    (result, times[3])
}

fn assess(
    name: &str,
    real: &[f64],
    axis: &PpmAxis,
    mhz: f64,
    signed: bool,
    threshold: f64,
    truth: &[(f64, f64)],
) {
    let end = axis.ppm(real.len() as f64);
    let magnitude: Vec<_> = real.iter().map(|y| y.abs()).collect();
    let detection = if signed { &magnitude } else { real };
    let (peaks, detection_ms) =
        timed(|| analysis::find_peaks(detection, axis, axis.first, end, threshold, 3));
    let (fit, fitting_ms) =
        timed(|| deconvolution::deconvolve(real, axis, mhz, threshold, signed, None));
    let regions = analysis::integral_regions_from_peaks(
        detection,
        axis,
        mhz,
        axis.first,
        end,
        analysis::Reading {
            threshold,
            separation: 3,
            max_j_hz: 20.0,
        },
        &fit.peaks.iter().map(|p| p.ppm).collect::<Vec<_>>(),
    );
    let (_, sum_ms) = timed(|| {
        regions
            .iter()
            .map(|&(a, b)| analysis::integrate(real, axis, a, b))
            .collect::<Vec<_>>()
    });
    let (_, fitted_ms) = timed(|| {
        regions
            .iter()
            .map(|&(a, b)| fit.integrate(a, b))
            .collect::<Vec<_>>()
    });
    let unavailable = fit
        .regions
        .iter()
        .filter(|r| r.status != deconvolution::FitStatus::Converged)
        .count();
    println!(
        "{name}: points={} standard={} components={} unavailable_regions={} regions={} detection_ms={detection_ms:.4} fitting_ms={fitting_ms:.4} sum_ms={sum_ms:.4} fitted_ms={fitted_ms:.4}",
        real.len(),
        peaks.len(),
        fit.peaks.len(),
        unavailable,
        regions.len()
    );
    for &(ppm, area) in truth {
        if let Some(p) = fit
            .peaks
            .iter()
            .min_by(|a, b| (a.ppm - ppm).abs().total_cmp(&(b.ppm - ppm).abs()))
        {
            let accepted = fit
                .integrate(p.ppm - axis.delta.abs(), p.ppm + axis.delta.abs())
                .is_some();
            println!(
                "  target_ppm={ppm:.6} delta_ppm={:.8} relative_area_error={:.8} accepted={accepted}",
                p.ppm - ppm,
                (p.area / area - 1.0).abs()
            );
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let axis = PpmAxis {
        first: 10.0,
        delta: -0.001,
    };
    for (name, lines) in [
        (
            "isolated",
            vec![(500.2, 12.0, 1.0, 0.0), (1500.0, 18.0, 0.5, 0.7)],
        ),
        (
            "overlap",
            vec![(900.0, 16.0, 1.0, 0.0), (925.0, 12.0, 0.35, 0.4)],
        ),
        (
            "hidden_shoulder",
            vec![(900.0, 20.0, 1.0, 0.0), (915.0, 14.0, 0.25, 0.0)],
        ),
        (
            "signed",
            vec![(700.0, 15.0, -1.0, 0.3), (1400.0, 12.0, 0.5, 0.0)],
        ),
    ] {
        let real: Vec<_> = (0..2048)
            .map(|i| {
                lines
                    .iter()
                    .map(|&(at, w, h, q)| {
                        let u = (2.0 * (i as f64 - at) / w).powi(2);
                        h * ((1.0 - q) / (1.0 + u) + q * (1.0 + u / 2.0) / (1.0 + u + u * u))
                    })
                    .sum()
            })
            .collect();
        let truth: Vec<_> = lines
            .iter()
            .map(|&(at, w, h, q)| {
                (
                    axis.ppm(at),
                    h * w * axis.delta.abs() * PI / 2.0 * (1.0 - (1.0 - 3.0_f64.sqrt() / 2.0) * q),
                )
            })
            .collect();
        assess(name, &real, &axis, 400.0, name == "signed", 0.005, &truth);
    }
    for input in std::env::args().skip(1) {
        let path = Path::new(&input);
        let raw = if path.join("1r").is_file() {
            bruker::read_processed(path)?
        } else {
            bruker::read_fid(path)?
        };
        let spectrum = process::process(&raw, &process::auto_1d(&raw, true, true)?)?;
        let real: Vec<_> = spectrum.data.iter().map(|z| z.re).collect();
        assess(
            &input,
            &real,
            &spectrum.ppm_axis(),
            spectrum.meta.obs_mhz,
            spectrum.meta.signed,
            analysis::noise_level(&real) * 5.0,
            &[],
        );
    }
    Ok(())
}
