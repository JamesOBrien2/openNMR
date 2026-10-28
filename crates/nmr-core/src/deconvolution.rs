//! Bounded 1D generalized-Lorentzian (GL) deconvolution.
//!
//! The line shape is a generalized Lorentzian with a kurtosis parameter, the
//! model used for high-precision qNMR by Schoenberger et al., *Analytical
//! Chemistry* 88 (2016) 3836 (DOI 10.1021/acs.analchem.5b04911), and for
//! multiplet deconvolution by Jeannerat & Cobas, *Magnetic Resonance* 2 (2021)
//! 545 (DOI 10.5194/mr-2-545-2021). The area is its analytic integral.
//! Detection, optimization and model selection are local choices.

use crate::{PpmAxis, analysis};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

const MAX_COMPONENTS: usize = 8;
// The aspirin fixture has 134 standard candidates; cap work without rejecting it outright.
const MAX_TOTAL: usize = 512;
const MAX_SAMPLES: usize = 2048;
const ITERATIONS: usize = 140;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FittedPeak {
    pub ppm: f64,
    pub height: f64,
    pub width_hz: f64,
    pub area: f64,
    pub shape: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FitStatus {
    Converged,
    Unresolved,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FitReason {
    InvalidInput,
    BelowThreshold,
    ComponentLimit,
    InsufficientSamples,
    Optimizer,
    Degenerate,
    Residual,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FitRegion {
    pub from: f64,
    pub to: f64,
    pub status: FitStatus,
    pub rms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<FitReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PeakFit {
    #[serde(default)]
    pub source: String,
    pub peaks: Vec<FittedPeak>,
    pub regions: Vec<FitRegion>,
}

impl PeakFit {
    /// Full component areas with centres in [low, high), independent of tails.
    /// A failed/ambiguous intersecting fit makes the measurement unavailable.
    pub fn integrate(&self, from: f64, to: f64) -> Option<f64> {
        if !from.is_finite() || !to.is_finite() || from == to || !self.valid() {
            return None;
        }
        let (low, high) = (from.min(to), from.max(to));
        let intersects = |r: &&FitRegion| r.from.max(r.to) > low && r.from.min(r.to) < high;
        let mut regions = self.regions.iter().filter(intersects).peekable();
        if regions.peek().is_none() || regions.any(|r| r.status != FitStatus::Converged) {
            return None;
        }
        let peaks: Vec<_> = self
            .peaks
            .iter()
            .filter(|p| p.ppm >= low && p.ppm < high)
            .collect();
        if peaks.is_empty()
            || peaks.iter().any(|p| {
                !self.regions.iter().any(|r| {
                    r.status == FitStatus::Converged
                        && p.ppm >= r.from.min(r.to)
                        && p.ppm <= r.from.max(r.to)
                })
            })
        {
            return None;
        }
        let area: f64 = peaks.iter().map(|p| p.area).sum();
        area.is_finite().then_some(area)
    }

    /// Validate restored models before making measurements from their fields.
    pub fn valid(&self) -> bool {
        self.peaks.len() <= MAX_TOTAL
            && self.regions.len() <= MAX_TOTAL
            && self.peaks.iter().all(|p| {
                p.ppm.is_finite()
                    && p.height.is_finite()
                    && p.height != 0.0
                    && p.width_hz.is_finite()
                    && p.width_hz > 0.0
                    && p.area.is_finite()
                    && p.shape.is_finite()
                    && (-1.0..=2.0).contains(&p.shape)
            })
            && self.regions.iter().all(|r| {
                r.from.is_finite()
                    && r.to.is_finite()
                    && r.from != r.to
                    && r.rms.is_none_or(|v| v.is_finite() && v >= 0.0)
                    && (r.status != FitStatus::Converged || r.rms.is_some())
            })
    }

    pub fn fits_source(&self, real: &[f64], axis: &PpmAxis) -> bool {
        self.valid() && self.source == fingerprint(real, axis)
    }

    /// Validate cached/project models against the actual spectrum and physical units.
    pub fn valid_for(&self, real: &[f64], axis: &PpmAxis, obs_mhz: f64) -> bool {
        if !self.fits_source(real, axis)
            || real.is_empty()
            || !axis.first.is_finite()
            || !axis.delta.is_finite()
            || axis.delta == 0.0
            || !obs_mhz.is_finite()
            || obs_mhz <= 0.0
            || real.iter().any(|v| !v.is_finite())
        {
            return false;
        }
        let end = axis.ppm((real.len() - 1) as f64);
        let (low, high) = (axis.first.min(end), axis.first.max(end));
        self.regions.iter().all(|r| {
            r.from >= low
                && r.from <= high
                && r.to >= low
                && r.to <= high
                && (r.status != FitStatus::Converged || r.rms.is_some())
        }) && self.peaks.iter().all(|p| {
            let expected = p.height * (p.width_hz / obs_mhz) * PI / 2.0
                * (1.0 - (1.0 - 3.0_f64.sqrt() / 2.0) * p.shape);
            p.ppm >= low
                && p.ppm <= high
                && p.width_hz <= (high - low) * obs_mhz
                && expected.is_finite()
                && (p.area - expected).abs() <= expected.abs().max(1e-12) * 1e-8
                && self
                    .regions
                    .iter()
                    .any(|r| p.ppm >= r.from.min(r.to) && p.ppm <= r.from.max(r.to))
        })
    }
}

/// Deterministic FNV-1a identity, including the axis so referencing invalidates a fit.
/// This detects stale local models; it is not a cryptographic authenticity check.
pub fn fingerprint(real: &[f64], axis: &PpmAxis) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for value in [axis.first, axis.delta].iter().chain(real) {
        for byte in value.to_bits().to_le_bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    format!("gl1-{hash:016x}")
}

fn shape(x: f64, center: f64, width: f64, q: f64) -> f64 {
    let s = (2.0 * (x - center) / width).powi(2);
    (1.0 - q) / (1.0 + s) + q * (1.0 + 0.5 * s) / (1.0 + s + s * s)
}

// Parameters are centre in points, log(width in points), normalized height, shape.
fn model(x: f64, p: &[f64]) -> f64 {
    p.as_chunks::<4>()
        .0
        .iter()
        .map(|p| p[2] * shape(x, p[0], p[1].exp(), p[3]))
        .sum()
}

fn objective(xs: &[f64], ys: &[f64], p: &[f64]) -> f64 {
    let offset = p[p.len() - 1];
    xs.iter()
        .zip(ys)
        .map(|(&x, &y)| (y - offset - model(x, p)).powi(2))
        .sum()
}

fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for i in 0..n {
        let pivot = (i..n).max_by(|&j, &k| a[j][i].abs().total_cmp(&a[k][i].abs()))?;
        if a[pivot][i].abs() < 1e-14 || !a[pivot][i].is_finite() {
            return None;
        }
        a.swap(i, pivot);
        b.swap(i, pivot);
        let (prior, rows) = a.split_at_mut(i + 1);
        let pivot = &prior[i];
        for (j, row) in rows.iter_mut().enumerate() {
            let ratio = row[i] / pivot[i];
            for (value, coefficient) in row[i..].iter_mut().zip(&pivot[i..]) {
                *value -= ratio * coefficient;
            }
            b[i + 1 + j] -= ratio * b[i];
        }
    }
    let mut out = vec![0.0; n];
    for i in (0..n).rev() {
        out[i] = (b[i] - (i + 1..n).map(|j| a[i][j] * out[j]).sum::<f64>()) / a[i][i];
    }
    out.iter().all(|v| v.is_finite()).then_some(out)
}

fn refine(xs: &[f64], ys: &[f64], p: &mut Vec<f64>, signed: bool) -> bool {
    let mut error = objective(xs, ys, p);
    let mut damping = 1e-3;
    let n = p.len();
    let (low, high) = (xs[0], xs[xs.len() - 1]);
    for _ in 0..ITERATIONS {
        let mut a = vec![vec![0.0; n]; n];
        let mut b = vec![0.0; n];
        let mut jac = vec![0.0; n];
        for (&x, &y) in xs.iter().zip(ys) {
            for (i, c) in p.as_chunks::<4>().0.iter().enumerate() {
                let width = c[1].exp();
                let t = 2.0 * (x - c[0]) / width;
                let s = t * t;
                let lorentz = 1.0 / (1.0 + s);
                let denom = 1.0 + s + s * s;
                let general = (1.0 + 0.5 * s) / denom;
                let derivative = -(1.0 - c[3]) * lorentz * lorentz
                    + c[3] * (0.5 * denom - (1.0 + 0.5 * s) * (1.0 + 2.0 * s)) / denom.powi(2);
                jac[4 * i] = c[2] * derivative * -4.0 * t / width;
                jac[4 * i + 1] = c[2] * derivative * -2.0 * s;
                jac[4 * i + 2] = (1.0 - c[3]) * lorentz + c[3] * general;
                jac[4 * i + 3] = c[2] * (general - lorentz);
            }
            jac[n - 1] = 1.0;
            let residual = y - p[n - 1] - model(x, p);
            for i in 0..n {
                b[i] += jac[i] * residual;
                for j in 0..=i {
                    a[i][j] += jac[i] * jac[j];
                }
            }
        }
        for i in 0..n {
            let (prior, remaining) = a.split_at_mut(i);
            let row = &mut remaining[0];
            for (j, previous) in prior.iter_mut().enumerate() {
                previous[i] = row[j];
            }
            row[i] += damping * row[i].max(1e-8);
        }
        // Hold parameters whose descent direction points outside a physical bound.
        for i in 0..n - 1 {
            let blocked = match i % 4 {
                2 => !signed && p[i] == 0.0 && b[i] < 0.0,
                3 => (p[i] == -1.0 && b[i] < 0.0) || (p[i] == 2.0 && b[i] > 0.0),
                _ => false,
            };
            if blocked {
                a[i].fill(0.0);
                for row in &mut a {
                    row[i] = 0.0;
                }
                a[i][i] = 1.0;
                b[i] = 0.0;
            }
        }
        let Some(delta) = solve(a, b) else {
            return false;
        };
        let mut candidate: Vec<f64> = p.iter().zip(&delta).map(|(x, d)| x + d).collect();
        for c in candidate.as_chunks_mut::<4>().0 {
            c[0] = c[0].clamp(low, high);
            c[1] = c[1].clamp(1.2_f64.ln(), ((high - low) / 3.0).max(1.21).ln());
            c[2] = c[2].clamp(if signed { -2.0 } else { 0.0 }, 2.0);
            c[3] = c[3].clamp(-1.0, 2.0);
        }
        let next = objective(xs, ys, &candidate);
        if next < error {
            let improvement = error - next;
            *p = candidate;
            if improvement <= 1e-10 * error.max(1e-12) || next < 1e-20 {
                return true;
            }
            error = next;
            damping = (damping / 3.0).max(1e-9);
        } else {
            damping *= 5.0;
            if damping > 1e10 {
                return candidate
                    .iter()
                    .zip(p.iter())
                    .all(|(next, old)| (next - old).abs() < 1e-4);
            }
        }
    }
    false
}

#[derive(Clone, Copy)]
struct Seed {
    at: f64,
    width: f64,
}

/// Fit corrected 1D real data. `Some(seeds)` never discovers additional lines.
/// Component and sample/iteration limits produce visible unresolved regions.
pub fn deconvolve(
    real: &[f64],
    axis: &PpmAxis,
    obs_mhz: f64,
    threshold: f64,
    signed: bool,
    seeds: Option<&[f64]>,
) -> PeakFit {
    let mut fit = PeakFit {
        source: fingerprint(real, axis),
        peaks: vec![],
        regions: vec![],
    };
    let endpoint = axis.ppm(real.len().saturating_sub(1) as f64);
    let failed = |fit: &mut PeakFit| {
        fit.regions.push(FitRegion {
            from: if axis.first.is_finite() {
                axis.first
            } else {
                0.0
            },
            to: if endpoint.is_finite() && endpoint != axis.first {
                endpoint
            } else {
                1.0
            },
            status: FitStatus::Failed,
            rms: None,
            reason: Some(FitReason::InvalidInput),
        });
    };
    if real.len() < 8
        || !axis.first.is_finite()
        || !axis.delta.is_finite()
        || axis.delta == 0.0
        || !obs_mhz.is_finite()
        || obs_mhz <= 0.0
        || !(axis.delta.abs() * obs_mhz).is_finite()
        || !endpoint.is_finite()
        || !threshold.is_finite()
        || threshold < 0.0
        || real.iter().any(|v| !v.is_finite())
    {
        failed(&mut fit);
        return fit;
    }
    let magnitude: Vec<f64> = real
        .iter()
        .map(|y| if signed { y.abs() } else { *y })
        .collect();
    let differences: Vec<f64> = real.windows(2).map(|p| p[1] - p[0]).collect();
    let noise = analysis::noise_level(&differences) / 2.0_f64.sqrt();
    let scale = real.iter().fold(0.0_f64, |m, y| m.max(y.abs()));
    let cutoff = threshold.max(5.0 * noise).max(scale * 1e-7);
    if scale == 0.0 {
        return fit;
    }
    let detected: Vec<f64>;
    let positions = match seeds {
        Some(positions) => positions,
        None => {
            let mut candidates: Vec<_> = analysis::find_peaks(
                &magnitude,
                axis,
                axis.first,
                axis.ppm(real.len() as f64),
                cutoff,
                2,
            )
            .iter()
            .map(|p| p.ppm)
            .collect();
            // Standard's ppm width ceiling excludes broad nuclei; let the model test a dominant line.
            if candidates.is_empty()
                && let Some((at, &height)) = magnitude
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                && at > 1
                && at + 2 < real.len()
                && height > cutoff
            {
                candidates.push(axis.ppm(at as f64));
            }
            detected = candidates;
            &detected
        }
    };
    if positions.len() > MAX_TOTAL {
        failed(&mut fit);
        return fit;
    }
    let hz_per_point = axis.delta.abs() * obs_mhz;
    let mut initial: Vec<Seed> = positions
        .iter()
        .filter_map(|&ppm| {
            let at = axis.index(ppm);
            if !ppm.is_finite() || !at.is_finite() || at < 0.0 || at > (real.len() - 1) as f64 {
                return None;
            }
            let i = at.round() as usize;
            if magnitude[i] < cutoff {
                if seeds.is_some() {
                    fit.regions.push(FitRegion {
                        from: axis.ppm(i.saturating_sub(1) as f64),
                        to: axis.ppm((i + 1).min(real.len() - 1) as f64),
                        status: FitStatus::Failed,
                        rms: None,
                        reason: Some(FitReason::BelowThreshold),
                    });
                }
                return None;
            }
            Some(Seed {
                at,
                width: (analysis::line_width_hz(&magnitude, axis, obs_mhz, ppm) / hz_per_point)
                    .clamp(1.5, real.len() as f64 / 4.0),
            })
        })
        .collect();
    initial.sort_by(|a, b| a.at.total_cmp(&b.at));
    initial.dedup_by(|a, b| (a.at - b.at).abs() < 0.5);
    let mut clusters: Vec<(usize, usize, Vec<Seed>)> = Vec::new();
    for seed in initial {
        let low = (seed.at - 10.0 * seed.width).max(0.0) as usize;
        let high = ((seed.at + 10.0 * seed.width).ceil() as usize).min(real.len() - 1);
        if let Some(cluster) = clusters.last_mut().filter(|c| low <= c.1) {
            cluster.1 = cluster.1.max(high);
            cluster.2.push(seed);
        } else {
            clusters.push((low, high, vec![seed]));
        }
    }
    // Break oversized clusters only where the measured signal falls to noise.
    let mut pending: Vec<_> = clusters.into_iter().rev().collect();
    let mut clusters = Vec::new();
    while let Some((low, high, starts)) = pending.pop() {
        if starts.len() > MAX_COMPONENTS {
            let split = starts
                .windows(2)
                .enumerate()
                .filter_map(|(i, pair)| {
                    let left = pair[0].at.ceil() as usize;
                    let right = pair[1].at.floor() as usize;
                    if right <= left + 1
                        || pair[1].at - pair[0].at < 4.0 * pair[0].width.max(pair[1].width)
                    {
                        return None;
                    }
                    let at = (left + 1..right)
                        .min_by(|&a, &b| real[a].abs().total_cmp(&real[b].abs()))?;
                    (real[at - 1..=at + 1].iter().all(|y| y.abs() <= cutoff)).then_some((i + 1, at))
                })
                .min_by_key(|(i, _)| (starts.len() as isize - 2 * *i as isize).unsigned_abs());
            if let Some((index, at)) = split {
                pending.push((at, high, starts[index..].to_vec()));
                pending.push((low, at, starts[..index].to_vec()));
                continue;
            }
        }
        clusters.push((low, high, starts));
    }
    for (low, high, starts) in clusters {
        let max_components = MAX_COMPONENTS.min(MAX_TOTAL - fit.peaks.len());
        let mut region = FitRegion {
            from: axis.ppm(low as f64),
            to: axis.ppm(high as f64),
            status: FitStatus::Unresolved,
            rms: None,
            reason: None,
        };
        if starts.len() > max_components {
            region.reason = Some(FitReason::ComponentLimit);
            fit.regions.push(region);
            continue;
        }
        if high - low < 8 {
            region.reason = Some(FitReason::InsufficientSamples);
            fit.regions.push(region);
            continue;
        }
        let stride = (high - low).div_ceil(MAX_SAMPLES - 1).max(1);
        if starts.iter().any(|s| s.width / (stride as f64) < 2.0) {
            region.reason = Some(FitReason::InsufficientSamples);
            fit.regions.push(region);
            continue;
        }
        let xs: Vec<f64> = (low..=high).step_by(stride).map(|i| i as f64).collect();
        let ys: Vec<f64> = xs.iter().map(|&i| real[i as usize] / scale).collect();
        let mut p: Vec<f64> = starts
            .iter()
            .flat_map(|s| [s.at, s.width.ln(), real[s.at.round() as usize] / scale, 0.0])
            .collect();
        p.push(0.0);
        let mut converged = refine(&xs, &ys, &mut p, signed);
        if seeds.is_none() {
            for _ in p.len() / 4..max_components {
                let residual: Vec<f64> = xs
                    .iter()
                    .zip(&ys)
                    .map(|(&x, &y)| y - p[p.len() - 1] - model(x, &p))
                    .collect();
                let candidate = (1..xs.len() - 1)
                    .filter(|&i| {
                        let height = if signed {
                            residual[i].abs()
                        } else {
                            residual[i]
                        };
                        height > cutoff / scale
                            && height >= residual[i - 1].abs()
                            && height >= residual[i + 1].abs()
                            && p.as_chunks::<4>()
                                .0
                                .iter()
                                .all(|c| (xs[i] - c[0]).abs() > 0.35 * c[1].exp())
                    })
                    .max_by(|&i, &j| residual[i].abs().total_cmp(&residual[j].abs()));
                let Some(i) = candidate else {
                    break;
                };
                let nearest = p
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .min_by(|a, b| (a[0] - xs[i]).abs().total_cmp(&(b[0] - xs[i]).abs()))
                    .unwrap();
                let width = nearest[1].exp() * 0.7;
                let old_error = objective(&xs, &ys, &p);
                let mut trial = p[..p.len() - 1].to_vec();
                trial.extend([xs[i], width.ln(), residual[i], 0.0]);
                trial.push(p[p.len() - 1]);
                let success = refine(&xs, &ys, &mut trial, signed);
                let gain = old_error - objective(&xs, &ys, &trial);
                let penalty = (20.0 * (noise / scale).powi(2) * (xs.len() as f64).ln())
                    .max(old_error * 0.04)
                    .max(1e-15);
                if gain <= penalty {
                    break;
                }
                p = trial;
                converged = success;
            }
        }
        let rms = (objective(&xs, &ys, &p) / xs.len() as f64).sqrt() * scale;
        region.rms = rms.is_finite().then_some(rms);
        let components: Vec<_> = p.as_chunks::<4>().0.iter().collect();
        let undersampled = components
            .iter()
            .any(|c| c[1].exp() / (stride as f64) < 2.0 - 1e-6);
        let degenerate = components.iter().enumerate().any(|(i, a)| {
            a[2].abs() < cutoff / scale * 0.5
                || a[1].exp() <= 1.21
                || components[i + 1..]
                    .iter()
                    .any(|b| (a[0] - b[0]).abs() < 0.4 * a[1].exp().min(b[1].exp()))
        });
        if converged
            && !undersampled
            && !degenerate
            && rms <= (2.0 * noise).max(cutoff * 0.15).max(scale * 1e-5)
        {
            region.status = FitStatus::Converged;
        } else {
            region.reason = Some(if undersampled {
                FitReason::InsufficientSamples
            } else if !converged {
                FitReason::Optimizer
            } else if degenerate {
                FitReason::Degenerate
            } else {
                FitReason::Residual
            });
        }
        for c in components {
            let height = c[2] * scale;
            if height == 0.0 {
                continue;
            }
            let width_ppm = c[1].exp() * axis.delta.abs();
            let area = height * width_ppm * PI / 2.0 * (1.0 - (1.0 - 3.0_f64.sqrt() / 2.0) * c[3]);
            fit.peaks.push(FittedPeak {
                ppm: axis.ppm(c[0]),
                height,
                width_hz: width_ppm * obs_mhz,
                area,
                shape: c[3],
            });
        }
        fit.regions.push(region);
    }
    fit.peaks.sort_by(|a, b| b.ppm.total_cmp(&a.ppm));
    fit
}

#[cfg(test)]
mod tests {
    use super::*;
    const AXIS: PpmAxis = PpmAxis {
        first: 10.0,
        delta: -0.001,
    };
    fn spectrum(lines: &[(f64, f64, f64, f64)]) -> Vec<f64> {
        (0..2048)
            .map(|i| {
                lines
                    .iter()
                    .map(|&(at, width, height, q)| height * shape(i as f64, at, width, q))
                    .sum()
            })
            .collect()
    }
    fn area(width: f64, height: f64, q: f64) -> f64 {
        height * width * AXIS.delta.abs() * PI / 2.0 * (1.0 - (1.0 - 3.0_f64.sqrt() / 2.0) * q)
    }
    fn check(lines: &[(f64, f64, f64, f64)], signed: bool) -> PeakFit {
        let real = spectrum(lines);
        let start = std::time::Instant::now();
        let fit = deconvolve(&real, &AXIS, 400.0, 0.005, signed, None);
        eprintln!(
            "fit {} lines in {:?}: {:?}",
            lines.len(),
            start.elapsed(),
            fit
        );
        assert_eq!(fit.peaks.len(), lines.len(), "{fit:?}");
        assert!(
            fit.regions.iter().all(|r| r.status == FitStatus::Converged),
            "{fit:?}"
        );
        assert!(fit.regions.windows(2).all(|r| r[0].from > r[1].from));
        for &(at, width, height, q) in lines {
            let p = fit
                .peaks
                .iter()
                .min_by(|a, b| {
                    (AXIS.index(a.ppm) - at)
                        .abs()
                        .total_cmp(&(AXIS.index(b.ppm) - at).abs())
                })
                .unwrap();
            assert!((AXIS.index(p.ppm) - at).abs() < 0.3, "{p:?}, target {at}");
            assert!(
                (p.area / area(width, height, q) - 1.0).abs() < 0.02,
                "{p:?}"
            );
        }
        fit
    }
    #[test]
    fn isolated_lorentz_and_generalized_lines_keep_analytical_areas() {
        let fit = check(&[(500.2, 12.0, 1.0, 0.0), (1500.0, 18.0, 0.5, 0.7)], false);
        assert!(fit.integrate(10.0, 8.0).is_some());
    }
    #[test]
    fn generalized_shapes_outside_the_old_range_preserve_areas() {
        check(&[(500.2, 12.0, 1.0, -0.7), (1500.0, 18.0, 0.5, 1.7)], false);
    }
    #[test]
    fn unequal_overlapping_lines_are_fitted_jointly() {
        check(&[(900.0, 16.0, 1.0, 0.0), (925.0, 12.0, 0.35, 0.4)], false);
    }
    #[test]
    fn residual_finds_a_shoulder_without_a_distinct_maximum() {
        let lines = [(900.0, 20.0, 1.0, 0.0), (915.0, 14.0, 0.25, 0.0)];
        let real = spectrum(&lines);
        assert_eq!(
            analysis::find_peaks(&real, &AXIS, 10.0, 8.0, 0.005, 2).len(),
            1
        );
        check(&lines, false);
    }
    #[test]
    fn a_broad_nucleus_is_not_limited_by_standard_picking_width() {
        let axis = PpmAxis {
            first: 100.0,
            delta: -0.01,
        };
        let real = spectrum(&[(900.0, 100.0, 1.0, 0.0)]);
        assert!(analysis::find_peaks(&real, &axis, 100.0, 79.52, 0.005, 3).is_empty());
        let fit = deconvolve(&real, &axis, 130.0, 0.005, false, None);
        assert_eq!(fit.peaks.len(), 1);
        assert_eq!(fit.regions[0].status, FitStatus::Converged, "{fit:?}");
        assert!((fit.peaks[0].area / (100.0 * 0.01 * PI / 2.0) - 1.0).abs() < 0.02);
    }

    #[test]
    fn signed_lines_keep_negative_areas() {
        let fit = check(&[(700.0, 15.0, -1.0, 0.3), (1400.0, 12.0, 0.5, 0.0)], true);
        assert!(fit.peaks.iter().any(|p| p.area < 0.0));
        assert!(
            deconvolve(
                &spectrum(&[(700.0, 15.0, -1.0, 0.0)]),
                &AXIS,
                400.0,
                0.005,
                false,
                None
            )
            .peaks
            .is_empty()
        );
    }
    #[test]
    fn quiet_data_and_invalid_inputs_never_offer_fitted_measurements() {
        let noise: Vec<_> = (0..2048).map(|i| 0.001 * (i as f64 * 1.7).sin()).collect();
        for real in [vec![0.0; 2048], noise] {
            let fit = deconvolve(&real, &AXIS, 400.0, 0.005, false, None);
            assert!(fit.peaks.is_empty());
            assert!(fit.integrate(10.0, 8.0).is_none());
        }
        let bad = deconvolve(&[0.0, f64::NAN, 1.0], &AXIS, 400.0, 0.005, false, None);
        assert_eq!(bad.regions[0].status, FitStatus::Failed);
        assert_eq!(bad.regions[0].reason, Some(FitReason::InvalidInput));
        assert!(bad.integrate(10.0, 8.0).is_none());
    }
    #[test]
    fn accepted_seeds_do_not_reintroduce_excluded_components() {
        let real = spectrum(&[(500.0, 12.0, 1.0, 0.0), (1500.0, 12.0, 0.8, 0.0)]);
        let fit = deconvolve(
            &real,
            &AXIS,
            400.0,
            0.005,
            false,
            Some(&[AXIS.ppm(500.0), f64::NAN, 90.0]),
        );
        assert_eq!(fit.peaks.len(), 1);
        assert!((AXIS.index(fit.peaks[0].ppm) - 500.0).abs() < 0.3);
        assert!(
            deconvolve(&real, &AXIS, 400.0, 0.005, false, Some(&[]))
                .peaks
                .is_empty()
        );
    }
    #[test]
    fn accepted_subthreshold_lines_make_their_measurement_unavailable() {
        let real = spectrum(&[(900.0, 20.0, 1.0, 0.0), (1500.0, 12.0, 0.001, 0.0)]);
        let fit = deconvolve(
            &real,
            &AXIS,
            400.0,
            0.005,
            false,
            Some(&[AXIS.ppm(900.0), AXIS.ppm(1500.0)]),
        );
        assert!(fit.regions.iter().any(|r| r.status == FitStatus::Failed));
        assert!(
            fit.regions
                .iter()
                .any(|r| r.reason == Some(FitReason::BelowThreshold))
        );
        assert!(fit.integrate(AXIS.ppm(1400.0), AXIS.ppm(1600.0)).is_none());
        assert!(fit.integrate(10.0, 8.0).is_none());
        assert!(fit.integrate(AXIS.ppm(800.0), AXIS.ppm(1000.0)).is_some());
    }

    #[test]
    fn degenerate_fits_are_not_measurements_and_boundaries_do_not_double_count() {
        let real = spectrum(&[(900.0, 20.0, 1.0, 0.0), (902.0, 20.0, 0.5, 0.0)]);
        let fit = deconvolve(
            &real,
            &AXIS,
            400.0,
            0.005,
            false,
            Some(&[AXIS.ppm(900.0), AXIS.ppm(902.0)]),
        );
        assert_eq!(fit.regions[0].status, FitStatus::Unresolved);
        assert!(fit.integrate(10.0, 8.0).is_none());
        let good = check(&[(900.0, 20.0, 1.0, 0.0)], false);
        let center = good.peaks[0].ppm;
        assert!(good.integrate(center, center + 0.1).is_some());
        assert!(good.integrate(center - 0.1, center).is_none());
    }
    #[test]
    fn fingerprint_checks_data_and_axis_and_restored_fields() {
        let real = spectrum(&[(900.0, 20.0, 1.0, 0.0)]);
        let mut fit = deconvolve(&real, &AXIS, 400.0, 0.005, false, None);
        assert!(fit.fits_source(&real, &AXIS));
        assert!(fit.valid_for(&real, &AXIS, 400.0));
        let mut corrupt = fit.clone();
        corrupt.peaks[0].area *= 2.0;
        assert!(!corrupt.valid_for(&real, &AXIS, 400.0));
        assert!(!fit.valid_for(&real, &AXIS, 500.0));
        corrupt = fit.clone();
        corrupt.regions[0].rms = None;
        assert!(!corrupt.valid_for(&real, &AXIS, 400.0));
        assert!(corrupt.integrate(10.0, 8.0).is_none());
        let mut changed = real.clone();
        changed[0] += 1.0;
        assert!(!fit.fits_source(&changed, &AXIS));
        assert!(!fit.fits_source(
            &real,
            &PpmAxis {
                first: 9.0,
                delta: AXIS.delta
            }
        ));
        fit.peaks[0].width_hz = -1.0;
        assert!(!fit.fits_source(&real, &AXIS));
    }

    #[test]
    fn realistic_noise_does_not_add_components_and_resolution_preserves_area() {
        for ratio in [1, 2, 4] {
            let axis = PpmAxis {
                first: AXIS.first,
                delta: AXIS.delta / ratio as f64,
            };
            let mut state = 42_u64;
            let real: Vec<_> = (0..2048 * ratio)
                .map(|i| {
                    let noise: f64 = (0..12)
                        .map(|_| {
                            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                            ((state >> 32) as f64 / u32::MAX as f64) - 0.5
                        })
                        .sum();
                    shape(i as f64 / ratio as f64, 900.0, 20.0, 0.4) + noise * 0.001
                })
                .collect();
            let fit = deconvolve(&real, &axis, 400.0, 0.005, false, None);
            assert_eq!(fit.peaks.len(), 1, "{fit:?}");
            assert_eq!(fit.regions[0].status, FitStatus::Converged, "{fit:?}");
            assert!((fit.peaks[0].area / area(20.0, 1.0, 0.4) - 1.0).abs() < 0.02);
            assert!(fit.valid_for(&real, &axis, 400.0));
        }
    }

    #[test]
    fn a_component_limit_produces_unavailable_regions() {
        let lines: Vec<_> = (0..MAX_COMPONENTS + 1)
            .map(|i| (700.0 + 15.0 * i as f64, 15.0, 1.0, 0.0))
            .collect();
        let real = spectrum(&lines);
        let seeds: Vec<_> = lines.iter().map(|p| AXIS.ppm(p.0)).collect();
        let fit = deconvolve(&real, &AXIS, 400.0, 0.005, false, Some(&seeds));
        assert_eq!(fit.regions[0].status, FitStatus::Unresolved);
        assert_eq!(fit.regions[0].reason, Some(FitReason::ComponentLimit));
        assert!(fit.integrate(10.0, 8.0).is_none());
    }

    #[test]
    fn crowded_clusters_split_only_at_noise_level_gaps() {
        let lines: Vec<_> = (0..9)
            .map(|i| (300.0 + 155.0 * i as f64, 15.0, 1.0, 0.0))
            .collect();
        let real = spectrum(&lines);
        let seeds: Vec<_> = lines.iter().map(|p| AXIS.ppm(p.0)).collect();
        let fit = deconvolve(&real, &AXIS, 400.0, 0.025, false, Some(&seeds));
        assert_eq!(fit.peaks.len(), lines.len(), "{fit:?}");
        assert!(
            fit.regions.iter().all(|r| r.status == FitStatus::Converged),
            "{fit:?}"
        );
        assert!(fit.regions.windows(2).all(|r| r[0].from > r[1].from));
        for peak in &fit.peaks {
            assert!(
                (peak.area / area(15.0, 1.0, 0.0) - 1.0).abs() < 0.02,
                "{peak:?}"
            );
        }
    }

    #[test]
    fn narrow_lines_explain_the_sampling_rejection() {
        let real = spectrum(&[(900.0, 1.0, 1.0, 0.0)]);
        let fit = deconvolve(&real, &AXIS, 400.0, 0.005, false, Some(&[AXIS.ppm(900.0)]));
        assert_eq!(fit.regions[0].reason, Some(FitReason::InsufficientSamples));
        assert!(fit.integrate(10.0, 8.0).is_none());
    }

    #[test]
    fn narrow_line_rejection_does_not_offer_an_area_at_different_subpoint_positions() {
        for width in [1.0, 1.5] {
            for center in [900.0, 900.25, 900.5] {
                let real = spectrum(&[(center, width, 1.0, 0.0)]);
                let fit = deconvolve(&real, &AXIS, 400.0, 0.005, false, Some(&[AXIS.ppm(center)]));
                assert_ne!(fit.regions[0].status, FitStatus::Converged, "{fit:?}");
                assert!(fit.integrate(10.0, 8.0).is_none());
            }
        }
    }

    #[test]
    fn adequately_sampled_narrow_lines_can_be_measured() {
        for center in [900.0, 900.25, 900.5] {
            let fit = check(&[(center, 2.0, 1.0, 0.0)], false);
            assert!(fit.integrate(10.0, 8.0).is_some());
        }
    }

    #[test]
    fn degenerate_candidates_do_not_invalidate_the_entire_fit() {
        let real = spectrum(&[(900.0, 20.0, 1.0, 0.0), (920.0, 10.0, -0.05, 0.0)]);
        let mut zero_candidate_seen = false;
        for offset in 3..25 {
            let seeds = [
                AXIS.ppm(900.0),
                AXIS.ppm(900.0 + offset as f64),
                AXIS.ppm(900.0 - offset as f64),
            ];
            let fit = deconvolve(&real, &AXIS, 400.0, 0.005, false, Some(&seeds));
            assert!(fit.valid_for(&real, &AXIS, 400.0), "{fit:?}");
            if fit.peaks.len() < seeds.len() {
                zero_candidate_seen = true;
                assert!(fit.regions.iter().all(|r| r.status != FitStatus::Converged));
                assert!(fit.integrate(10.0, 8.0).is_none());
            }
        }
        assert!(zero_candidate_seen);
    }
}
