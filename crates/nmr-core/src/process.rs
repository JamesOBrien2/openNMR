//! Processing operations. A processed spectrum is the raw data with an ordered
//! list of operations replayed on a copy, so the list is the whole history.
//!
//! Sign conventions follow nmrglue's `process.proc_base`.

use std::f64::consts::PI;

use rustfft::FftPlanner;
use serde::{Deserialize, Serialize};

use crate::{Complex64, Domain, Error, Metadata, Result, Spectrum, baseline, phase, predict};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    /// Mirrors the spectrum about the carrier by conjugating the FID, for
    /// datasets whose quadrature convention is the opposite of nmrglue's.
    Conjugate,
    /// Shifts the FID left by the digital filter group delay (nmrglue
    /// `rm_dig_filter`), cutting the points the filter spent arriving.
    RemoveDigitalFilter,
    /// Takes the digital filter's delay off as a first-order phase of 360° per
    /// point of delay applied with the Fourier transform, nothing cut. Put
    /// where [`Operation::RemoveDigitalFilter`] goes.
    DigitalFilter,
    /// Subtracts the mean of the last quarter of the FID.
    DcOffset,
    /// Exponential line broadening.
    Exponential {
        lb_hz: f64,
    },
    /// Lorentz-to-Gauss window with NMRPipe `GM` parameters: inverse exponential
    /// width `g1_hz`, Gaussian width `g2_hz`, maximum at fraction `g3`.
    Gaussian {
        g1_hz: f64,
        g2_hz: f64,
        g3: f64,
    },
    /// `sin(pi*offset + pi*(end - offset)*k/(n - 1))^power`; power 2 is the
    /// squared sine-bell.
    SineBell {
        offset: f64,
        end: f64,
        power: f64,
    },
    /// Pads with zeros or truncates to `size` points.
    ZeroFill {
        size: usize,
    },
    /// Carries the FID on to `size` points by forward linear prediction from
    /// the `order` points before each. See [`predict::extend`].
    LinearPrediction {
        size: usize,
        order: usize,
    },
    Ft,
    /// Phase in degrees; `pivot` is the fraction of the spectrum (0 = first
    /// point) where the first-order term is zero.
    Phase {
        p0: f64,
        p1: f64,
        pivot: f64,
    },
    Magnitude,
    Power,
    Reverse,
    /// Shifts the ppm axis so that `from_ppm` becomes `to_ppm`.
    Reference {
        from_ppm: f64,
        to_ppm: f64,
    },
    /// Subtracts an automatic Whittaker (airPLS) baseline from the real part;
    /// larger `lambda` gives a smoother baseline. See [`baseline::airpls`].
    BaselineAirpls {
        lambda: f64,
    },
    /// Fits a Whittaker baseline through recognised signal-free points.
    /// `lambda` uses OpenNMR's point-density scaling.
    /// `filter_hz` is the recognition width (a minimum when `auto_filter` is true).
    /// See [`baseline::cobas`].
    BaselineWhittaker {
        lambda: f64,
        #[serde(default = "default_filter_hz")]
        filter_hz: f64,
        /// Widen the filter to protect detected broad lines. Missing in older projects.
        #[serde(default = "default_auto_filter")]
        auto_filter: bool,
    },
    /// Subtracts an automatic polynomial baseline from the real part.
    /// See [`baseline::polynomial`].
    BaselinePolynomial {
        order: usize,
    },
    /// Subtracts a baseline drawn through points set by hand. Each point takes
    /// the mean of the real part over `span` points either side, unless it was
    /// placed freely;
    /// `through` says how they are joined, with `order` for a polynomial and
    /// `lambda` for the Whittaker smoother. See [`baseline::through_points`].
    BaselinePoints {
        points: Vec<BaselinePoint>,
        through: Through,
        order: usize,
        lambda: f64,
        span: usize,
    },
}

/// A point of a hand-set baseline. `at` is a fraction of the spectrum (0 =
/// first point), as the phase pivot is, so zero filling leaves it where it
/// was. `y`, for a point placed freely, is a fraction of the largest absolute
/// value of the real part before the baseline is taken off, so a change of
/// window or scale earlier in the list carries it along.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BaselinePoint {
    pub at: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Through {
    Segments,
    Spline,
    Polynomial,
    Whittaker,
}

/// A standard 1D pipeline: digital filter removal, 0.3 Hz ¹H line broadening,
/// no ¹³C window, 1 Hz for other nuclei, and FT. Zero filling is left
/// to the user, since it changes how many points every later step works on.
/// Spectra that are already in the frequency domain get no operations.
pub fn default_1d(raw: &Spectrum) -> Vec<Operation> {
    let mut ops = Vec::new();
    if raw.domain == Domain::Frequency {
        return ops;
    }
    if raw.meta.conjugate {
        ops.push(Operation::Conjugate);
    }
    if raw.meta.group_delay > 0.0 {
        ops.push(Operation::RemoveDigitalFilter);
    }
    match raw.meta.nucleus.as_str() {
        "1H" => ops.push(Operation::Exponential { lb_hz: 0.3 }),
        "13C" => {}
        _ => ops.push(Operation::Exponential { lb_hz: 1.0 }),
    }
    ops.push(Operation::Ft);
    if let Some([p0, p1]) = shifted_phase(&raw.meta) {
        ops.push(Operation::Phase { p0, p1, pivot: 0.0 });
    }
    ops
}

/// The recorded phase for a spectrum whose digital filter delay was shifted out
/// of the FID ([`Operation::RemoveDigitalFilter`]): the stored PHC0 leaves out
/// the half turn per point of delay the shift leaves at the first point, which
/// turned the old-firmware aspirin in `testdata` (61.02 points) upside down.
/// NMRium's file migration takes 180° off for an odd delay, the same rule
/// rounded.
pub fn shifted_phase(meta: &Metadata) -> Option<[f64; 2]> {
    meta.phase.map(|[p0, p1]| {
        let p0 = p0 + 180.0 * meta.group_delay;
        [(p0 + 180.0).rem_euclid(360.0) - 180.0, p1]
    })
}

/// The pipeline used when a document is opened: [`default_1d`] followed by
/// automatic phase and baseline correction, so a freshly opened FID already
/// shows absorptive peaks on a flat baseline. Both steps are ordinary
/// operations and can be edited or removed afterwards. Spectra that are
/// already processed are left alone.
pub fn auto_1d(raw: &Spectrum, phase: bool, baseline: bool) -> Result<Vec<Operation>> {
    let mut ops = recorded(raw, default_1d(raw));
    if ops.is_empty() {
        return Ok(ops);
    }
    // The digital filter comes off as a first-order phase, and the recorded
    // phase is then applied as it stands. Shifted out of the FID as nmrglue
    // does, a delay that is not a whole number of points bent the feet of every
    // strong line: on the aspirin in `testdata` 10 times further from the
    // stored spectrum, the strychnine 8 times.
    for op in &mut ops {
        match op {
            Operation::RemoveDigitalFilter => *op = Operation::DigitalFilter,
            Operation::Phase { p0, p1, .. } => {
                if let Some([r0, r1]) = raw.meta.phase {
                    (*p0, *p1) = (r0, r1);
                }
            }
            _ => {}
        }
    }
    // default_1d already carries the phase the spectrometer recorded, if any, and
    // that is applied whatever is asked for here: it is what the operator
    // decided and what the stored spectrum shows. `phase` asks only for a phase
    // to be worked out when the dataset carries none.
    if phase && !ops.iter().any(|op| matches!(op, Operation::Phase { .. })) {
        let (p0, p1) = phase::auto_phase(&process(raw, &ops)?.data, &raw.meta);
        ops.push(Operation::Phase { p0, p1, pivot: 0.0 });
    }
    // Find the signal-free regions before fitting: airPLS follows broad lines.
    if baseline {
        ops.push(Operation::BaselineWhittaker {
            lambda: WHITTAKER_LAMBDA,
            filter_hz: baseline::COBAS_FILTER_HZ,
            auto_filter: true,
        });
    }
    Ok(ops)
}

/// The smooth factor for the automatic baseline on the dataset in `testdata`;
/// see [`baseline::cobas`].
pub const WHITTAKER_LAMBDA: f64 = 1e5;

/// Projects written before the filter could be set used the automatic one.
fn default_filter_hz() -> f64 {
    baseline::COBAS_FILTER_HZ
}

fn default_auto_filter() -> bool {
    true
}

/// `ops` with the window and zero filling the dataset records: `LB` for the
/// exponential window, and `SI` points after zero filling. With no `SI` the FID
/// is zero filled to twice its length, rounded up to a power of two, the usual
/// default; without it the spectrum had half the recorded points on the dataset
/// in `testdata`, and positions and couplings were read that much more
/// coarsely.
fn recorded(raw: &Spectrum, mut ops: Vec<Operation>) -> Vec<Operation> {
    let Some(ft) = ops.iter().position(|op| *op == Operation::Ft) else {
        return ops;
    };
    if let Some(lb_hz) = raw.meta.line_broadening {
        ops.retain(|op| !matches!(op, Operation::Exponential { .. }));
        if lb_hz > 0.0 {
            let at = ops.iter().position(|op| *op == Operation::Ft).unwrap_or(ft);
            ops.insert(at, Operation::Exponential { lb_hz });
        }
    }
    let points = raw.data.len();
    let size = raw.meta.size.unwrap_or(points.next_power_of_two() * 2);
    // Equal too: removing the digital filter drops about GRPDLY points, so a
    // dataset whose SI is TD/2 came out that many points short of the recorded
    // size.
    if size >= points {
        let at = ops.iter().position(|op| *op == Operation::Ft).unwrap_or(ft);
        ops.insert(at, Operation::ZeroFill { size });
    }
    ops
}

/// Replays `ops` on a copy of `raw`.
pub fn process(raw: &Spectrum, ops: &[Operation]) -> Result<Spectrum> {
    let mut s = raw.clone();
    for op in ops {
        op.apply(&mut s)?;
    }
    Ok(s)
}

impl Operation {
    pub(crate) fn apply(&self, s: &mut Spectrum) -> Result<()> {
        let sw = s.meta.sw_hz;
        match *self {
            Self::RemoveDigitalFilter => {
                require(s, Domain::Time, "Digital filter removal")?;
                remove_digital_filter(&mut s.data, s.meta.group_delay);
                s.meta.group_delay = 0.0;
            }
            Self::DigitalFilter => {
                require(s, Domain::Time, "Digital filter removal")?;
                s.meta.delay_at_ft = s.meta.group_delay > 0.0;
            }
            Self::Conjugate => {
                require(s, Domain::Time, "Conjugation")?;
                for z in &mut s.data {
                    *z = z.conj();
                }
            }
            Self::DcOffset => {
                require(s, Domain::Time, "DC offset correction")?;
                let tail = &s.data[s.data.len() * 3 / 4..];
                let mean = tail.iter().sum::<Complex64>() / tail.len().max(1) as f64;
                s.data.iter_mut().for_each(|z| *z -= mean);
            }
            Self::Exponential { lb_hz } => {
                require(s, Domain::Time, "Exponential apodization")?;
                let lb = lb_hz / sw;
                let delay = delay_of(s);
                apodize(&mut s.data, delay, |k, _| (-PI * k * lb).exp());
            }
            Self::Gaussian { g1_hz, g2_hz, g3 } => {
                require(s, Domain::Time, "Gaussian apodization")?;
                let (g1, g2) = (g1_hz / sw, g2_hz / sw);
                let delay = delay_of(s);
                apodize(&mut s.data, delay, |k, n| {
                    let g = 0.6 * PI * g2 * (g3 * (n - 1.0) - k);
                    (PI * k * g1 - g * g).exp()
                });
            }
            Self::SineBell { offset, end, power } => {
                require(s, Domain::Time, "Sine-bell apodization")?;
                let delay = delay_of(s);
                apodize(&mut s.data, delay, |k, n| {
                    (PI * offset + PI * (end - offset) * k / (n - 1.0))
                        .sin()
                        .powf(power)
                });
            }
            Self::ZeroFill { size } => {
                require(s, Domain::Time, "Zero filling")?;
                s.data.resize(size, Complex64::ZERO);
            }
            Self::LinearPrediction { size, order } => {
                require(s, Domain::Time, "Linear prediction")?;
                predict::extend(&mut s.data, size, order);
            }
            Self::Ft => {
                require(s, Domain::Time, "The Fourier transform")?;
                ft(&mut s.data);
                s.domain = Domain::Frequency;
                if s.meta.delay_at_ft {
                    phase::apply(&mut s.data, 0.0, 360.0 * s.meta.group_delay, 0.0);
                    s.meta.group_delay = 0.0;
                    s.meta.delay_at_ft = false;
                }
            }
            Self::Phase { p0, p1, pivot } => {
                require(s, Domain::Frequency, "Phase correction")?;
                phase::apply(&mut s.data, p0, p1, pivot);
            }
            Self::Magnitude => {
                require(s, Domain::Frequency, "Magnitude calculation")?;
                s.data.iter_mut().for_each(|z| *z = z.norm().into());
            }
            Self::Power => {
                require(s, Domain::Frequency, "Power calculation")?;
                s.data.iter_mut().for_each(|z| *z = z.norm_sqr().into());
            }
            Self::Reverse => s.data.reverse(),
            Self::Reference { from_ppm, to_ppm } => {
                s.meta.carrier_hz += (to_ppm - from_ppm) * s.meta.obs_mhz;
            }
            Self::BaselineAirpls { lambda } => {
                require(s, Domain::Frequency, "Baseline correction")?;
                subtract_baseline(&mut s.data, |re| baseline::airpls(re, lambda));
            }
            Self::BaselineWhittaker {
                lambda,
                filter_hz,
                auto_filter,
            } => {
                require(s, Domain::Frequency, "Baseline correction")?;
                for (key, value) in [("Smooth Factor", lambda), ("Filter (Hz)", filter_hz)] {
                    if !value.is_finite() || value <= 0.0 {
                        return Err(Error::InvalidParameter {
                            key: key.into(),
                            value: value.to_string(),
                        });
                    }
                }
                let points = filter_hz * s.data.len() as f64 / s.meta.sw_hz;
                let axis = s.ppm_axis();
                subtract_baseline(&mut s.data, |re| {
                    let wide = if auto_filter {
                        baseline::BROAD_LINE_FILTER * baseline::signal_width(re, &axis)
                    } else {
                        0.0
                    };
                    let dilation = points
                        .max(wide)
                        .round()
                        .max(2.0)
                        .min(re.len().max(2) as f64) as usize;
                    baseline::cobas(re, lambda, dilation)
                });
            }
            Self::BaselinePolynomial { order } => {
                require(s, Domain::Frequency, "Baseline correction")?;
                subtract_baseline(&mut s.data, |re| baseline::polynomial(re, order));
            }
            Self::BaselinePoints { .. } => {
                require(s, Domain::Frequency, "Baseline correction")?;
                subtract_baseline(&mut s.data, |re| {
                    hand_set(re, self).map_or_else(|| vec![0.0; re.len()], |(_, curve)| curve)
                });
            }
        }
        Ok(())
    }
}

fn require(s: &Spectrum, domain: Domain, operation: &'static str) -> Result<()> {
    if s.domain == domain {
        Ok(())
    } else {
        Err(Error::Order {
            operation,
            before: domain == Domain::Time,
        })
    }
}

/// A hand-set baseline: its points as `(index, value)`, and the curve.
pub type HandSet = (Vec<(f64, f64)>, Vec<f64>);

/// The points of a hand-set baseline on `real`, the real part it is taken
/// from, as `(index, value)`, and the baseline through them; `None` for any
/// other operation. The plot draws both while the points are being set.
pub fn hand_set(real: &[f64], op: &Operation) -> Option<HandSet> {
    let Operation::BaselinePoints {
        points,
        through,
        order,
        lambda,
        span,
    } = op
    else {
        return None;
    };
    let n = real.len();
    let top = real.iter().fold(0.0, |m: f64, v| m.max(v.abs()));
    let points: Vec<(usize, Option<f64>)> = points
        .iter()
        .map(|p| (index_at(p.at, n), p.y.map(|y| y * top)))
        .collect();
    let anchors = baseline::anchors(real, &points, *span);
    let through = match through {
        Through::Segments => baseline::Through::Segments,
        Through::Spline => baseline::Through::Spline,
        Through::Polynomial => baseline::Through::Polynomial(*order),
        Through::Whittaker => baseline::Through::Whittaker(*lambda),
    };
    let curve = baseline::through_points(n, &anchors, through);
    Some((anchors, curve))
}

/// The point a fraction of the spectrum falls on, as the pivot is placed.
pub fn index_at(at: f64, n: usize) -> usize {
    ((at * n as f64).round().max(0.0) as usize).min(n.saturating_sub(1))
}

/// Baselines are fitted to and removed from the real part only, so the imaginary
/// part stays available for later phasing.
fn subtract_baseline(data: &mut [Complex64], fit: impl Fn(&[f64]) -> Vec<f64>) {
    let real: Vec<f64> = data.iter().map(|z| z.re).collect();
    for (z, b) in data.iter_mut().zip(fit(&real)) {
        z.re -= b;
    }
}

/// Where a window starts: with the digital filter's delay still in the FID
/// ([`Operation::DigitalFilter`]), at the end of it, where the signal starts.
/// The stored 2D spectra say so: started at the first point, the sine bell put
/// the COSY in `testdata` at 0.989 against its stored 2rr, at the end of the
/// delay 0.998, and the HSQC from 0.992 to 0.994. An exponential only changes
/// scale.
fn delay_of(s: &Spectrum) -> f64 {
    if s.meta.delay_at_ft {
        s.meta.group_delay
    } else {
        0.0
    }
}

/// `window(k, n)` over the points from `delay` on; the points before it take
/// the window's first value.
fn apodize(data: &mut [Complex64], delay: f64, window: impl Fn(f64, f64) -> f64) {
    let n = data.len() as f64 - delay;
    for (k, z) in data.iter_mut().enumerate() {
        *z *= window((k as f64 - delay).max(0.0), n);
    }
}

/// nmrglue `proc_base.fft`: unnormalised forward FFT, then `fftshift`.
fn ft(data: &mut [Complex64]) {
    let n = data.len();
    FftPlanner::new().plan_fft_forward(n).process(data);
    data.rotate_right(n / 2);
}

/// nmrglue `bruker.rm_dig_filter(truncate_grpdly=False)`: Fourier shift by the
/// exact group delay (`proc_base.fsh2`), then fold the wrapped filter points from
/// the end onto the start and drop them. A zero delay leaves the data untouched,
/// where nmrglue would still drop two points.
fn remove_digital_filter(data: &mut Vec<Complex64>, delay: f64) {
    let n = data.len();
    if delay <= 0.0 || n == 0 {
        return;
    }
    let mut planner = FftPlanner::new();
    data.rotate_left(n / 2);
    planner.plan_fft_forward(n).process(data);
    let scale = 1.0 / n as f64;
    for (k, z) in data.iter_mut().enumerate() {
        *z *= Complex64::from_polar(scale, 2.0 * PI * delay * k as f64 / n as f64);
    }
    planner.plan_fft_inverse(n).process(data);
    data.rotate_right(n / 2);

    let skip = ((delay + 2.0).floor() as usize).min(n);
    let add = skip.saturating_sub(6);
    for i in 0..add {
        let wrapped = data[n - 1 - i];
        data[i] += wrapped;
    }
    data.truncate(n - skip);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Metadata;

    fn fid(data: Vec<Complex64>) -> Spectrum {
        Spectrum {
            data,
            meta: Metadata {
                nucleus: "1H".into(),
                sw_hz: 1000.0,
                obs_mhz: 100.0,
                carrier_hz: 0.0,
                group_delay: 0.0,
                phase: None,
                solvent: None,
                conjugate: false,
                signed: false,
                size: None,
                line_broadening: None,
                delay_at_ft: false,
            },
            domain: Domain::Time,
        }
    }

    fn close(a: Complex64, b: Complex64) -> bool {
        (a - b).norm() < 1e-9
    }

    #[test]
    fn ft_puts_positive_frequency_left_of_centre() {
        let n = 64;
        let raw = fid((0..n)
            .map(|k| Complex64::from_polar(1.0, 2.0 * PI * 8.0 * k as f64 / n as f64))
            .collect());
        let s = process(&raw, &[Operation::Ft]).unwrap();
        let peak = (0..n)
            .max_by(|&a, &b| s.data[a].norm().total_cmp(&s.data[b].norm()))
            .unwrap();
        // fftshift moves bin +8 to index 32 + 8; ppm decreases with index.
        assert_eq!(peak, 40);
        assert!(close(s.data[peak], Complex64::new(n as f64, 0.0)));
    }

    #[test]
    fn integer_group_delay_is_circular_shift_then_trim() {
        let data: Vec<Complex64> = (0..32)
            .map(|k| Complex64::new(k as f64, -(k as f64)))
            .collect();
        let mut raw = fid(data.clone());
        raw.meta.group_delay = 3.0;
        let s = process(&raw, &[Operation::RemoveDigitalFilter]).unwrap();
        assert_eq!(s.data.len(), 32 - 5);
        for (k, z) in s.data.iter().enumerate() {
            assert!(close(*z, data[k + 3]));
        }
        assert_eq!(s.meta.group_delay, 0.0);
    }

    #[test]
    fn large_group_delay_folds_end_onto_start() {
        let data: Vec<Complex64> = (0..128).map(|k| Complex64::new(k as f64, 0.0)).collect();
        let mut raw = fid(data.clone());
        raw.meta.group_delay = 10.0;
        let s = process(&raw, &[Operation::RemoveDigitalFilter]).unwrap();
        // skip = 12, add = 6: the last 6 shifted points (reversed) are added to the first 6.
        assert_eq!(s.data.len(), 116);
        let shifted = |k: usize| data[(k + 10) % 128];
        assert!(close(s.data[0], shifted(0) + shifted(127)));
        assert!(close(s.data[5], shifted(5) + shifted(122)));
        assert!(close(s.data[6], shifted(6)));
    }

    #[test]
    fn phase_pivot_keeps_pivot_point_unchanged() {
        let mut raw = fid(vec![Complex64::new(1.0, 0.0); 100]);
        raw.domain = Domain::Frequency;
        let s = process(
            &raw,
            &[Operation::Phase {
                p0: 0.0,
                p1: 90.0,
                pivot: 0.25,
            }],
        )
        .unwrap();
        assert!(close(s.data[25], Complex64::new(1.0, 0.0)));
        assert!(close(
            s.data[75],
            Complex64::from_polar(1.0, 45f64.to_radians())
        ));
    }

    #[test]
    fn exponential_uses_hz() {
        let raw = fid(vec![Complex64::new(1.0, 0.0); 10]);
        let s = process(&raw, &[Operation::Exponential { lb_hz: 10.0 }]).unwrap();
        assert!((s.data[5].re - (-PI * 5.0 * 0.01).exp()).abs() < 1e-12);
    }

    #[test]
    fn sine_bell_and_zero_fill() {
        let raw = fid(vec![Complex64::new(1.0, 0.0); 5]);
        let ops = [
            Operation::SineBell {
                offset: 0.5,
                end: 1.0,
                power: 2.0,
            },
            Operation::ZeroFill { size: 8 },
        ];
        let s = process(&raw, &ops).unwrap();
        assert!(close(s.data[0], Complex64::new(1.0, 0.0)));
        assert!(s.data[4].norm() < 1e-12);
        assert_eq!(s.data.len(), 8);
    }

    #[test]
    fn dc_offset_removes_tail_mean() {
        let raw = fid(vec![Complex64::new(3.0, -1.0); 16]);
        let s = process(&raw, &[Operation::DcOffset]).unwrap();
        assert!(s.data.iter().all(|z| z.norm() < 1e-12));
    }

    #[test]
    fn reference_moves_axis() {
        let raw = fid(vec![Complex64::ZERO; 16]);
        let before = raw.ppm_axis().ppm(3.0);
        let s = process(
            &raw,
            &[Operation::Reference {
                from_ppm: before,
                to_ppm: 0.0,
            }],
        )
        .unwrap();
        assert!(s.ppm_axis().ppm(3.0).abs() < 1e-12);
    }

    #[test]
    fn default_pipeline_ends_in_frequency_domain() {
        let mut raw = fid(vec![Complex64::new(1.0, 0.0); 1000]);
        raw.meta.group_delay = 67.98;
        let ops = default_1d(&raw);
        assert_eq!(ops[0], Operation::RemoveDigitalFilter);
        assert!(
            !ops.iter()
                .any(|op| matches!(op, Operation::ZeroFill { .. }))
        );
        // Nothing pads the FID any more, so the transform keeps exactly what the
        // digital filter removal left behind.
        let filtered = process(&raw, &ops[..1]).unwrap();
        let s = process(&raw, &ops).unwrap();
        assert_eq!(
            (s.domain, s.data.len()),
            (Domain::Frequency, filtered.data.len())
        );
    }

    #[test]
    fn conjugation_mirrors_the_spectrum() {
        let n = 64;
        let mut raw = fid((0..n)
            .map(|k| Complex64::from_polar(1.0, 2.0 * PI * 8.0 * f64::from(k) / f64::from(n)))
            .collect());
        let peak = |s: &Spectrum| {
            s.data
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.norm().total_cmp(&b.1.norm()))
                .unwrap()
                .0
        };
        let plain = peak(&process(&raw, &[Operation::Ft]).unwrap());
        let mirrored = peak(&process(&raw, &[Operation::Conjugate, Operation::Ft]).unwrap());
        assert_eq!(mirrored, n as usize - plain, "mirrored about the centre");

        raw.meta.conjugate = true;
        assert_eq!(default_1d(&raw)[0], Operation::Conjugate);
    }

    #[test]
    fn auto_1d_zero_fills_and_broadens_as_recorded() {
        let mut raw = fid(vec![Complex64::new(1.0, 0.0); 1000]);
        let zero_fill = |ops: &[Operation]| {
            ops.iter().find_map(|op| match op {
                Operation::ZeroFill { size } => Some(*size),
                _ => None,
            })
        };
        let lb = |ops: &[Operation]| {
            ops.iter().find_map(|op| match op {
                Operation::Exponential { lb_hz } => Some(*lb_hz),
                _ => None,
            })
        };
        // Nothing recorded: twice the length, rounded up to a power of two.
        let ops = auto_1d(&raw, false, false).unwrap();
        assert_eq!(zero_fill(&ops), Some(2048));
        assert_eq!(lb(&ops), Some(0.3));

        raw.meta.nucleus = "13C".into();
        assert_eq!(lb(&auto_1d(&raw, false, false).unwrap()), None);

        raw.meta.size = Some(4096);
        raw.meta.line_broadening = Some(1.5);
        let ops = auto_1d(&raw, false, false).unwrap();
        assert_eq!(zero_fill(&ops), Some(4096));
        assert_eq!(lb(&ops), Some(1.5));
        let ft = ops.iter().position(|op| *op == Operation::Ft).unwrap();
        assert!(matches!(ops[ft - 1], Operation::ZeroFill { .. }));

        // No window recorded means none here either.
        raw.meta.line_broadening = Some(0.0);
        assert_eq!(lb(&auto_1d(&raw, false, false).unwrap()), None);
    }

    #[test]
    fn auto_1d_phases_the_spectrum() {
        // A decaying signal 60 degrees out of phase, with some noise as every
        // real one has; after opening, its peak has to point up.
        let n = 512;
        let raw = fid((0..n)
            .map(|k| {
                let t = f64::from(k) / 1000.0;
                let noise = f64::from((k * 7919) % 997) / 997.0 - 0.5;
                Complex64::from_polar((-t * 20.0).exp(), 60f64.to_radians() + 2.0 * PI * 100.0 * t)
                    + Complex64::new(noise, -noise) * 0.01
            })
            .collect());
        let ops = auto_1d(&raw, true, true).unwrap();
        assert!(matches!(ops[ops.len() - 2], Operation::Phase { .. }));
        let s = process(&raw, &ops).unwrap();
        let max = s.data.iter().map(|z| z.re).fold(f64::MIN, f64::max);
        let min = s.data.iter().map(|z| z.re).fold(f64::MAX, f64::min);
        assert!(
            max > 10.0 * min.abs(),
            "not absorptive: max {max}, min {min}"
        );
    }

    #[test]
    fn magnitude_power_and_reverse() {
        let mut raw = fid(vec![Complex64::new(3.0, 4.0), Complex64::new(0.0, -2.0)]);
        raw.domain = Domain::Frequency;
        let m = process(&raw, &[Operation::Magnitude]).unwrap();
        assert_eq!(
            m.data,
            vec![Complex64::new(5.0, 0.0), Complex64::new(2.0, 0.0)]
        );
        let p = process(&raw, &[Operation::Power, Operation::Reverse]).unwrap();
        assert_eq!(
            p.data,
            vec![Complex64::new(4.0, 0.0), Complex64::new(25.0, 0.0)]
        );
    }

    #[test]
    fn baseline_keeps_imaginary_part() {
        let mut raw = fid(vec![Complex64::new(5.0, 2.0); 64]);
        raw.domain = Domain::Frequency;
        let s = process(&raw, &[Operation::BaselinePolynomial { order: 1 }]).unwrap();
        assert!(s.data.iter().all(|z| z.re.abs() < 1e-9 && z.im == 2.0));
        let time = fid(vec![Complex64::ZERO; 8]);
        let err = process(&time, &[Operation::BaselineAirpls { lambda: 1e5 }]);
        assert!(matches!(err, Err(Error::Order { before: false, .. })));
    }

    #[test]
    fn whittaker_preserves_weaker_broad_signed_lines() {
        let n = 16_384;
        for sign in [-1.0, 1.0] {
            let signal: Vec<f64> = (0..n)
                .map(|i| {
                    1000.0 / (1.0 + ((i as f64 - 4000.0) / 3.0).powi(2))
                        + sign * 100.0 / (1.0 + ((i as f64 - 10_000.0) / 50.0).powi(2))
                })
                .collect();
            let mut raw = fid((0..n)
                .map(|i| {
                    Complex64::new(
                        10.0 + signal[i] + ((i * 7919 % 1000) as f64 / 500.0 - 1.0),
                        2.0,
                    )
                })
                .collect());
            raw.domain = Domain::Frequency;
            raw.meta.sw_hz = n as f64;
            raw.meta.obs_mhz = 1024.0;
            raw.meta.signed = true;
            let corrected = process(
                &raw,
                &[Operation::BaselineWhittaker {
                    lambda: 1e5,
                    filter_hz: baseline::COBAS_FILTER_HZ,
                    auto_filter: true,
                }],
            )
            .unwrap();
            let region = 8500..11_500;
            let truth: f64 = signal[region.clone()].iter().sum();
            let measured: f64 = corrected.data[region].iter().map(|v| v.re).sum();
            let area_error = ((measured - truth) / truth).abs();
            assert!(area_error < 0.05, "sign {sign}: area error {area_error}");
            for quiet in [1000..2000, 13_000..14_000] {
                let rms = (quiet
                    .clone()
                    .map(|i| (raw.data[i].re - corrected.data[i].re - 10.0).powi(2))
                    .sum::<f64>()
                    / quiet.len() as f64)
                    .sqrt();
                assert!(rms < 0.1, "sign {sign}: quiet baseline RMS {rms}");
            }
            assert!(corrected.data.iter().all(|z| z.im == 2.0));
        }
    }

    #[test]
    fn manual_whittaker_filter_is_not_widened_by_broad_signals() {
        let n = 16_384;
        let mut raw = fid((0..n)
            .map(|i| {
                let broad = 100.0 / (1.0 + ((i as f64 - 8000.0) / 50.0).powi(2));
                let noise = (i * 7919 % 1000) as f64 / 500.0 - 1.0;
                Complex64::new(10.0 + broad + noise, 2.0)
            })
            .collect());
        raw.domain = Domain::Frequency;
        raw.meta.sw_hz = n as f64;
        let real: Vec<f64> = raw.data.iter().map(|z| z.re).collect();
        let op = |auto_filter| Operation::BaselineWhittaker {
            lambda: 1e5,
            filter_hz: 7.3,
            auto_filter,
        };
        let manual = process(&raw, &[op(false)]).unwrap();
        let expected = baseline::cobas(&real, 1e5, 7);
        for ((source, corrected), baseline) in raw.data.iter().zip(&manual.data).zip(expected) {
            assert!((source.re - corrected.re - baseline).abs() < 1e-10);
            assert_eq!(corrected.im, source.im);
        }
        let automatic = process(&raw, &[op(true)]).unwrap();
        assert!((automatic.data[8000].re - manual.data[8000].re).abs() > 1.0);
        let different_filter = process(
            &raw,
            &[Operation::BaselineWhittaker {
                lambda: 1e5,
                filter_hz: 100.0,
                auto_filter: false,
            }],
        )
        .unwrap();
        assert!((different_filter.data[8000].re - manual.data[8000].re).abs() > 1.0);
        let different_factor = process(
            &raw,
            &[Operation::BaselineWhittaker {
                lambda: 1e3,
                filter_hz: 7.3,
                auto_filter: false,
            }],
        )
        .unwrap();
        assert!(
            different_factor
                .data
                .iter()
                .zip(&manual.data)
                .any(|(a, b)| (a.re - b.re).abs() > 0.01)
        );
    }

    #[test]
    fn whittaker_rejects_invalid_tuning_values() {
        let mut raw = fid(vec![Complex64::new(10.0, 2.0); 64]);
        raw.domain = Domain::Frequency;
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            for (lambda, filter_hz) in [(bad, 7.3), (1e5, bad)] {
                assert!(matches!(
                    process(
                        &raw,
                        &[Operation::BaselineWhittaker {
                            lambda,
                            filter_hz,
                            auto_filter: false,
                        }]
                    ),
                    Err(Error::InvalidParameter { .. })
                ));
            }
        }
    }

    #[test]
    fn whittaker_preserves_edge_lines_and_removes_noise_offsets() {
        let n = 16_384;
        let at = n * 99 / 100;
        for height in [-1000.0, 0.0, 1000.0] {
            let mut raw = fid((0..n)
                .map(|i| {
                    let line = height / (1.0 + ((i as f64 - at as f64) / 3.0).powi(2));
                    let noise = (i * 7919 % 1000) as f64 / 500.0 - 1.0;
                    Complex64::new(10.0 + line + noise, 0.0)
                })
                .collect());
            raw.domain = Domain::Frequency;
            raw.meta.sw_hz = n as f64;
            raw.meta.obs_mhz = 1024.0;
            let corrected = process(
                &raw,
                &[Operation::BaselineWhittaker {
                    lambda: 1e5,
                    filter_hz: baseline::COBAS_FILTER_HZ,
                    auto_filter: true,
                }],
            )
            .unwrap();
            assert!(
                (corrected.data[at].re - height).abs() < 1.0 + 0.005 * height.abs(),
                "height {height}: corrected {}",
                corrected.data[at].re
            );
            let rms = ((1000..2000)
                .map(|i| (raw.data[i].re - corrected.data[i].re - 10.0).powi(2))
                .sum::<f64>()
                / 1000.0)
                .sqrt();
            assert!(rms < 0.1, "height {height}: quiet baseline RMS {rms}");
        }
    }

    #[test]
    fn hand_set_baseline_keeps_its_points_through_zero_filling() {
        let op = |points: Vec<BaselinePoint>| Operation::BaselinePoints {
            points,
            through: Through::Segments,
            order: 3,
            lambda: 1e5,
            span: 0,
        };
        let at = |at| BaselinePoint { at, y: None };
        // A ramp: segments through two of its points take all of it off, at
        // either size, since the points are fractions of the spectrum.
        for n in [64, 256] {
            let ramp = (0..n)
                .map(|k| Complex64::new(k as f64 / n as f64, 1.0))
                .collect();
            let mut raw = fid(ramp);
            raw.domain = Domain::Frequency;
            let s = process(&raw, &[op(vec![at(0.25), at(0.75)])]).unwrap();
            let middle = &s.data[n / 4..3 * n / 4];
            assert!(
                middle.iter().all(|z| z.re.abs() < 1e-9 && z.im == 1.0),
                "{n}"
            );
        }
        // A point placed freely stands at its fraction of the largest value.
        let mut raw = fid(vec![Complex64::new(2.0, 0.0); 16]);
        raw.domain = Domain::Frequency;
        let free = op(vec![BaselinePoint {
            at: 0.5,
            y: Some(0.25),
        }]);
        let s = process(&raw, &[free]).unwrap();
        assert!(s.data.iter().all(|z| (z.re - 1.5).abs() < 1e-12));
    }

    #[test]
    fn wrong_order_is_an_error() {
        let raw = fid(vec![Complex64::ZERO; 16]);
        let err = process(
            &raw,
            &[Operation::Ft, Operation::Exponential { lb_hz: 1.0 }],
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Exponential apodization must come before the Fourier transform"
        );
    }
}
