//! A first-order multiplet fitted to the spectrum by least squares.
//!
//! The lines follow from the shift, the class and the couplings, as
//! `simulateLines` draws them in the app: every coupling splits every line in two
//! and lines that land together add up. Each line is a Lorentzian of one width.
//! For any shape the height and a constant offset under it are solved exactly, so
//! only the shape's own parameters are searched for: the width alone, by golden
//! section, or the shift, the couplings and the width together, by Nelder-Mead.
//! Measuring each line at half its height read overlapping lines wrong and noisy
//! lines unsteadily; a fit uses every point of the signal.

use crate::PpmAxis;

/// Letters of a class and how often each splits the multiplet; the longer
/// spellings first, or `quint` reads as a `q`. As `splittingsOf` in the app.
const LETTERS: [(&str, usize); 10] = [
    ("quint", 4),
    ("sext", 5),
    ("sept", 6),
    ("oct", 7),
    ("non", 8),
    ("d", 1),
    ("t", 2),
    ("q", 3),
    ("s", 0),
    ("m", 0),
];

/// Narrowest and broadest width searched for, in Hz.
const WIDTH_RANGE: (f64, f64) = (0.1, 40.0);
const GOLDEN_STEPS: usize = 60;
const SIMPLEX_STEPS: usize = 600;

/// How often each coupling of `kind` splits it, or `None` for a class that is
/// not a first-order pattern (`m`, or anything unread).
pub fn splittings(kind: &str) -> Option<Vec<usize>> {
    let mut rest = kind.trim();
    let mut out = Vec::new();
    while !rest.is_empty() {
        let (spelling, times) = LETTERS.iter().find(|(s, _)| rest.starts_with(s))?;
        if *times > 0 {
            out.push(*times);
        }
        rest = &rest[spelling.len()..];
    }
    Some(out)
}

/// Lines of the multiplet as `(ppm, weight)`.
fn lines(ppm: f64, kind: &[usize], js_hz: &[f64], obs_mhz: f64) -> Vec<(f64, f64)> {
    let mut lines = vec![(ppm, 1.0)];
    for (i, &times) in kind.iter().enumerate() {
        let half = js_hz.get(i).copied().unwrap_or(0.0).abs() / 2.0 / obs_mhz;
        for _ in 0..times {
            let mut split: Vec<(f64, f64)> = Vec::with_capacity(lines.len() * 2);
            for &(at, weight) in &lines {
                for side in [-1.0, 1.0] {
                    let to = at + side * half;
                    match split.iter_mut().find(|(p, _)| (p - to).abs() < 1e-12) {
                        Some(same) => same.1 += weight / 2.0,
                        None => split.push((to, weight / 2.0)),
                    }
                }
            }
            lines = split;
        }
    }
    lines
}

/// The best fit the multiplet reaches, and how far from the spectrum it stays.
#[derive(Debug, Clone, PartialEq)]
pub struct Fit {
    pub ppm: f64,
    pub js_hz: Vec<f64>,
    /// Full width at half height of every line, in Hz.
    pub width_hz: f64,
    /// Sum of squared differences left over.
    pub residual: f64,
    /// Share of the range's variation the fitted lines account for, 0 to 1 (R²):
    /// how far to trust the class and couplings, read without knowing the noise.
    pub explained: f64,
}

/// What is searched for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Free {
    /// The width alone, the shift and couplings as given.
    Width,
    /// The shift, every coupling and the width together.
    All,
    /// Nothing: how well the shape as given fits.
    Nothing,
}

struct Problem<'a> {
    xs: Vec<f64>,
    ys: &'a [f64],
    kind: Vec<usize>,
    obs_mhz: f64,
}

impl Problem<'_> {
    /// Residual of the best height and offset for this shape. A shape that only
    /// fits upside down is no fit.
    fn residual(&self, ppm: f64, js_hz: &[f64], width_hz: f64) -> f64 {
        let half = (width_hz / 2.0 / self.obs_mhz).max(1e-9);
        let lines = lines(ppm, &self.kind, js_hz, self.obs_mhz);
        let shape: Vec<f64> = self
            .xs
            .iter()
            .map(|x| {
                lines
                    .iter()
                    .map(|(at, w)| w / (1.0 + ((x - at) / half).powi(2)))
                    .sum()
            })
            .collect();
        let n = shape.len() as f64;
        let (sf, sy) = (shape.iter().sum::<f64>(), self.ys.iter().sum::<f64>());
        let sff = shape.iter().map(|f| f * f).sum::<f64>();
        let sfy = shape.iter().zip(self.ys).map(|(f, y)| f * y).sum::<f64>();
        let det = n * sff - sf * sf;
        if det.abs() < f64::MIN_POSITIVE {
            return f64::INFINITY;
        }
        let height = (n * sfy - sf * sy) / det;
        if height <= 0.0 {
            return f64::INFINITY;
        }
        let offset = (sy - height * sf) / n;
        shape
            .iter()
            .zip(self.ys)
            .map(|(f, y)| (y - height * f - offset).powi(2))
            .sum()
    }
}

/// Fits the multiplet between `from_ppm` and `to_ppm` starting from `ppm`,
/// `kind`, `js_hz` and `width_hz`. `None` for a class that is not a first-order
/// pattern, or a range with too few points to say anything.
pub fn fit_multiplet(
    real: &[f64],
    axis: &PpmAxis,
    obs_mhz: f64,
    from_ppm: f64,
    to_ppm: f64,
    start: (f64, &str, &[f64], f64),
    free: Free,
) -> Option<Fit> {
    let (ppm, kind, js_hz, width_hz) = start;
    // An unresolved multiplet says nothing about where its lines are.
    if kind.trim() == "m" {
        return None;
    }
    let kind = splittings(kind)?;
    let [i, j] = [axis.index(from_ppm), axis.index(to_ppm)].map(|v| v.round());
    let (lo, hi) = (
        i.min(j).max(0.0) as usize,
        (i.max(j) as usize).min(real.len().checked_sub(1)?),
    );
    if hi <= lo + 4 || obs_mhz <= 0.0 {
        return None;
    }
    let problem = Problem {
        xs: (lo..=hi).map(|k| axis.ppm(k as f64)).collect(),
        ys: &real[lo..=hi],
        kind,
        obs_mhz,
    };
    let js: Vec<f64> = js_hz.iter().copied().take(problem.kind.len()).collect();
    let width = width_hz.clamp(WIDTH_RANGE.0, WIDTH_RANGE.1);
    let fit = match free {
        Free::Nothing => Fit {
            ppm,
            js_hz: js.clone(),
            width_hz: width,
            residual: problem.residual(ppm, &js, width),
            explained: 0.0,
        },
        Free::Width => {
            let width = golden(|w| problem.residual(ppm, &js, w), WIDTH_RANGE);
            Fit {
                ppm,
                js_hz: js.clone(),
                width_hz: width,
                residual: problem.residual(ppm, &js, width),
                explained: 0.0,
            }
        }
        Free::All => {
            // Searched in Hz from the start, and the width by its logarithm, so
            // every parameter moves in steps of about the same size.
            let mut start = vec![width.ln(), 0.0];
            start.extend(&js);
            let unpack = |p: &[f64]| (ppm + p[1] / obs_mhz, p[2..].to_vec(), p[0].exp());
            let best = simplex(
                |p| {
                    let (at, js, w) = unpack(p);
                    if !(WIDTH_RANGE.0..=WIDTH_RANGE.1).contains(&w) || js.iter().any(|j| *j <= 0.0)
                    {
                        return f64::INFINITY;
                    }
                    problem.residual(at, &js, w)
                },
                start,
            );
            let (at, js, w) = unpack(&best);
            Fit {
                ppm: at,
                js_hz: js.clone(),
                width_hz: w,
                residual: problem.residual(at, &js, w),
                explained: 0.0,
            }
        }
    };
    let mean = problem.ys.iter().sum::<f64>() / problem.ys.len() as f64;
    let total: f64 = problem.ys.iter().map(|y| (y - mean).powi(2)).sum();
    let explained = (1.0 - fit.residual / total).clamp(0.0, 1.0);
    (fit.residual.is_finite() && total > 0.0).then_some(Fit { explained, ..fit })
}

/// Minimum of `f` over `range` by golden-section search on the logarithm, since
/// a width is as likely to be off by a factor at 0.3 Hz as at 10.
fn golden(f: impl Fn(f64) -> f64, range: (f64, f64)) -> f64 {
    let ratio = (5f64.sqrt() - 1.0) / 2.0;
    let (mut a, mut b) = (range.0.ln(), range.1.ln());
    let mut c = b - ratio * (b - a);
    let mut d = a + ratio * (b - a);
    let (mut fc, mut fd) = (f(c.exp()), f(d.exp()));
    for _ in 0..GOLDEN_STEPS {
        if fc < fd {
            b = d;
            d = c;
            fd = fc;
            c = b - ratio * (b - a);
            fc = f(c.exp());
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + ratio * (b - a);
            fd = f(d.exp());
        }
    }
    ((a + b) / 2.0).exp()
}

/// Minimum of `f` by the Nelder-Mead simplex, from `start` with steps of about
/// one unit (0.3 in the width's logarithm, 0.5 Hz in shift and couplings).
fn simplex(f: impl Fn(&[f64]) -> f64, start: Vec<f64>) -> Vec<f64> {
    let n = start.len();
    let mut points: Vec<(Vec<f64>, f64)> = (0..=n)
        .map(|k| {
            let mut p = start.clone();
            if k > 0 {
                p[k - 1] += if k == 1 { 0.3 } else { 0.5 };
            }
            let v = f(&p);
            (p, v)
        })
        .collect();
    for _ in 0..SIMPLEX_STEPS {
        points.sort_by(|a, b| a.1.total_cmp(&b.1));
        let centroid: Vec<f64> = (0..n)
            .map(|i| points[..n].iter().map(|(p, _)| p[i]).sum::<f64>() / n as f64)
            .collect();
        let toward = |t: f64| -> Vec<f64> {
            centroid
                .iter()
                .zip(&points[n].0)
                .map(|(c, w)| c + t * (w - c))
                .collect()
        };
        let reflected = toward(-1.0);
        let fr = f(&reflected);
        if fr < points[0].1 {
            let expanded = toward(-2.0);
            let fe = f(&expanded);
            points[n] = if fe < fr {
                (expanded, fe)
            } else {
                (reflected, fr)
            };
        } else if fr < points[n - 1].1 {
            points[n] = (reflected, fr);
        } else {
            let contracted = toward(0.5);
            let fc = f(&contracted);
            if fc < points[n].1 {
                points[n] = (contracted, fc);
            } else {
                let best = points[0].0.clone();
                for (p, v) in points.iter_mut().skip(1) {
                    *p = best
                        .iter()
                        .zip(p.iter())
                        .map(|(b, x)| b + 0.5 * (x - b))
                        .collect();
                    *v = f(p);
                }
            }
        }
    }
    points.sort_by(|a, b| a.1.total_cmp(&b.1));
    points.swap_remove(0).0
}

#[cfg(test)]
mod tests {
    use super::*;

    const AXIS: PpmAxis = PpmAxis {
        first: 8.0,
        delta: -0.0005,
    };
    const MHZ: f64 = 400.0;

    /// A triplet with J = 7.2 Hz and lines 1.6 Hz wide at 7.300 ppm, with noise.
    fn triplet() -> Vec<f64> {
        let lines = lines(7.3, &[2], &[7.2], MHZ);
        let half = 1.6 / 2.0 / MHZ;
        let mut seed = 3u64;
        (0..4000)
            .map(|k| {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let noise = ((seed >> 33) as f64 / (1u64 << 31) as f64 - 0.5) * 0.02;
                let x = AXIS.ppm(k as f64);
                lines
                    .iter()
                    .map(|(at, w)| 100.0 * w / (1.0 + ((x - at) / half).powi(2)))
                    .sum::<f64>()
                    + noise
            })
            .collect()
    }

    #[test]
    fn classes_split_as_their_letters_say() {
        assert_eq!(splittings("dt"), Some(vec![1, 2]));
        assert_eq!(splittings("quint"), Some(vec![4]));
        assert_eq!(splittings("m"), Some(vec![]));
        assert_eq!(splittings("x"), None);
        let t = lines(0.0, &[2], &[10.0], 100.0);
        let mut weights: Vec<f64> = t.iter().map(|(_, w)| *w).collect();
        weights.sort_by(f64::total_cmp);
        assert_eq!(weights, vec![0.25, 0.25, 0.5]);
    }

    #[test]
    fn the_width_is_found_whatever_it_starts_from() {
        let real = triplet();
        for start in [0.3, 1.6, 6.0] {
            let fit = fit_multiplet(
                &real,
                &AXIS,
                MHZ,
                7.34,
                7.26,
                (7.3, "t", &[7.2], start),
                Free::Width,
            )
            .unwrap();
            assert!(
                (fit.width_hz - 1.6).abs() < 0.05,
                "from {start}: {}",
                fit.width_hz
            );
        }
    }

    #[test]
    fn shift_couplings_and_width_are_fitted_together() {
        let real = triplet();
        let fit = fit_multiplet(
            &real,
            &AXIS,
            MHZ,
            7.34,
            7.26,
            (7.302, "t", &[6.5], 1.0),
            Free::All,
        )
        .unwrap();
        assert!((fit.ppm - 7.3).abs() < 0.0005, "{}", fit.ppm);
        assert!((fit.js_hz[0] - 7.2).abs() < 0.05, "{:?}", fit.js_hz);
        assert!((fit.width_hz - 1.6).abs() < 0.05, "{}", fit.width_hz);
        assert!(fit.explained > 0.99, "{}", fit.explained);
    }

    #[test]
    fn a_wrong_class_explains_less() {
        let real = triplet();
        let share = |kind| {
            fit_multiplet(
                &real,
                &AXIS,
                MHZ,
                7.34,
                7.26,
                (7.3, kind, &[7.2], 1.6),
                Free::Nothing,
            )
            // A shape that only fits upside down is no fit at all.
            .map_or(0.0, |f| f.explained)
        };
        assert!(
            share("t") > 0.99 && share("d") < 0.8,
            "{} {}",
            share("t"),
            share("d")
        );
    }

    #[test]
    fn a_multiplet_that_is_not_first_order_is_not_fitted() {
        let real = triplet();
        for kind in ["m", "xyz"] {
            let fit = fit_multiplet(
                &real,
                &AXIS,
                MHZ,
                7.34,
                7.26,
                (7.3, kind, &[], 1.0),
                Free::Width,
            );
            assert!(fit.is_none(), "{kind}");
        }
    }
}
