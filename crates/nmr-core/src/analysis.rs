//! Noise estimation, integration and peak picking on the real part of 1D spectra.

use std::collections::BTreeMap;
use std::ops::Range;

use crate::PpmAxis;

/// Ratio of the standard deviation to the median absolute deviation for normally
/// distributed noise.
const MAD_TO_SIGMA: f64 = 1.4826;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Peak {
    pub ppm: f64,
    pub intensity: f64,
}

/// Robust noise standard deviation: 1.4826 × the median absolute deviation from the
/// median. Signals occupy few points of an NMR spectrum, so they barely move it.
/// How sharply a maximum has to curve, in multiples of the curvature noise, to
/// count as a peak rather than a ripple.
const CURVATURE_SIGMA: f64 = 4.0;
/// How far above the lowest point so far the spectrum must rise, in noise, before
/// a walk down a line's flank takes it for the valley before a neighbour. Less,
/// and noise on the flank of a broad line stopped the walk two or three points
/// out, so a real line at fifteen times the noise was judged across a span where
/// its curvature was only noise, and thrown away.
const VALLEY_SIGMA: f64 = 3.0;
/// Half the width at half height beyond which a maximum is a swell of the
/// baseline rather than a line: 0.25 ppm is 100 Hz at 400 MHz, broader than an
/// exchanging NH or OH. Without it, a hump a ppm and a half wide passed as a
/// peak once noise on its flanks no longer cut its measured width short.
const MAX_HALF_WIDTH_PPM: f64 = 0.25;
/// Points either side of a maximum that its curvature is measured across.
/// Adjacent points are too fine on a spectrum that is not zero filled: a real but
/// slightly broad line barely curves from one point to the next, and was thrown
/// away even well above the threshold.
pub fn noise_level(real: &[f64]) -> f64 {
    if real.is_empty() {
        return 0.0;
    }
    let mut values = real.to_vec();
    let centre = median(&mut values);
    values.iter_mut().for_each(|v| *v = (*v - centre).abs());
    MAD_TO_SIGMA * median(&mut values)
}

fn median(values: &mut [f64]) -> f64 {
    let len = values.len();
    let (lower_half, &mut upper, _) = values.select_nth_unstable_by(len / 2, f64::total_cmp);
    if len % 2 == 1 {
        return upper;
    }
    let lower = lower_half.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    (lower + upper) / 2.0
}

/// Point indices covered by two ppm limits, rounded and sorted as nmrglue's
/// `integrate` does, and clamped to the data.
fn index_range(axis: &PpmAxis, len: usize, a: f64, b: f64) -> Range<usize> {
    let index = |ppm: f64| axis.index(ppm).round().clamp(0.0, len as f64) as usize;
    let (i, j) = (index(a), index(b));
    i.min(j)..i.max(j)
}

/// Integral between two ppm limits, as nmrglue's `analysis.integration.integrate`:
/// the sum of the points times the point spacing in ppm.
pub fn integrate(real: &[f64], axis: &PpmAxis, from_ppm: f64, to_ppm: f64) -> f64 {
    let range = index_range(axis, real.len(), from_ppm, to_ppm);
    real[range].iter().sum::<f64>() * axis.delta.abs()
}

/// Running integral between two ppm limits, from the high-ppm end, sampled at up to
/// `samples` evenly spaced points as interleaved `ppm, value` pairs for drawing.
/// The last value equals [`integrate`].
pub fn integral_curve(
    real: &[f64],
    axis: &PpmAxis,
    from_ppm: f64,
    to_ppm: f64,
    samples: usize,
) -> Vec<f32> {
    let range = index_range(axis, real.len(), from_ppm, to_ppm);
    let dx = axis.delta.abs();
    let mut running = 0.0;
    let cumulative: Vec<f64> = real[range.clone()]
        .iter()
        .map(|y| {
            running += y * dx;
            running
        })
        .collect();
    let Some(last) = cumulative.len().checked_sub(1) else {
        return Vec::new();
    };
    let samples = samples.clamp(1, cumulative.len());
    (0..samples)
        .flat_map(|k| {
            let i = if samples == 1 {
                last
            } else {
                k * last / (samples - 1)
            };
            [
                axis.ppm((range.start + i) as f64) as f32,
                cumulative[i] as f32,
            ]
        })
        .collect()
}

/// Indices in `range` of points above `threshold` that no point within `separation`
/// on either side exceeds, as nmrglue's `peakpick.find_all_thres_fast`.
pub fn local_maxima(
    real: &[f64],
    range: Range<usize>,
    threshold: f64,
    separation: usize,
) -> Vec<usize> {
    let end = range.end.min(real.len());
    (range.start.min(end)..end)
        .filter(|&i| {
            let y = real[i];
            let window = i.saturating_sub(separation)..(i + separation + 1).min(real.len());
            // A flat top is one line, not one for every point of it. Taking every
            // point that nothing in its window beats accepts both halves of a tie,
            // and `refine` then puts them at the same shift: the first point of
            // the tie is the line, and its offset carries it to the middle.
            y > threshold
                && real[window.clone()].iter().all(|&v| v <= y)
                && real[window.start..i].iter().all(|&v| v < y)
        })
        .collect()
}

/// Half of how wide a line is at half its height, in points: the span at which a
/// line's curvature answers for its own height rather than for how sharp it is.
/// The same rule as [`line_width_hz`], which is the width a chemist reads.
fn width_in_points(real: &[f64], centre: usize, valley: f64) -> f64 {
    let Some(last) = real.len().checked_sub(1) else {
        return 1.0;
    };
    let half = real[centre] / 2.0;
    if half <= 0.0 || !half.is_finite() {
        return 1.0;
    }
    let halves = half_widths(real, centre, half, (real[centre] / 20.0).max(valley), last);
    match (halves.left_stopped, halves.right_stopped) {
        (true, false) => halves.right,
        (false, true) => halves.left,
        _ => (halves.left + halves.right) / 2.0,
    }
    .max(1.0)
}

/// Second difference: strongly negative at the top of a line, near zero on the
/// slow swell of a baseline.
fn curvature(real: &[f64], i: usize, span: usize) -> f64 {
    real[i - span] - 2.0 * real[i] + real[i + span]
}

/// Spread of the curvature away from peaks, from its median absolute value.
fn curvature_noise(real: &[f64], span: usize) -> f64 {
    if real.len() < 2 * span + 1 {
        return 0.0;
    }
    let mut values: Vec<f64> = (span..real.len() - span)
        .map(|i| curvature(real, i, span).abs())
        .collect();
    let middle = values.len() / 2;
    let (_, median, _) = values.select_nth_unstable_by(middle, f64::total_cmp);
    MAD_TO_SIGMA * *median
}

/// Peaks above `threshold` between two ppm limits, refined by parabolic
/// interpolation through each maximum and its neighbours.
///
/// Deviation from nmrglue, whose `peakpick.pick` only compares with a level: a
/// maximum also has to curve like a line. The curvature is judged across the
/// line's own width, where a Lorentzian's second difference comes to minus its
/// own height whatever that width is, and against the noise measured at that same
/// span. Judged across a fixed few points it asks instead whether a line is
/// narrow: measured, it threw away a line at 32 times the noise for being 6.9 Hz
/// wide while keeping one at 5.7 times and 1.8 Hz. The
/// second difference is large and negative at the top of a line and near zero on
/// a rolling baseline, so ripple riding high on a hump is rejected while a
/// shoulder on a strong line is kept. What is picked is then set by the threshold
/// line, which is dragged in the plot. [`local_maxima`] keeps nmrglue's
/// behaviour, and the golden tests compare against that.
pub fn find_peaks(
    real: &[f64],
    axis: &PpmAxis,
    from_ppm: f64,
    to_ppm: f64,
    threshold: f64,
    separation: usize,
) -> Vec<Peak> {
    let range = index_range(axis, real.len(), from_ppm, to_ppm);
    // Judged across the line's own width, both sides of the test together. A
    // Lorentzian of height h and half-width w has, across a span of exactly w, a
    // second difference of -h whatever w is, so a broad line is no longer asked
    // to curve like a narrow one: measured, a line at 32 times the noise and 6.9
    // Hz wide was thrown away while one at 5.7 times and 1.8 Hz was kept. The
    // noise it is held against is measured at the same span, or a ripple a point
    // or two wide is judged against a bar meant for a line ten times its width
    // and passes.
    let valley = VALLEY_SIGMA * noise_level(real);
    let candidates: Vec<(usize, usize)> = local_maxima(real, range, threshold, separation)
        .into_iter()
        .map(|i| {
            (
                i,
                (width_in_points(real, i, valley).round() as usize).clamp(1, real.len() / 4),
            )
        })
        .collect();
    let mut bars: BTreeMap<usize, f64> = BTreeMap::new();
    for &(_, span) in &candidates {
        bars.entry(span)
            .or_insert_with(|| -CURVATURE_SIGMA * curvature_noise(real, span));
    }
    candidates
        .into_iter()
        .filter(|&(i, span)| {
            if i < span || i + span >= real.len() {
                return false;
            }
            if span as f64 * axis.delta.abs() > MAX_HALF_WIDTH_PPM {
                return false;
            }
            // The tallest thing within its own width, so a ripple riding on the
            // flank of a broad line loses to the line, and a summit that noise has
            // split in two is counted once.
            let y = real[i];
            let window = i - span..(i + span + 1).min(real.len());
            real[window.clone()].iter().all(|&v| v <= y)
                && real[window.start..i].iter().all(|&v| v < y)
                && curvature(real, i, span) < bars[&span]
        })
        .map(|(i, _)| refine(real, axis, i))
        .collect()
}

/// How far a signal may reach beyond its outermost line, in widths at half
/// height of that line; the noise usually stops it first. A Lorentzian holds 98 %
/// of its area within 32 widths either side, and at eight the broad foot of a
/// water line, narrow at the top, was cut off.
const REACH: f64 = 32.0;

/// How far past the point where a signal meets the noise its region goes on, in
/// widths of its outermost line: the tail is still there under the noise. On the
/// datasets in `testdata` stopping at the noise kept 94-99 % of each signal's
/// area, and two widths on 97-100 %, so the proton counts came out more even.
const MARGIN: f64 = 2.0;

/// Regions smaller than this share of the median one are left out, as a minimum
/// area is in peak-picking integration: on the datasets in `testdata` what fell
/// below it was three or four hundredths of a proton, and at a tenth the same
/// impurity at 0.1 H was kept in one spectrum and dropped from the next.
const MIN_SHARE: f64 = 0.05;

/// Regions worth integrating, built from the lines `find_peaks` picks, as a
/// peak-picking integration is: lines closer than `max_j_hz` are one signal,
/// since that is as far apart as a coupling puts them, and so are lines whose
/// valley stays above a fifth of the weaker one, since no cut between them
/// would leave either whole. Two signals are parted at the lowest point between
/// them, and an outer edge goes down to the foot of the line or `REACH` widths,
/// whichever is nearer, and `MARGIN` widths beyond - the skirts carry area that
/// stopping at the threshold throws away. Regions under `MIN_SHARE` of the median are dropped.
///
/// Runs of points above the threshold were what this read before: it merged
/// every run within a fixed number of points, so two singlets 24 Hz apart came
/// out as one integral, and its edges did not follow the line's own width.
pub fn integral_regions(
    real: &[f64],
    axis: &PpmAxis,
    obs_mhz: f64,
    from_ppm: f64,
    to_ppm: f64,
    reading: Reading,
) -> Vec<(f64, f64)> {
    let peaks: Vec<f64> = find_peaks(
        real,
        axis,
        from_ppm,
        to_ppm,
        reading.threshold,
        reading.separation,
    )
    .iter()
    .map(|p| p.ppm)
    .collect();
    integral_regions_from_peaks(real, axis, obs_mhz, from_ppm, to_ppm, reading, &peaks)
}

/// Build integration regions from accepted peaks without detecting excluded lines again.
/// Invalid positions are ignored, and peaks at the same data point count once.
pub fn integral_regions_from_peaks(
    real: &[f64],
    axis: &PpmAxis,
    obs_mhz: f64,
    from_ppm: f64,
    to_ppm: f64,
    reading: Reading,
    peaks: &[f64],
) -> Vec<(f64, f64)> {
    let range = index_range(axis, real.len(), from_ppm, to_ppm);
    let hz_per_point = axis.delta.abs() * obs_mhz;
    if range.is_empty() || !hz_per_point.is_finite() || hz_per_point <= 0.0 {
        return Vec::new();
    }
    // Where each line is, in points, and how wide it is.
    let mut lines: Vec<(usize, f64)> = peaks
        .iter()
        .copied()
        .filter(|ppm| {
            ppm.is_finite() && *ppm >= from_ppm.min(to_ppm) && *ppm <= from_ppm.max(to_ppm)
        })
        .filter_map(|ppm| {
            let at = axis.index(ppm).round();
            if !at.is_finite() || at < range.start as f64 || at >= range.end as f64 {
                return None;
            }
            let width = line_width_hz(real, axis, obs_mhz, ppm) / hz_per_point;
            Some((at as usize, width.max(1.0)))
        })
        .collect();
    lines.sort_by_key(|l| l.0);
    lines.dedup_by_key(|l| l.0);

    let valley = |a: usize, b: usize| (a..=b).min_by(|&i, &j| real[i].total_cmp(&real[j]));
    let mut groups: Vec<Vec<(usize, f64)>> = Vec::new();
    for line in lines {
        if let Some(group) = groups.last_mut() {
            let last = group[group.len() - 1].0;
            let weaker = real[last].min(real[line.0]);
            let low = valley(last, line.0).map_or(0.0, |i| real[i]);
            if (line.0 - last) as f64 * hz_per_point <= reading.max_j_hz || low > weaker / 5.0 {
                group.push(line);
                continue;
            }
        }
        groups.push(vec![line]);
    }

    let foot = reading.threshold / 4.0;
    let parts: Vec<usize> = groups
        .windows(2)
        .filter_map(|pair| valley(pair[0][pair[0].len() - 1].0, pair[1][0].0))
        .collect();
    let regions = groups
        .iter()
        .enumerate()
        .map(|(g, group)| {
            let (first, last) = (group[0], group[group.len() - 1]);
            let low = if g == 0 { range.start } else { parts[g - 1] };
            let high = parts.get(g).map_or(range.end - 1, |&p| p);
            let mut from = first.0;
            while from > low && real[from - 1] > foot && (first.0 - from) as f64 <= REACH * first.1
            {
                from -= 1;
            }
            from = from.saturating_sub((MARGIN * first.1) as usize).max(low);
            let mut to = last.0;
            while to < high && real[to + 1] > foot && (to - last.0) as f64 <= REACH * last.1 {
                to += 1;
            }
            to = (to + (MARGIN * last.1) as usize).min(high);
            from..to + 1
        })
        .collect::<Vec<_>>();
    let area = |r: &Range<usize>| real[r.clone()].iter().sum::<f64>();
    let mut areas: Vec<f64> = regions.iter().map(area).collect();
    areas.sort_by(f64::total_cmp);
    let floor = areas.get(areas.len() / 2).map_or(0.0, |m| m * MIN_SHARE);
    regions
        .into_iter()
        .filter(|r| area(r) >= floor)
        .map(|r| {
            let (a, b) = (axis.ppm(r.start as f64), axis.ppm(r.end as f64));
            (a.max(b), a.min(b))
        })
        .collect()
}

/// A signal read as one multiplet: where it sits, what pattern its lines make,
/// and the couplings behind them.
#[derive(Debug, Clone, PartialEq)]
pub struct Multiplet {
    /// Intensity-weighted centre of the lines, which is the shift quoted.
    pub ppm: f64,
    /// `s`, `d`, `t`, `q`, `quint`, `sext`, `sept`, `dd`, or `m` for anything a
    /// first-order reading does not explain.
    pub kind: String,
    pub js_hz: Vec<f64>,
    pub lines: usize,
    /// How wide its lines are, in Hz: the middle one of them, so that neither a
    /// broad neighbour nor a sharp spike decides it on its own.
    pub width_hz: f64,
}

/// What a reading of a multiplet needs beyond the data: how high a line has to
/// stand, how far apart two maxima have to be to be two lines, and what a
/// coupling can be for the nucleus at hand.
#[derive(Debug, Clone, Copy)]
pub struct Reading {
    pub threshold: f64,
    pub separation: usize,
    pub max_j_hz: f64,
}

/// How far two spacings may differ and still be read as the same coupling.
const J_TOLERANCE: f64 = 0.12;
/// Couplings closer than this are the same line measured twice, in Hz.
const J_FLOOR: f64 = 0.3;

fn even(spacings: &[f64]) -> bool {
    let mean = spacings.iter().sum::<f64>() / spacings.len() as f64;
    mean > J_FLOOR
        && spacings
            .iter()
            .all(|s| (s - mean).abs() <= J_TOLERANCE * mean)
}

fn mean(spacings: &[f64]) -> f64 {
    spacings.iter().sum::<f64>() / spacings.len() as f64
}

/// How far a line's height may be off a whole number of the weakest line and
/// still be read as that many of them.
const UNIT_TOLERANCE: f64 = 0.15;

/// What a coupling repeated that many times is called, largest first.
const LETTERS: [&str; 8] = ["d", "t", "q", "quint", "sext", "sept", "oct", "non"];

/// The couplings a set of lines is split by, read as a chemist reads a multiplet.
///
/// The weakest line is one unit and every line is a whole number of units tall,
/// which is what tells a triplet (1:2:1) from three signals that happen to be
/// evenly spaced. The widest gap at the edge is the smallest coupling - the
/// outermost line is every coupling pointing the same way, and its neighbour is
/// that line with the smallest one turned over - so pairing each line with the one
/// that far below it and keeping the midpoints leaves the same multiplet with one
/// coupling fewer. Repeating that until one line is left gives them all.
///
/// `None` when the lines are not a first-order pattern: heights that are not whole
/// units, a line with no partner, or more couplings than a multiplet carries. The
/// caller then reads the spacings instead, which is all that can be said.
fn splittings(peaks: &[Peak], obs_mhz: f64, max_j_hz: f64) -> Option<Vec<f64>> {
    let unit = peaks
        .iter()
        .map(|p| p.intensity.abs())
        .fold(f64::INFINITY, f64::min);
    if unit <= 0.0 || !unit.is_finite() {
        return None;
    }
    let mut lines: Vec<(f64, f64)> = Vec::with_capacity(peaks.len());
    for peak in peaks {
        let exact = peak.intensity.abs() / unit;
        let units = exact.round();
        if (exact - units).abs() > UNIT_TOLERANCE * exact {
            return None;
        }
        lines.push((peak.ppm * obs_mhz, units));
    }
    lines.sort_by(|a, b| b.0.total_cmp(&a.0));
    // A first-order multiplet has two lines for every coupling, some of them
    // landing together: anything else is not one.
    let total: f64 = lines.iter().map(|l| l.1).sum();
    let couplings = total.log2();
    if (couplings - couplings.round()).abs() > 1e-6 || couplings.round() > 6.0 {
        return None;
    }
    let mut js = Vec::new();
    while lines.len() > 1 {
        let j = lines[0].0 - lines[1].0;
        if j <= J_FLOOR || j > max_j_hz {
            return None;
        }
        let mut left = lines.clone();
        let mut next = Vec::new();
        for i in 0..left.len() {
            let (at, units) = left[i];
            if units <= 0.0 {
                continue;
            }
            let partner = left
                .iter()
                .position(|(p, u)| *u > 0.0 && (*p - (at - j)).abs() <= J_TOLERANCE * j)?;
            if left[partner].1 + 1e-9 < units {
                return None;
            }
            left[partner].1 -= units;
            left[i].1 = 0.0;
            next.push((at - j / 2.0, units));
        }
        js.push(j);
        lines = next;
    }
    Some(js)
}

/// The name for a set of couplings: equal ones are one coupling repeated, which
/// is what makes a triplet rather than two doublets, and the letters are written
/// largest coupling first, as a report writes them.
fn classify(mut js: Vec<f64>) -> Option<(String, Vec<f64>)> {
    js.sort_by(|a, b| b.total_cmp(a));
    let mut groups: Vec<(f64, usize)> = Vec::new();
    for j in js {
        match groups.last_mut() {
            Some((mean, count)) if (j - *mean).abs() <= J_TOLERANCE * *mean => {
                *mean = (*mean * *count as f64 + j) / (*count as f64 + 1.0);
                *count += 1;
            }
            _ => groups.push((j, 1)),
        }
    }
    let mut name = String::new();
    for (_, count) in &groups {
        name.push_str(LETTERS.get(count - 1).copied()?);
    }
    if name.is_empty() {
        name.push('s');
    }
    Some((name, groups.iter().map(|(j, _)| *j).collect()))
}

/// Reads a range as one multiplet, first order: the lines above `threshold`, the
/// spacings between them as couplings, and the pattern those spacings make.
///
/// This is what a chemist does by hand and what a report quotes; it is not the
/// full fit a deconvolution performs, so anything the simple reading does not
/// explain is left as `m` with nothing invented for it.
///
/// `max_j_hz` is what a coupling can be for the nucleus at hand. Two lines much
/// further apart than that are two signals, not one split one: measured on the
/// dataset in testdata, a range covering water and methanol together read as a
/// doublet with a 35.7 Hz coupling, which is not a coupling.
pub fn multiplet(
    real: &[f64],
    axis: &PpmAxis,
    obs_mhz: f64,
    from_ppm: f64,
    to_ppm: f64,
    reading: Reading,
) -> Multiplet {
    let peaks = find_peaks(
        real,
        axis,
        from_ppm,
        to_ppm,
        reading.threshold,
        reading.separation,
    );
    let centre = (from_ppm + to_ppm) / 2.0;
    if peaks.is_empty() {
        return Multiplet {
            ppm: centre,
            kind: "m".to_string(),
            js_hz: Vec::new(),
            lines: 0,
            width_hz: 0.0,
        };
    }
    let mut widths: Vec<f64> = peaks
        .iter()
        .map(|p| line_width_hz(real, axis, obs_mhz, p.ppm))
        .filter(|w| *w > 0.0)
        .collect();
    widths.sort_by(f64::total_cmp);
    let width_hz = widths.get(widths.len() / 2).copied().unwrap_or(0.0);
    let weight: f64 = peaks.iter().map(|p| p.intensity.abs()).sum();
    let ppm = if weight > 0.0 {
        peaks.iter().map(|p| p.ppm * p.intensity.abs()).sum::<f64>() / weight
    } else {
        centre
    };
    let spacings: Vec<f64> = peaks
        .windows(2)
        .map(|w| (w[0].ppm - w[1].ppm).abs() * obs_mhz)
        .collect();
    // Further apart than this nucleus can couple: two signals caught in one
    // range, not one signal split in two.
    if spacings.iter().any(|s| *s > reading.max_j_hz) {
        return Multiplet {
            ppm,
            kind: "m".to_string(),
            js_hz: Vec::new(),
            lines: peaks.len(),
            width_hz,
        };
    }
    // What the lines themselves say, when they say anything: this reads every
    // class a first-order multiplet can have. The spacings below are what is left
    // when they do not - lines of uneven height, or a pattern that does not peel.
    if let Some((kind, js_hz)) = splittings(&peaks, obs_mhz, reading.max_j_hz).and_then(classify) {
        return Multiplet {
            ppm,
            kind,
            js_hz,
            lines: peaks.len(),
            width_hz,
        };
    }
    let (kind, js_hz) = match spacings.len() {
        0 => ("s", Vec::new()),
        1 if spacings[0] > J_FLOOR => ("d", vec![spacings[0]]),
        2 if even(&spacings) => ("t", vec![mean(&spacings)]),
        2 => ("dd", spacings.clone()),
        3 if even(&spacings) => ("q", vec![mean(&spacings)]),
        // Four lines with matching outer spacings are two couplings: the small one
        // splits each line of the large one.
        3 if (spacings[0] - spacings[2]).abs() <= J_TOLERANCE * spacings[0].max(spacings[2]) => (
            "dd",
            vec![spacings[0] + spacings[1], (spacings[0] + spacings[2]) / 2.0],
        ),
        4 if even(&spacings) => ("quint", vec![mean(&spacings)]),
        5 if even(&spacings) => ("sext", vec![mean(&spacings)]),
        6 if even(&spacings) => ("sept", vec![mean(&spacings)]),
        _ => ("m", Vec::new()),
    };
    Multiplet {
        ppm,
        kind: kind.to_string(),
        js_hz,
        lines: peaks.len(),
        width_hz,
    }
}

/// The highest point within `radius` points of `ppm`, refined like [`find_peaks`]:
/// peak-by-peak picking.
pub fn nearest_peak(real: &[f64], axis: &PpmAxis, ppm: f64, radius: usize) -> Option<Peak> {
    let centre = axis.index(ppm).round();
    if !(0.0..real.len() as f64).contains(&centre) {
        return None;
    }
    let c = centre as usize;
    let window = c.saturating_sub(radius)..(c + radius + 1).min(real.len());
    let i = window.max_by(|&a, &b| real[a].total_cmp(&real[b]))?;
    Some(refine(real, axis, i))
}

/// The cross peak nearest `[f2, f1]` in ppm: the largest absolute value
/// within `radius` points each way (F2's, then F1's: a pixel is seldom as many
/// points along both), so the negative peaks of an edited HSQC are
/// found too, placed between points by a parabola through it and its
/// neighbours along each axis. `real` is row by row, `cols` wide, one row per
/// F1 point; `axes` are F2's and F1's.
pub fn nearest_peak_2d(
    real: &[f64],
    cols: usize,
    axes: &[PpmAxis; 2],
    [f2, f1]: [f64; 2],
    [along, across]: [usize; 2],
) -> Option<[f64; 2]> {
    let rows = real.len().checked_div(cols)?;
    let (c, r) = (axes[0].index(f2).round(), axes[1].index(f1).round());
    if !(0.0..cols as f64).contains(&c) || !(0.0..rows as f64).contains(&r) {
        return None;
    }
    let (c, r) = (c as usize, r as usize);
    let height = |r: usize, c: usize| real[r * cols + c].abs();
    let (r, c) = (r.saturating_sub(across)..(r + across + 1).min(rows))
        .flat_map(|r| (c.saturating_sub(along)..(c + along + 1).min(cols)).map(move |c| (r, c)))
        .max_by(|a, b| height(a.0, a.1).total_cmp(&height(b.0, b.1)))?;
    // The vertex of the parabola through three heights, as an offset from the
    // middle one; none at an edge or on a flat top.
    let vertex = |a: f64, b: f64, d: f64| {
        let curve = a - 2.0 * b + d;
        if curve < 0.0 {
            0.5 * (a - d) / curve
        } else {
            0.0
        }
    };
    let dc = if c > 0 && c + 1 < cols {
        vertex(height(r, c - 1), height(r, c), height(r, c + 1))
    } else {
        0.0
    };
    let dr = if r > 0 && r + 1 < rows {
        vertex(height(r - 1, c), height(r, c), height(r + 1, c))
    } else {
        0.0
    };
    Some([axes[0].ppm(c as f64 + dc), axes[1].ppm(r as f64 + dr)])
}

/// Width of the line at `ppm` where it is half its height, in Hz.
///
/// A line with a close neighbour reads narrow: its inner half is cut short at the
/// valley between them and it is the narrower half that is doubled. Measured on a
/// doublet 7.8 Hz across, each line reads about a fifth under what the same line
/// reads standing alone. That is the price of never reading the pair as one line,
/// and erring narrow is the safer way round - a simulated shape drawn too broad
/// swamps the signal it is meant to be compared with, and the width can be typed
/// over in the multiplet's window anyway.
///
/// A simulated multiplet has to be drawn with a width if it is to be laid over
/// the signal and compared with it, and the width the spectrum already shows is
/// the one to start from.
pub fn line_width_hz(real: &[f64], axis: &PpmAxis, obs_mhz: f64, ppm: f64) -> f64 {
    let Some(last) = real.len().checked_sub(1) else {
        return 0.0;
    };
    let centre = axis.index(ppm).round().clamp(0.0, last as f64) as usize;
    let half = real[centre] / 2.0;
    if half <= 0.0 || !half.is_finite() {
        return 0.0;
    }
    // A line beside a taller one does not fall to half its own height before the
    // taller one lifts it again: real lines are Lorentzian, whose tails reach
    // far. The walk stops at the valley between them, or it measures the pair -
    // measured on the dataset in testdata, the weaker line of a doublet 6 Hz wide
    // came back as 16 Hz. A rise of a twentieth of the peak is a neighbour, not
    // noise on the flank.
    let rise = real[centre] / 20.0;
    let halves = half_widths(real, centre, half, rise, last);
    // A side stopped at the valley is not this line's own width at all, so the
    // other side stands for both. When neither was stopped, the two are averaged:
    // a neighbour near enough to matter lifts one flank so it reaches half height
    // late and leaves the other short, and the two errors are opposite. Measured
    // against lines whose width is known exactly - a Lorentzian of 6.00 Hz beside
    // a taller one, and a Gaussian doublet of 3.00 Hz - this reads 6.11 and 3.11,
    // where taking the narrower half read 0.00 and 2.48, and taking the wider read
    // 3.74. Counting both sides of a stopped line was what gave 8.7 Hz for a line
    // of about 3 on the dataset in testdata.
    let half_width = match (halves.left_stopped, halves.right_stopped) {
        (true, false) => halves.right,
        (false, true) => halves.left,
        _ => (halves.left + halves.right) / 2.0,
    };
    2.0 * half_width * axis.delta.abs() * obs_mhz
}

/// How far each side of a line reaches before it falls to half its height, in
/// points, and whether the walk was stopped by a neighbour rather than by the
/// line itself. A side that was stopped is not the line's own width.
#[derive(Debug, Clone, Copy)]
struct Halves {
    left: f64,
    right: f64,
    left_stopped: bool,
    right_stopped: bool,
}

fn half_widths(real: &[f64], centre: usize, half: f64, rise: f64, last: usize) -> Halves {
    let mut left = centre;
    let mut left_stopped = false;
    let mut low = real[centre];
    while left > 0 && real[left] > half {
        low = low.min(real[left]);
        if real[left - 1] > low + rise {
            left_stopped = true;
            break;
        }
        left -= 1;
    }
    let mut right = centre;
    let mut right_stopped = false;
    low = real[centre];
    while right < last && real[right] > half {
        low = low.min(real[right]);
        if real[right + 1] > low + rise {
            right_stopped = true;
            break;
        }
        right += 1;
    }
    // Where the line crosses half its height, between the points either side of
    // the crossing: rounding to whole points made a narrow line a third wider.
    let across = |outside: usize, inside: usize| {
        let (out, ins) = (real[outside], real[inside]);
        let step = ins - out;
        if step.abs() < f64::MIN_POSITIVE {
            outside as f64
        } else {
            outside as f64 + (half - out) / step * (inside as f64 - outside as f64)
        }
    };
    let from = if real[left] > half {
        left as f64
    } else {
        across(left, left + 1)
    };
    let to = if real[right] > half {
        right as f64
    } else {
        across(right, right - 1)
    };
    Halves {
        left: centre as f64 - from,
        right: to - centre as f64,
        left_stopped,
        right_stopped,
    }
}

/// Intensity at `ppm` by linear interpolation between the nearest points.
pub fn intensity_at(real: &[f64], axis: &PpmAxis, ppm: f64) -> f64 {
    let Some(last) = real.len().checked_sub(1) else {
        return 0.0;
    };
    let x = axis.index(ppm).clamp(0.0, last as f64);
    let i = (x.floor() as usize).min(last.saturating_sub(1));
    let t = x - i as f64;
    real[i] * (1.0 - t) + real[(i + 1).min(last)] * t
}

fn refine(real: &[f64], axis: &PpmAxis, i: usize) -> Peak {
    let y = real[i];
    let neighbours = i
        .checked_sub(1)
        .and_then(|l| Some((real[l], *real.get(i + 1)?)));
    let Some((left, right)) = neighbours else {
        return Peak {
            ppm: axis.ppm(i as f64),
            intensity: y,
        };
    };
    let curvature = left - 2.0 * y + right;
    let offset = if curvature < 0.0 {
        (0.5 * (left - right) / curvature).clamp(-0.5, 0.5)
    } else {
        0.0
    };
    Peak {
        ppm: axis.ppm(i as f64 + offset),
        intensity: y - 0.25 * (left - right) * offset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cross_peak_is_found_between_points_whatever_its_sign() {
        // A negative peak at column 12.3, row 7.6 of a 20 × 16 grid.
        let (cols, rows) = (20, 16);
        let real: Vec<f64> = (0..cols * rows)
            .map(|i| {
                let (r, c) = ((i / cols) as f64 - 7.6, (i % cols) as f64 - 12.3);
                -(-(r * r + c * c) / 4.0).exp()
            })
            .collect();
        let axis = |first: f64, delta: f64| PpmAxis { first, delta };
        let axes = [axis(10.0, -0.1), axis(150.0, -1.0)];
        let [f2, f1] = nearest_peak_2d(&real, cols, &axes, [8.9, 143.0], [3, 3]).unwrap();
        assert!((f2 - axes[0].ppm(12.3)).abs() < 0.01, "F2 {f2}");
        assert!((f1 - axes[1].ppm(7.6)).abs() < 0.1, "F1 {f1}");
        assert!(nearest_peak_2d(&real, cols, &axes, [50.0, 143.0], [3, 3]).is_none());
        // Each axis searches as far as its own radius: four points off along
        // F2 is reached with a radius of 5 there, and not with 1.
        let off = [axes[0].ppm(8.0), axes[1].ppm(8.0)];
        let [far, _] = nearest_peak_2d(&real, cols, &axes, off, [5, 1]).unwrap();
        assert!((far - axes[0].ppm(12.3)).abs() < 0.01, "F2 {far}");
        let [near, _] = nearest_peak_2d(&real, cols, &axes, off, [1, 5]).unwrap();
        assert!((near - axes[0].ppm(12.3)).abs() > 0.2, "F2 {near}");
    }

    const AXIS: PpmAxis = PpmAxis {
        first: 10.0,
        delta: -0.01,
    };

    fn lorentzian(n: usize, centre: f64, height: f64) -> Vec<f64> {
        (0..n)
            .map(|i| height / (1.0 + ((i as f64 - centre) / 2.0).powi(2)))
            .collect()
    }

    #[test]
    fn a_broad_line_is_picked_once_and_noise_not_at_all() {
        // Height says whether a line is there; width says only what shape it is.
        // Judged across a fixed few points, curvature stands in for sharpness and
        // a broad line is thrown away however tall it is - measured on the dataset
        // in testdata, one at 32 times the noise.
        let axis = PpmAxis {
            first: 10.0,
            delta: -0.0005,
        };
        let mut seed = 7u64;
        let mut rng = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((seed >> 33) as f64 / (1u64 << 31) as f64) - 0.5
        };
        let noise: Vec<f64> = (0..32768).map(|_| rng() * 2.0).collect();
        for half_width in [2.0_f64, 5.0, 10.0, 20.0, 40.0] {
            let mut real = noise.clone();
            for (i, v) in real.iter_mut().enumerate() {
                let d = (i as f64 - 16000.0) / half_width;
                *v += 100.0 / (1.0 + d * d);
            }
            let level = noise_level(&real);
            let found = find_peaks(
                &real,
                &axis,
                axis.ppm(15400.0),
                axis.ppm(16600.0),
                level * 5.0,
                3,
            );
            // One line is one peak, however wide: ripple riding on the flank of a
            // broad line measures its own small width and would pass on that, so
            // a maximum has to be the tallest thing within its own width too.
            assert_eq!(found.len(), 1, "half-width {half_width}: {found:?}");
        }
        // And nothing is picked out of noise that has no line in it.
        let level = noise_level(&noise);
        let none = find_peaks(
            &noise,
            &axis,
            axis.ppm(1000.0),
            axis.ppm(31000.0),
            level * 5.0,
            3,
        );
        assert!(none.is_empty(), "{} picked out of noise", none.len());
    }

    #[test]
    fn a_broad_weak_line_with_a_noisy_top_is_picked() {
        // A broad NH a few tens of times the noise: its flanks are ragged with
        // noise, and a walk down them that stops at the first dip measured it two
        // or three points wide, across which its curvature is only noise.
        let axis = PpmAxis {
            first: 12.0,
            delta: -0.0003,
        };
        let mut seed = 11u64;
        let mut rng = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((seed >> 33) as f64 / (1u64 << 31) as f64) - 0.5
        };
        let noise: Vec<f64> = (0..32768).map(|_| rng() * 2.0).collect();
        let sigma = noise_level(&noise);
        for half_width in [15.0_f64, 30.0, 60.0] {
            for height in [15.0, 25.0] {
                let mut real = noise.clone();
                for (i, v) in real.iter_mut().enumerate() {
                    let d = (i as f64 - 16000.0) / half_width;
                    *v += height * sigma / (1.0 + d * d);
                }
                let found = find_peaks(
                    &real,
                    &axis,
                    axis.ppm(15000.0),
                    axis.ppm(17000.0),
                    noise_level(&real) * 5.0,
                    3,
                );
                assert_eq!(
                    found.len(),
                    1,
                    "half-width {half_width}, {height} x noise: {:?}",
                    found.iter().map(|p| p.ppm).collect::<Vec<_>>()
                );
            }
        }
    }

    #[test]
    fn a_width_is_read_against_lines_whose_width_is_known() {
        // Shapes whose width can be worked out rather than measured, so the
        // reading has something to be right or wrong about: a Lorentzian of
        // half-width 2.5 points is 5 points across at half height, and
        // exp(-(d/1.5)^2) is 2 * 1.5 * sqrt(ln 2).
        let axis = PpmAxis {
            first: 10.0,
            delta: -0.002,
        };
        let obs = 600.0;
        let hz = |points: f64| 2.0 * points * axis.delta.abs() * obs;
        let shape = |i: f64, at: f64, height: f64| height / (1.0 + ((i - at) / 2.5).powi(2));
        let beside: Vec<f64> = (0..400)
            .map(|i| shape(i as f64, 200.0, 1000.0) + shape(i as f64, 205.0, 300.0))
            .collect();
        let alone: Vec<f64> = (0..400).map(|i| shape(i as f64, 205.0, 300.0)).collect();
        let gauss = hz(1.5 * (2.0_f64.ln()).sqrt());
        for (name, real, ppm, truth) in [
            (
                "a line beside a taller one",
                beside,
                axis.ppm(205.0),
                hz(2.5),
            ),
            ("the same line alone", alone, axis.ppm(205.0), hz(2.5)),
            (
                "a line of a doublet",
                lines(&axis, obs, 7.30, 7.8, 2),
                7.3 + 7.8 / 2.0 / obs,
                gauss,
            ),
            (
                "a line on its own",
                lines(&axis, obs, 4.00, 0.0, 1),
                4.00,
                gauss,
            ),
            (
                "the middle of a triplet",
                lines(&axis, obs, 5.00, 7.5, 3),
                5.00,
                gauss,
            ),
        ] {
            let got = line_width_hz(&real, &axis, obs, ppm);
            // Within a twentieth. What is left is the half-height crossing being
            // taken between points, which reads every line a little wide, the one
            // standing alone included.
            assert!(
                (got - truth).abs() < truth * 0.05,
                "{name}: {got:.2} Hz against {truth:.2}"
            );
        }
    }

    #[test]
    fn a_line_beside_a_taller_one_is_measured_on_its_own() {
        let axis = PpmAxis {
            first: 10.0,
            delta: -0.002,
        };
        let obs = 600.0;
        // Lorentzian, as a real line is: its tails reach much further than the
        // Gaussians the other tests are built from, which is what makes this
        // happen at all.
        let shape = |i: f64, at: f64, height: f64| height / (1.0 + ((i - at) / 2.5).powi(2));
        let pair: Vec<f64> = (0..400)
            .map(|i| shape(i as f64, 200.0, 1000.0) + shape(i as f64, 205.0, 300.0))
            .collect();
        let alone: Vec<f64> = (0..400).map(|i| shape(i as f64, 205.0, 300.0)).collect();
        let at = axis.ppm(205.0);
        let together = line_width_hz(&pair, &axis, obs, at);
        let by_itself = line_width_hz(&alone, &axis, obs, at);
        // Within a tenth of the line's own width, not merely "no wider": this
        // passed while the reading was 0.00 Hz, because nothing said it had to be
        // a width at all. The pair's inner flank is stopped at the valley, so the
        // outer side stands for both.
        assert!(
            (together - by_itself).abs() < by_itself * 0.1,
            "alone {by_itself}, beside {together}"
        );
    }

    #[test]
    fn noise_level_ignores_sparse_peaks() {
        let mut real: Vec<f64> = (0..10_000)
            .map(|i| if i % 2 == 0 { 3.0 } else { -3.0 })
            .collect();
        real[5000] = 1e6;
        real[7000] = 5e5;
        assert!((noise_level(&real) - 3.0 * MAD_TO_SIGMA).abs() < 1e-9);
        assert_eq!(noise_level(&[]), 0.0);
    }

    #[test]
    fn only_sharply_curved_maxima_are_peaks() {
        // Two real signals, plus a ripple riding on a broad hump: the ripple's
        // crests clear the threshold but do not stand above their surroundings.
        let mut real = lorentzian(2000, 500.0, 100.0);
        let second = lorentzian(2000, 1200.0, 60.0);
        real.iter_mut().zip(&second).for_each(|(a, b)| *a += b);
        for (i, v) in real.iter_mut().enumerate() {
            let x = i as f64;
            *v += 40.0 * (-((x - 1600.0) / 200.0).powi(2)).exp() + 2.0 * (x / 3.0).sin();
        }
        let maxima = local_maxima(&real, 0..real.len(), 10.0, 3);
        assert!(
            maxima.len() > 10,
            "the level alone accepts {}",
            maxima.len()
        );
        let peaks = find_peaks(&real, &AXIS, 10.0, -10.0, 10.0, 3);
        assert_eq!(peaks.len(), 2, "{peaks:?}");
    }

    #[test]
    fn integral_is_sum_times_spacing() {
        let real = vec![2.0; 1000];
        // 9.0 ppm is index 100, 8.0 ppm is index 200.
        assert!((integrate(&real, &AXIS, 8.0, 9.0) - 2.0 * 100.0 * 0.01).abs() < 1e-12);
        let curve = integral_curve(&real, &AXIS, 9.0, 8.0, 11);
        assert_eq!(curve.len(), 22);
        assert!((curve[0] - 9.0).abs() < 1e-6);
        assert!((curve[21] - integrate(&real, &AXIS, 9.0, 8.0) as f32).abs() < 1e-6);
        assert!(integral_curve(&real, &AXIS, 20.0, 15.0, 10).is_empty());
    }

    #[test]
    fn peaks_are_local_maxima_with_interpolated_position() {
        let mut real = lorentzian(1000, 300.3, 100.0);
        let second = lorentzian(1000, 600.0, 40.0);
        real.iter_mut().zip(&second).for_each(|(a, b)| *a += b);
        let peaks = find_peaks(&real, &AXIS, 10.0, 0.0, 10.0, 3);
        let positions: Vec<f64> = peaks.iter().map(|p| AXIS.index(p.ppm)).collect();
        assert_eq!(positions.len(), 2);
        // Parabolic interpolation on a Lorentzian 4 points wide is off by a few
        // hundredths of a point and reads the height about 1 % low.
        assert!((positions[0] - 300.3).abs() < 0.1, "{positions:?}");
        assert!((peaks[0].intensity - 100.0).abs() < 2.0, "{peaks:?}");
        assert!((positions[1] - 600.0).abs() < 1e-3, "{positions:?}");

        assert_eq!(find_peaks(&real, &AXIS, 10.0, 0.0, 60.0, 3).len(), 1);
        assert_eq!(find_peaks(&real, &AXIS, 10.0, 5.0, 10.0, 3).len(), 1);
        assert!(find_peaks(&real, &AXIS, 2.0, 0.0, 10.0, 3).is_empty());
    }

    #[test]
    fn nearest_peak_snaps_to_maximum() {
        let real = lorentzian(1000, 400.0, 50.0);
        let peak = nearest_peak(&real, &AXIS, AXIS.ppm(395.0), 8).unwrap();
        assert!((peak.ppm - AXIS.ppm(400.0)).abs() < 1e-9);
        assert_eq!(nearest_peak(&real, &AXIS, 50.0, 8), None);
    }

    #[test]
    fn intensity_interpolates_linearly() {
        let real = [0.0, 10.0, 20.0];
        assert!((intensity_at(&real, &AXIS, AXIS.ppm(1.5)) - 15.0).abs() < 1e-9);
        assert_eq!(intensity_at(&real, &AXIS, 99.0), 0.0);
        assert_eq!(intensity_at(&[], &AXIS, 1.0), 0.0);
    }

    #[test]
    fn integral_regions_follow_the_signals() {
        let axis = PpmAxis {
            first: 10.0,
            delta: -0.01,
        };
        let line = |i: f64, centre: f64| (-((i - centre) / 3.0).powi(2)).exp();
        let real: Vec<f64> = (0..200)
            .map(|i| line(i as f64, 50.0) + line(i as f64, 150.0))
            .collect();
        let regions = integral_regions(&real, &axis, 100.0, 10.0, 8.0, reading(0.1));
        assert_eq!(regions.len(), 2);
        // Each range reads high shift first, and the list reads down the spectrum.
        assert!(regions[0].0 > regions[0].1);
        assert!(regions[0].0 > regions[1].0);
        let centre = |r: (f64, f64)| (r.0 + r.1) / 2.0;
        assert!((centre(regions[0]) - axis.ppm(50.0)).abs() < 0.05);
        assert!((centre(regions[1]) - axis.ppm(150.0)).abs() < 0.05);
        // Wider than the run above the threshold: the skirts carry area too.
        assert!(regions[0].0 - regions[0].1 > 0.06);
    }

    #[test]
    fn integral_regions_use_only_accepted_peaks() {
        let real: Vec<f64> = (0..200)
            .map(|i| {
                (-((i as f64 - 50.0) / 3.0).powi(2)).exp()
                    + (-((i as f64 - 150.0) / 3.0).powi(2)).exp()
            })
            .collect();
        assert_eq!(
            integral_regions(&real, &AXIS, 100.0, 10.0, 8.0, reading(0.1)).len(),
            2
        );
        let accepted = AXIS.ppm(50.0);
        let regions =
            integral_regions_from_peaks(&real, &AXIS, 100.0, 10.0, 8.0, reading(0.1), &[accepted]);
        assert_eq!(regions.len(), 1, "{regions:?}");
        assert!(regions[0].0 >= accepted && regions[0].1 <= accepted);
        assert!(regions[0].1 > AXIS.ppm(150.0));
        assert!(
            integral_regions_from_peaks(&real, &AXIS, 100.0, 10.0, 8.0, reading(0.1), &[],)
                .is_empty()
        );
        assert_eq!(
            integral_regions_from_peaks(
                &real,
                &AXIS,
                100.0,
                10.0,
                8.0,
                reading(0.1),
                &[
                    accepted,
                    accepted + AXIS.delta * 0.1,
                    f64::NAN,
                    f64::INFINITY,
                    12.0,
                    7.0
                ],
            ),
            regions
        );
    }

    #[test]
    fn supplied_peak_regions_ignore_invalid_positions_on_flat_or_noisy_data() {
        for real in [
            vec![0.0; 200],
            (0..200).map(|i| 0.01 * (i as f64).sin()).collect(),
        ] {
            assert!(
                integral_regions_from_peaks(
                    &real,
                    &AXIS,
                    100.0,
                    10.0,
                    8.0,
                    reading(0.1),
                    &[f64::NAN, f64::NEG_INFINITY, 10.1, 7.9],
                )
                .is_empty()
            );
            assert!(
                integral_regions_from_peaks(&real, &AXIS, 100.0, 9.8, 9.5, reading(0.1), &[9.0],)
                    .is_empty()
            );
        }
    }

    fn reading(threshold: f64) -> Reading {
        Reading {
            threshold,
            separation: 3,
            max_j_hz: 20.0,
        }
    }

    #[test]
    fn integral_regions_part_signals_at_the_valley_and_keep_a_multiplet_whole() {
        let (axis, mhz) = (
            PpmAxis {
                first: 10.0,
                delta: -0.001,
            },
            400.0,
        );
        // A doublet 8 Hz across, and a broad singlet 30 Hz from it, whose reach
        // alone would take it well into the doublet.
        let lorentz = |i: f64, at: f64, half: f64| 1.0 / (1.0 + ((i - at) / half).powi(2));
        let at = |hz: f64| axis.index(9.0 - hz / mhz);
        let real: Vec<f64> = (0..2000)
            .map(|i| {
                let i = i as f64;
                lorentz(i, at(-4.0), 2.0)
                    + lorentz(i, at(4.0), 2.0)
                    + 0.5 * lorentz(i, at(34.0), 4.0)
            })
            .collect();
        let regions = integral_regions(&real, &axis, mhz, 10.0, 8.0, reading(0.01));
        assert_eq!(regions.len(), 2, "{regions:?}");
        // Neither crosses the lowest point between the two.
        let (a, b) = (at(4.0).round() as usize, at(34.0).round() as usize);
        let lowest = (a..=b)
            .min_by(|&i, &j| real[i].total_cmp(&real[j]))
            .unwrap();
        let valley = axis.ppm(lowest as f64);
        assert!(
            regions[0].1 >= valley - 0.0015 && regions[1].0 <= valley + 1e-9,
            "{regions:?} {valley}"
        );
    }

    /// Lines `spacing_hz` apart, centred on `centre`, as a multiplet would be.
    fn lines(axis: &PpmAxis, obs_mhz: f64, centre: f64, spacing_hz: f64, count: usize) -> Vec<f64> {
        let step = spacing_hz / obs_mhz;
        let first = centre - step * (count as f64 - 1.0) / 2.0;
        let mut real = vec![0.0; 8000];
        for k in 0..count {
            let at = axis.index(first + step * k as f64);
            for (i, v) in real.iter_mut().enumerate() {
                let d = (i as f64 - at) / 1.5;
                *v += (-d * d).exp();
            }
        }
        real
    }

    /// Lines as successive splitting makes them: every coupling splits every line
    /// in two and lines that land together add up, which is what makes 1:2:1 and
    /// 1:3:3:1 without either being written down. `lines` above cannot build these
    /// - it makes lines of equal height, which is a different multiplet.
    fn split_lines(axis: &PpmAxis, obs_mhz: f64, centre: f64, js_hz: &[f64]) -> Vec<f64> {
        let mut lines: Vec<(f64, f64)> = vec![(centre, 1.0)];
        for j in js_hz {
            let half = j / 2.0 / obs_mhz;
            let mut split: Vec<(f64, f64)> = Vec::new();
            for (ppm, weight) in &lines {
                for side in [-1.0, 1.0] {
                    let at = ppm + side * half;
                    match split.iter_mut().find(|(p, _)| (p - at).abs() < 1e-9) {
                        Some(same) => same.1 += weight / 2.0,
                        None => split.push((at, weight / 2.0)),
                    }
                }
            }
            lines = split;
        }
        let tallest = lines.iter().map(|l| l.1).fold(0.0, f64::max);
        let mut real = vec![0.0; 8000];
        for (ppm, weight) in lines {
            let at = axis.index(ppm);
            for (i, v) in real.iter_mut().enumerate() {
                let d = (i as f64 - at) / 1.5;
                *v += (weight / tallest) * (-d * d).exp();
            }
        }
        real
    }

    #[test]
    fn the_integral_curve_runs_from_one_edge_to_the_whole_area() {
        // What the curve promises, which is more than what it draws: pairs of
        // ppm and area so far, counted from the `from` edge, ending at the area
        // of the whole range. The plot reads the last of them as the total and
        // reads along it to say what a part of the range comes to while an end is
        // being dragged, so a curve that stopped being cumulative would still
        // look like a curve and quietly give the wrong number.
        let axis = PpmAxis {
            first: 10.0,
            delta: -0.001,
        };
        let real = vec![1.0; 2000];
        let curve = integral_curve(&real, &axis, 9.0, 8.0, 256);
        assert_eq!(curve.len(), 512, "pairs of ppm and area");

        let total = integrate(&real, &axis, 9.0, 8.0);
        let last = curve[curve.len() - 1] as f64;
        assert!(
            (last - total).abs() < 1e-6 * total.max(1.0),
            "{last} against {total}"
        );

        // Counted from the `from` edge: the first sample is one point's worth, not
        // the whole area, and it only ever grows for a signal that is positive.
        assert!((curve[1] as f64) < total / 100.0, "starts at the from edge");
        for pair in curve.chunks(2).collect::<Vec<_>>().windows(2) {
            assert!(pair[1][1] >= pair[0][1], "area only grows: {:?}", pair);
        }
        // The shifts run from one end of the range to the other.
        assert!((curve[0] as f64 - 9.0).abs() < 0.01, "{}", curve[0]);
        assert!(
            (curve[curve.len() - 2] as f64 - 8.0).abs() < 0.01,
            "{}",
            curve[curve.len() - 2]
        );

        // Asking for more samples than there are points gives the points there are.
        let short = integral_curve(&real, &axis, 9.0, 8.995, 256);
        assert!(short.len() <= 2 * 6 && !short.is_empty(), "{}", short.len());
        // A range with nothing in it draws nothing rather than a point at zero.
        assert!(integral_curve(&[], &axis, 9.0, 8.0, 16).is_empty());
    }

    #[test]
    fn a_flat_topped_line_is_found_once() {
        // A line sitting exactly between two points has two points of equal
        // height, and every point of that tie passed the window test, so the line
        // was found twice and refined onto one shift. Two lines at one shift is
        // not a doublet, and it is what breaks a multiplet being read: the units
        // no longer add up to a first-order pattern.
        let axis = PpmAxis {
            first: 7.0,
            delta: -0.0002,
        };
        let mut real = vec![0.0; 8000];
        let at = 1000.5;
        for (i, v) in real.iter_mut().enumerate() {
            let d = (i as f64 - at) / 1.5;
            *v = (-d * d).exp();
        }
        assert_eq!(real[1000], real[1001], "the fixture has no flat top");
        let peaks = find_peaks(&real, &axis, 7.0, 5.0, 0.2, 3);
        assert_eq!(peaks.len(), 1, "{:?}", peaks);
        assert!((peaks[0].ppm - axis.ppm(at)).abs() < 1e-6, "{:?}", peaks[0]);
    }

    #[test]
    fn multiplets_are_read_first_order() {
        let axis = PpmAxis {
            first: 10.0,
            delta: -0.002,
        };
        let obs = 600.0;
        let reading = |max_j_hz: f64| Reading {
            threshold: 0.2,
            separation: 3,
            max_j_hz,
        };
        let doublet = lines(&axis, obs, 7.30, 7.8, 2);
        let m = multiplet(&doublet, &axis, obs, 7.5, 7.1, reading(20.0));
        assert_eq!(m.kind, "d");
        assert_eq!(m.lines, 2);
        // Its lines' own width, not the width of the pair: the two sit 7.8 Hz
        // apart, so anything near that would be measuring both at once. Measured,
        // each line of this doublet reads 2.48 Hz against 3.12 for the same line
        // standing alone - narrow by a fifth, because the inner half is cut off
        // at the valley and it is the inner half that gets doubled.
        let alone = line_width_hz(&lines(&axis, obs, 4.00, 0.0, 1), &axis, obs, 4.00);
        assert!(
            m.width_hz > alone * 0.7,
            "width {} against {alone}",
            m.width_hz
        );
        assert!(
            m.width_hz < alone * 1.1,
            "width {} against {alone}",
            m.width_hz
        );
        assert!((m.js_hz[0] - 7.8).abs() < 0.3);
        // The shift quoted is the middle of the lines, not of the range.
        assert!((m.ppm - 7.30).abs() < 0.01);

        let triplet = lines(&axis, obs, 5.00, 7.5, 3);
        let m = multiplet(&triplet, &axis, obs, 5.2, 4.8, reading(20.0));
        assert_eq!(m.kind, "t");
        assert!((m.js_hz[0] - 7.5).abs() < 0.3);

        // Four lines of the same height, evenly spaced, are not a quartet: a
        // quartet is 1:3:3:1. They are a dd whose larger coupling is twice the
        // smaller, which is what puts its four lines an even distance apart.
        let even_four = lines(&axis, obs, 2.00, 7.0, 4);
        let m = multiplet(&even_four, &axis, obs, 2.2, 1.8, reading(20.0));
        assert_eq!(m.kind, "dd");
        assert!((m.js_hz[0] - 14.0).abs() < 0.3, "{:?}", m.js_hz);
        assert!((m.js_hz[1] - 7.0).abs() < 0.3, "{:?}", m.js_hz);

        // A real quartet, built by splitting: one coupling three times, 1:3:3:1.
        let quartet = split_lines(&axis, obs, 2.00, &[7.0, 7.0, 7.0]);
        let m = multiplet(&quartet, &axis, obs, 2.2, 1.8, reading(20.0));
        assert_eq!(m.kind, "q");
        assert!((m.js_hz[0] - 7.0).abs() < 0.3, "{:?}", m.js_hz);

        // The classes the spacings alone could never tell apart, on an axis fine
        // enough to show them. Curvature is judged across `len / 8192` points, so
        // a fixture of 8000 points judges it between neighbours, where a line 1.6
        // Hz from the next is not a separate maximum at all; a real spectrum of
        // 65536 points judges it across 8. A tenth of the point spacing puts a 1.6
        // Hz coupling 13 points out, which is the spectrum these classes need.
        let fine = PpmAxis {
            first: 7.0,
            delta: -0.0002,
        };
        // A dt is one coupling once and a smaller one twice; a td is the same two
        // the other way round, and they are different multiplets.
        let dt = split_lines(&fine, obs, 6.00, &[8.0, 1.6, 1.6]);
        let m = multiplet(&dt, &fine, obs, 6.2, 5.8, reading(20.0));
        assert_eq!(m.kind, "dt", "J {:?}", m.js_hz);
        assert!(
            (m.js_hz[0] - 8.0).abs() < 0.3 && (m.js_hz[1] - 1.6).abs() < 0.3,
            "{:?}",
            m.js_hz
        );

        let td = split_lines(&fine, obs, 6.00, &[8.0, 8.0, 1.6]);
        let m = multiplet(&td, &fine, obs, 6.2, 5.8, reading(20.0));
        assert_eq!(m.kind, "td", "J {:?}", m.js_hz);
        assert!(
            (m.js_hz[0] - 8.0).abs() < 0.3 && (m.js_hz[1] - 1.6).abs() < 0.3,
            "{:?}",
            m.js_hz
        );

        // Three unequal couplings, none of them repeated: eight lines.
        let ddd = split_lines(&fine, obs, 6.00, &[12.0, 6.0, 2.0]);
        let m = multiplet(&ddd, &fine, obs, 6.2, 5.8, reading(20.0));
        assert_eq!(m.kind, "ddd", "J {:?}", m.js_hz);

        // Two couplings, each of them twice: nine lines, 1:2:1 of 1:2:1, which is
        // not a nonet however evenly the nine are spread.
        let tt = split_lines(&fine, obs, 6.00, &[9.0, 9.0, 2.5, 2.5]);
        let m = multiplet(&tt, &fine, obs, 6.2, 5.8, reading(20.0));
        assert_eq!(m.kind, "tt", "J {:?}", m.js_hz);

        // One line is a singlet, and nothing in range is an unexplained m.
        let singlet = lines(&axis, obs, 3.00, 0.0, 1);
        assert_eq!(
            multiplet(&singlet, &axis, obs, 3.2, 2.8, reading(20.0)).kind,
            "s"
        );
        // Two signals caught in one range are not a doublet, however tidy they
        // look: 40 Hz is not a proton coupling.
        let apart = lines(&axis, obs, 3.20, 40.0, 2);
        let m = multiplet(&apart, &axis, obs, 3.4, 3.0, reading(20.0));
        assert_eq!(m.kind, "m");
        assert!(m.js_hz.is_empty());
        // A nucleus whose couplings run larger reads the same lines as a doublet.
        assert_eq!(
            multiplet(&apart, &axis, obs, 3.4, 3.0, reading(300.0)).kind,
            "d"
        );

        // The width the spectrum shows, which a simulation is drawn with. The
        // lines here are exp(-(d/1.5)^2), half their height 1.25 points out.
        let one = lines(&axis, obs, 4.00, 0.0, 1);
        let width = line_width_hz(&one, &axis, obs, 4.00);
        assert!((width - 3.0).abs() < 0.3, "width {width}");

        let empty = vec![0.0; 4000];
        assert_eq!(
            multiplet(&empty, &axis, obs, 3.2, 2.8, reading(20.0)).kind,
            "m"
        );
    }
}
