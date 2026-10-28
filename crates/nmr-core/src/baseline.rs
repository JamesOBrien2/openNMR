//! Automatic baseline estimation for frequency-domain spectra.

const AIRPLS_MAX_ITERATIONS: usize = 20;
const AIRPLS_TOLERANCE: f64 = 0.001;
const AIRPLS_EDGE_FRACTION: f64 = 0.1;
const AIRPLS_EDGE_WEIGHT: f64 = 0.05;
const LAMBDA_REFERENCE_POINTS: f64 = 10_000.0;
const POLYNOMIAL_MAX_ITERATIONS: usize = 200;
const POLYNOMIAL_TOLERANCE: f64 = 1e-9;
/// Sigmas above the mean at which the power of the derivative stops being noise
/// (Dietrich et al.).
const DIETRICH_SIGMA: f64 = 3.0;
/// Neighbours either side the erosion looks at: Dietrich's rule is the two
/// adjacent points.
const COBAS_EROSION: usize = 1;
/// `cobas` takes its smoothing for this many points, scaled to the spectrum's own.
pub const COBAS_REFERENCE_POINTS: f64 = 131_072.0;
/// The wavelet's width in Hz used for signal recognition (80 points at
/// 131 072 points over 12 019 Hz). A signal broader than a few times this is
/// read as baseline and flattened, as the authors warn.
pub const COBAS_FILTER_HZ: f64 = 7.3;

/// Baseline by airPLS (Zhang, Chen & Liang, Analyst 135 (2010) 1138): a
/// second-order Whittaker smoother whose weights drop to zero for points above
/// the current baseline.
///
/// `lambda` is the smoothing for 10 000 points. The penalty grows with the fourth
/// power of the point density, so it is scaled by `(n / 10 000)^4` and zero filling
/// leaves the result unchanged.
pub fn airpls(y: &[f64], lambda: f64) -> Vec<f64> {
    let n = y.len();
    let total: f64 = y.iter().map(|v| v.abs()).sum();
    if n < 3 || total == 0.0 {
        return vec![0.0; n];
    }
    let lambda = lambda * (n as f64 / LAMBDA_REFERENCE_POINTS).powi(4);
    let len = n as f64;
    let head = (len * AIRPLS_EDGE_FRACTION).ceil() as usize;
    let tail = (len - len * AIRPLS_EDGE_FRACTION).floor() as usize;

    let mut weights = vec![1.0; n];
    let mut baseline = Vec::new();
    for iteration in 1..=AIRPLS_MAX_ITERATIONS {
        baseline = whittaker(y, &weights, lambda);
        let negative = -y
            .iter()
            .zip(&baseline)
            .map(|(v, b)| (v - b).min(0.0))
            .sum::<f64>();
        if negative < AIRPLS_TOLERANCE * total {
            break;
        }
        for (i, w) in weights.iter_mut().enumerate() {
            let d = y[i] - baseline[i];
            // Edge points 1..ceil(n*wep) and floor(n-n*wep)..n, shifted to 0-based.
            let edge = i < head || i + 1 >= tail;
            *w = if d < 0.0 {
                (iteration as f64 * -d / negative).exp()
            } else if edge {
                AIRPLS_EDGE_WEIGHT
            } else {
                0.0
            };
        }
    }
    baseline
}

/// Whittaker smoother with every weight 1: the single solve [`airpls`] iterates
/// on, with `lambda` scaled the same way. Useful on its own for a smooth trend,
/// and it is what the golden tests check the banded solver with, since the
/// airPLS iteration amplifies round-off.
pub fn smooth(y: &[f64], lambda: f64) -> Vec<f64> {
    let n = y.len();
    if n < 3 {
        return y.to_vec();
    }
    let lambda = lambda * (n as f64 / LAMBDA_REFERENCE_POINTS).powi(4);
    whittaker(y, &vec![1.0; n], lambda)
}

/// Solves `(W + lambda D'D) z = W y` with second-order differences `D`. The matrix
/// is symmetric pentadiagonal, so a banded LDLᵀ factorisation solves it in O(n).
fn whittaker(y: &[f64], weights: &[f64], lambda: f64) -> Vec<f64> {
    let n = y.len();
    let mut diag = weights.to_vec();
    let mut off1 = vec![0.0; n];
    let mut off2 = vec![0.0; n];
    for r in 0..n - 2 {
        diag[r] += lambda;
        diag[r + 1] += 4.0 * lambda;
        diag[r + 2] += lambda;
        off1[r] -= 2.0 * lambda;
        off1[r + 1] -= 2.0 * lambda;
        off2[r] += lambda;
    }

    // Unit lower L with l1[i] = L[i, i-1], l2[i] = L[i, i-2]; d is the diagonal of D.
    let mut d = vec![0.0; n];
    let mut l1 = vec![0.0; n];
    let mut l2 = vec![0.0; n];
    d[0] = diag[0];
    l1[1] = off1[0] / d[0];
    d[1] = diag[1] - l1[1] * l1[1] * d[0];
    for i in 2..n {
        l2[i] = off2[i - 2] / d[i - 2];
        l1[i] = (off1[i - 1] - l2[i] * d[i - 2] * l1[i - 1]) / d[i - 1];
        d[i] = diag[i] - l1[i] * l1[i] * d[i - 1] - l2[i] * l2[i] * d[i - 2];
    }

    let mut z: Vec<f64> = y.iter().zip(weights).map(|(v, w)| v * w).collect();
    z[1] -= l1[1] * z[0];
    for i in 2..n {
        z[i] -= l1[i] * z[i - 1] + l2[i] * z[i - 2];
    }
    for (zi, di) in z.iter_mut().zip(&d) {
        *zi /= di;
    }
    z[n - 2] -= l1[n - 1] * z[n - 1];
    for i in (0..n - 2).rev() {
        z[i] -= l1[i + 1] * z[i + 1] + l2[i + 2] * z[i + 2];
    }
    z
}

/// The Whittaker baseline's filter is at least this many times the width at
/// half height of the tallest line. The wavelet reads the slow wings of a line
/// far wider than itself as baseline, and the baseline then climbed into them:
/// on the ⁸⁷Rb in `testdata` (212 Hz) the 7.3 Hz filter kept a third of the
/// line's area, 1.75 times its width all of it (wider again, the recognition
/// breaks down: 1000 Hz took most of it off). Every ¹H and ¹³C there has its
/// tallest line 0.8 to 4.1 Hz wide, which leaves the filter as it was.
pub const BROAD_LINE_FILTER: f64 = 1.75;

/// Width at half height, in points, of the tallest line of `y`.
pub fn tallest_width(y: &[f64]) -> usize {
    let Some((top, &height)) = y.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)) else {
        return 0;
    };
    if height <= 0.0 {
        return 0;
    }
    let half = height / 2.0;
    let left = (0..top).rev().find(|&i| y[i] <= half).unwrap_or(0);
    let right = (top..y.len())
        .find(|&i| y[i] <= half)
        .unwrap_or(y.len() - 1);
    right - left
}

/// Automatic filter width in points, including weaker positive and negative lines.
/// A significant, localized tallest line still protects broad nuclei; additional
/// lines must pass the peak picker's curvature checks.
/// shortcut: weaker lines inherit its 0.25 ppm half-width limit, extend this when
/// weaker broad-nucleus reference spectra are available.
/// The tallest-line safeguard needs both flanks and a width below an eighth of
/// the spectrum, so a slow drift or an end bend cannot set the filter width.
/// This extension is a local safeguard, not part of the published algorithm.
pub fn signal_width(y: &[f64], axis: &crate::PpmAxis) -> f64 {
    if y.len() < 4 || !axis.delta.is_finite() || axis.delta == 0.0 {
        return tallest_width(y) as f64;
    }
    let mut centred = y.to_vec();
    let middle = centred.len() / 2;
    let offset = *centred.select_nth_unstable_by(middle, f64::total_cmp).1;
    for (v, source) in centred.iter_mut().zip(y) {
        *v = source - offset;
    }
    let threshold = 5.0 * crate::analysis::noise_level(&centred);
    let mut width = 0.0_f64;
    for negative in [false, true] {
        if negative {
            centred.iter_mut().for_each(|v| *v = -*v);
        }
        let tallest = tallest_width(&centred);
        let height = centred.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        // A feature occupying most of the axis gives too little quiet baseline to widen safely.
        if height > threshold
            && tallest < y.len() / 8
            && centred[0] <= height / 2.0
            && centred[y.len() - 1] <= height / 2.0
        {
            width = width.max(tallest as f64);
        }
        for peak in crate::analysis::find_peaks(
            &centred,
            axis,
            axis.ppm(0.0),
            axis.ppm(y.len() as f64),
            threshold,
            1,
        ) {
            let candidate =
                crate::analysis::line_width_hz(&centred, axis, 1.0, peak.ppm) / axis.delta.abs();
            width = width.max(candidate);
        }
    }
    width
}

/// Baseline after Cobas, Bernstein, Martín-Pastor & Tahoces, J. Magn. Reson. 183
/// (2006) 145.
///
/// Signal-free points are recognised first: the derivative is taken by
/// convolution with a Haar wavelet of `dilation` points (their Eq. 4), squared
/// into a power spectrum, and thresholded as Dietrich, Rüdel & Neumann do
/// (J. Magn. Reson. 91 (1991) 1): the mean plus three standard deviations,
/// recomputed from the points below it until no further point is excluded. A
/// baseline point whose neighbours are mostly signal becomes signal (their
/// erosion). That mask is the weight vector of a Whittaker smoother with
/// first-order differences (Eq. 10), so across a signal the baseline is carried
/// straight from one side to the other.
///
/// `lambda` is the smoothing for `COBAS_REFERENCE_POINTS` points: a first-order
/// penalty grows with the square of the point density, so it is scaled by
/// `(n / 131 072)^2` and zero filling leaves the result unchanged. `dilation` is
/// the wavelet's width in points. On the dataset in `testdata` (131 072 points,
/// 12 kHz), `lambda` 1e5 and a dilation of 80 points keep the fitted baseline
/// within 0.7 times the noise, where the masked fit this replaces was 3.7 times
/// off and airPLS 8.6.
pub fn cobas(y: &[f64], lambda: f64, dilation: usize) -> Vec<f64> {
    let n = y.len();
    if n < 3 {
        return vec![0.0; n];
    }
    let lambda = lambda * (n as f64 / COBAS_REFERENCE_POINTS).powi(2);
    let weights = free_of_signal(y, lambda, dilation.max(2));
    if weights.iter().all(|w| *w == 0.0) {
        return vec![0.0; n];
    }
    whittaker_first(y, &weights, lambda)
}

/// [`recognise`], with the smooth stretches at the ends made baseline too
/// ([`take_smooth_ends`]); `lambda` already scaled to the point count.
fn free_of_signal(y: &[f64], lambda: f64, dilation: usize) -> Vec<f64> {
    let mut weights = recognise(y, dilation);
    if weights.iter().any(|w| *w != 0.0) {
        let first = whittaker_first(y, &weights, lambda);
        take_smooth_ends(y, &first, &mut weights, dilation);
    }
    weights
}

/// How far, in the noise of the baseline, a cubic may miss an end stretch that
/// is to count as baseline. The bends in `testdata` miss by 0.1 to 2.2; a line
/// tens of times the noise misses by far more.
const SMOOTH_END_NOISE: f64 = 3.0;
/// The weight of an end stretch taken for baseline: the bend is steep, and
/// with the weight of the rest the smoother still lagged it by six times the
/// noise in the last few hundred points.
const SMOOTH_END_WEIGHT: f64 = 30.0;

/// The stretch from each end of the spectrum to the first run of baseline, if
/// a cubic follows it to within the baseline's own noise, made baseline too.
/// The digital filter bends the ends of a Bruker spectrum ("smiles") steeply
/// enough for the wavelet to read them as signal, and the smoother then held
/// the baseline flat across them; a line at the end leaves a cubic far off.
fn take_smooth_ends(y: &[f64], fitted: &[f64], weights: &mut [f64], run: usize) {
    let n = y.len();
    let free: Vec<usize> = (0..n).filter(|&i| weights[i] != 0.0).collect();
    let noise = (free
        .iter()
        .map(|&i| (y[i] - fitted[i]).powi(2))
        .sum::<f64>()
        / free.len().max(1) as f64)
        .sqrt();
    // Where the first run of `run` baseline points starts, from either end.
    let starts_run = |i: usize, step: isize| {
        (0..run).all(|k| {
            let j = i as isize + step * k as isize;
            j >= 0 && (j as usize) < n && weights[j as usize] != 0.0
        })
    };
    let left = (0..n).find(|&i| starts_run(i, 1));
    let right = (0..n).rev().find(|&i| starts_run(i, -1));
    for (from, to) in [(0, left.unwrap_or(0)), (right.map_or(n, |r| r + 1), n)] {
        // Short ends need nothing, long ones are more than a bend.
        if to <= from + run || to - from > n / 8 {
            continue;
        }
        // Overlapping the baseline it joins by a run, so the cubic meets it.
        let (a, b) = (from.saturating_sub(run), (to + run).min(n));
        let anchors: Vec<(f64, f64)> = (a..b).map(|i| ((i - a) as f64, y[i])).collect();
        let cubic = fit_polynomial(b - a, &anchors, 3);
        let miss =
            ((a..b).map(|i| (y[i] - cubic[i - a]).powi(2)).sum::<f64>() / (b - a) as f64).sqrt();
        if miss <= SMOOTH_END_NOISE * noise {
            weights[from..to]
                .iter_mut()
                .for_each(|w| *w = SMOOTH_END_WEIGHT);
        }
    }
}

/// One for a signal-free point, zero for a point of a signal.
fn recognise(y: &[f64], dilation: usize) -> Vec<f64> {
    let n = y.len();
    // Haar: +1 over the first half of the wavelet, -1 over the second, so the
    // convolution is the difference of two running sums.
    let half = dilation / 2;
    let mut sums = Vec::with_capacity(n + 1);
    sums.push(0.0);
    for v in y {
        sums.push(sums[sums.len() - 1] + v);
    }
    let sum = |from: isize, to: isize| {
        let clamp = |i: isize| i.clamp(0, n as isize) as usize;
        sums[clamp(to)] - sums[clamp(from)]
    };
    let power: Vec<f64> = (0..n as isize)
        .map(|i| {
            let h = half as isize;
            let d = sum(i, i + h) - sum(i - h, i);
            d * d
        })
        .collect();

    let mut limit = f64::INFINITY;
    loop {
        let kept: Vec<f64> = power.iter().copied().filter(|p| *p <= limit).collect();
        let count = kept.len() as f64;
        let mean = kept.iter().sum::<f64>() / count;
        let sd = (kept.iter().map(|p| (p - mean).powi(2)).sum::<f64>() / count).sqrt();
        let next = mean + DIETRICH_SIGMA * sd;
        if next >= limit || kept.len() == power.iter().filter(|p| **p <= next).count() {
            limit = next;
            break;
        }
        limit = next;
    }
    let baseline: Vec<bool> = power.iter().map(|p| *p <= limit).collect();
    (0..n)
        .map(|i| {
            let from = i.saturating_sub(COBAS_EROSION);
            let to = (i + COBAS_EROSION + 1).min(n);
            let others = to - from - 1;
            let signal = (from..to).filter(|&j| j != i && !baseline[j]).count();
            if baseline[i] && 2 * signal <= others {
                1.0
            } else {
                0.0
            }
        })
        .collect()
}

/// Solves `(W + lambda D'D) z = W y` with first-order differences `D`, which is
/// tridiagonal (Cobas et al. Eq. 10).
fn whittaker_first(y: &[f64], weights: &[f64], lambda: f64) -> Vec<f64> {
    let n = y.len();
    let mut diag: Vec<f64> = weights.to_vec();
    for r in 0..n - 1 {
        diag[r] += lambda;
        diag[r + 1] += lambda;
    }
    let off = -lambda;
    // Thomas algorithm on the symmetric tridiagonal system.
    let mut c = vec![0.0; n];
    let mut z: Vec<f64> = y.iter().zip(weights).map(|(v, w)| v * w).collect();
    c[0] = off / diag[0];
    z[0] /= diag[0];
    for i in 1..n {
        let m = diag[i] - off * c[i - 1];
        if i + 1 < n {
            c[i] = off / m;
        }
        z[i] = (z[i] - off * z[i - 1]) / m;
    }
    for i in (0..n - 1).rev() {
        z[i] -= c[i] * z[i + 1];
    }
    z
}

/// Baseline by iterative polynomial fitting (modpoly; Lieber & Mahadevan-Jansen,
/// Appl. Spectrosc. 57 (2003) 1363): fit, lower the data to the fit wherever it lies
/// above it, and refit until the fit stops changing.
pub fn polynomial(y: &[f64], order: usize) -> Vec<f64> {
    let n = y.len();
    if n == 0 {
        return Vec::new();
    }
    let basis = orthonormal_legendre(n, order.min(n - 1));
    let mut work = y.to_vec();
    let mut fit = project(&basis, &work);
    for _ in 0..POLYNOMIAL_MAX_ITERATIONS {
        for (w, f) in work.iter_mut().zip(&fit) {
            *w = w.min(*f);
        }
        let next = project(&basis, &work);
        let change = next
            .iter()
            .zip(&fit)
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f64>()
            .sqrt();
        let size = dot(&next, &next).sqrt();
        fit = next;
        if change <= POLYNOMIAL_TOLERANCE * size {
            break;
        }
    }
    fit
}

/// The grid the Whittaker smoother through points is solved on.
const WHITTAKER_POINTS_GRID: usize = 2048;

/// How a baseline is drawn through points set by hand: the first two pass
/// through every point, the last two are fitted to them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Through {
    Segments,
    Spline,
    Polynomial(usize),
    /// Smoothness for 10 000 points, scaled as [`smooth`]'s.
    Whittaker(f64),
}

/// The points a hand-set baseline goes through, `(index, value)` sorted by
/// index, one per index. Each `(index, value)` asked for takes `value` when it
/// is given (a point placed freely) and otherwise the mean of `y` over `span`
/// points either side, so noise does not decide where the baseline runs.
pub fn anchors(y: &[f64], points: &[(usize, Option<f64>)], span: usize) -> Vec<(f64, f64)> {
    let n = y.len();
    if n == 0 {
        return Vec::new();
    }
    let mut out: Vec<(f64, f64)> = points
        .iter()
        .map(|&(i, value)| {
            let i = i.min(n - 1);
            let v = value.unwrap_or_else(|| {
                let (a, b) = (i.saturating_sub(span), (i + span + 1).min(n));
                y[a..b].iter().sum::<f64>() / (b - a) as f64
            });
            (i as f64, v)
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

/// Baseline through `anchors` (from [`anchors`]) over `n` points. Beyond the
/// outermost anchors segments and splines hold their value, since nothing
/// there says which way it would go; the polynomial and the Whittaker smoother
/// carry on as they are. No anchor is no baseline, one is a constant.
pub fn through_points(n: usize, anchors: &[(f64, f64)], through: Through) -> Vec<f64> {
    match anchors {
        [] => return vec![0.0; n],
        [(_, v)] => return vec![*v; n],
        _ => {}
    }
    match through {
        Through::Segments => (0..n)
            .map(|i| {
                let x = i as f64;
                let k = anchors
                    .partition_point(|a| a.0 <= x)
                    .clamp(1, anchors.len() - 1);
                let ((x0, y0), (x1, y1)) = (anchors[k - 1], anchors[k]);
                y0 + (y1 - y0) * ((x - x0) / (x1 - x0)).clamp(0.0, 1.0)
            })
            .collect(),
        Through::Spline => natural_spline(n, anchors),
        Through::Polynomial(order) => fit_polynomial(n, anchors, order.min(anchors.len() - 1)),
        Through::Whittaker(lambda) => {
            // Solved on a coarse grid and carried across: with weight on a few
            // dozen points of 131k, a stiff smoothness scaled to the full count
            // reached 1e12 and the factorisation came apart (a sine wave, or
            // zero). The baseline through points is smooth over hundreds of
            // points, so the grid loses nothing of it.
            let m = n.min(WHITTAKER_POINTS_GRID);
            let to_grid = |x: f64| (x * (m - 1) as f64 / (n - 1).max(1) as f64).round() as usize;
            let mut weights = vec![0.0; m];
            let mut values = vec![0.0; m];
            for &(x, v) in anchors {
                let g = to_grid(x);
                values[g] = (values[g] * weights[g] + v) / (weights[g] + 1.0);
                weights[g] += 1.0;
            }
            let lambda = lambda * (m as f64 / LAMBDA_REFERENCE_POINTS).powi(4);
            let coarse = whittaker(&values, &weights, lambda);
            let grid: Vec<(f64, f64)> = coarse
                .iter()
                .enumerate()
                .map(|(g, v)| (g as f64 * (n - 1) as f64 / (m - 1).max(1) as f64, *v))
                .collect();
            through_points(n, &grid, Through::Segments)
        }
    }
}

/// Natural cubic spline through `anchors`, held flat beyond them.
fn natural_spline(n: usize, anchors: &[(f64, f64)]) -> Vec<f64> {
    let m = anchors.len();
    let (x, y): (Vec<f64>, Vec<f64>) = anchors.iter().copied().unzip();
    let h: Vec<f64> = x.windows(2).map(|w| w[1] - w[0]).collect();
    // Second derivatives, zero at both ends, by the tridiagonal (Thomas) solve.
    let mut second = vec![0.0; m];
    if m > 2 {
        let mut c = vec![0.0; m];
        let mut d = vec![0.0; m];
        for i in 1..m - 1 {
            let rhs = 6.0 * ((y[i + 1] - y[i]) / h[i] - (y[i] - y[i - 1]) / h[i - 1]);
            let diag = 2.0 * (h[i - 1] + h[i]) - h[i - 1] * c[i - 1];
            c[i] = h[i] / diag;
            d[i] = (rhs - h[i - 1] * d[i - 1]) / diag;
        }
        for i in (1..m - 1).rev() {
            second[i] = d[i] - c[i] * second[i + 1];
        }
    }
    (0..n)
        .map(|i| {
            let t = (i as f64).clamp(x[0], x[m - 1]);
            let k = x.partition_point(|&a| a <= t).clamp(1, m - 1) - 1;
            let (a, b) = ((x[k + 1] - t) / h[k], (t - x[k]) / h[k]);
            a * y[k]
                + b * y[k + 1]
                + ((a.powi(3) - a) * second[k] + (b.powi(3) - b) * second[k + 1]) * h[k] * h[k]
                    / 6.0
        })
        .collect()
}

/// Least-squares polynomial of `order` through `anchors`, in Legendre
/// polynomials over the whole spectrum so a high order stays well conditioned.
fn fit_polynomial(n: usize, anchors: &[(f64, f64)], order: usize) -> Vec<f64> {
    let scale = |x: f64| {
        if n > 1 {
            2.0 * x / (n - 1) as f64 - 1.0
        } else {
            0.0
        }
    };
    let legendre = |x: f64| {
        let mut p = vec![1.0, x];
        for k in 1..order {
            let k_f = k as f64;
            p.push(((2.0 * k_f + 1.0) * x * p[k] - k_f * p[k - 1]) / (k_f + 1.0));
        }
        p.truncate(order + 1);
        p
    };
    let size = order + 1;
    // Normal equations, solved by Gaussian elimination with partial pivoting.
    let mut a = vec![vec![0.0; size + 1]; size];
    for &(x, v) in anchors {
        let p = legendre(scale(x));
        for r in 0..size {
            for c in 0..size {
                a[r][c] += p[r] * p[c];
            }
            a[r][size] += p[r] * v;
        }
    }
    for col in 0..size {
        let pivot = (col..size)
            .max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))
            .unwrap_or(col);
        a.swap(col, pivot);
        if a[col][col] == 0.0 {
            continue;
        }
        let (top, rest) = a.split_at_mut(col + 1);
        let pivot_row = &top[col];
        for row in rest {
            let f = row[col] / pivot_row[col];
            row[col..]
                .iter_mut()
                .zip(&pivot_row[col..])
                .for_each(|(x, p)| *x -= f * p);
        }
    }
    let mut coef = vec![0.0; size];
    for r in (0..size).rev() {
        let s: f64 = (r + 1..size).map(|c| a[r][c] * coef[c]).sum();
        coef[r] = if a[r][r] == 0.0 {
            0.0
        } else {
            (a[r][size] - s) / a[r][r]
        };
    }
    (0..n)
        .map(|i| dot(&legendre(scale(i as f64)), &coef))
        .collect()
}

/// Points to start a hand-set baseline from: about `count` across the
/// spectrum, spread through the stretches [`cobas`] with the same `lambda` and
/// `dilation` takes for baseline, the bends at the ends included; a stretch
/// shorter than the spacing gets one, in its middle.
pub fn propose_points(y: &[f64], lambda: f64, dilation: usize, count: usize) -> Vec<usize> {
    let n = y.len();
    if n < 3 || count == 0 {
        return Vec::new();
    }
    let lambda = lambda * (n as f64 / COBAS_REFERENCE_POINTS).powi(2);
    let mut free: Vec<bool> = free_of_signal(y, lambda, dilation.max(2))
        .iter()
        .map(|w| *w != 0.0)
        .collect();
    // A few points of noise over the threshold do not make a line: close gaps
    // narrower than the wavelet, so a stretch is broken only by a signal.
    let mut last = None;
    for i in 0..n {
        if free[i] {
            if let Some(l) = last
                && i - l > 1
                && i - l <= dilation
            {
                free[l..i].iter_mut().for_each(|f| *f = true);
            }
            last = Some(i);
        }
    }
    let step = (n / count).max(1);
    // A stretch narrower than this is a gap between two lines, not baseline.
    let shortest = (step / 4).max(dilation);
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        if !free[i] {
            i += 1;
            continue;
        }
        let start = i;
        while i < n && free[i] {
            i += 1;
        }
        let (a, b) = (start, i - 1);
        if b - a + 1 < shortest {
            continue;
        }
        // In from the ends, where the stretch meets a line's foot.
        let inset = ((b - a) / 10).min(dilation);
        let (a, b) = (a + inset, b - inset);
        // A stretch shorter than the spacing gets one point, in its middle: two
        // close together, each with its own noise, set a spline swinging.
        if b - a < step {
            out.push((a + b) / 2);
            continue;
        }
        let parts = (b - a) / step;
        out.extend((0..=parts).map(|k| a + (b - a) * k / parts));
    }
    // Nor closer than half the spacing across a line.
    let mut kept: Vec<usize> = Vec::with_capacity(out.len());
    for p in out {
        if kept.last().is_none_or(|&last| p >= last + step / 2) {
            kept.push(p);
        }
    }
    kept
}

/// Legendre polynomials on [-1, 1], orthonormalised over the sample points
/// (Gram-Schmidt twice, for stability at high order).
fn orthonormal_legendre(n: usize, order: usize) -> Vec<Vec<f64>> {
    let step = if n > 1 { 2.0 / (n - 1) as f64 } else { 0.0 };
    let x: Vec<f64> = (0..n).map(|i| i as f64 * step - 1.0).collect();
    let mut polys = vec![vec![1.0; n], x.clone()];
    for k in 1..order {
        let k_f = k as f64;
        let next = (0..n)
            .map(|i| ((2.0 * k_f + 1.0) * x[i] * polys[k][i] - k_f * polys[k - 1][i]) / (k_f + 1.0))
            .collect();
        polys.push(next);
    }
    polys.truncate(order + 1);

    let mut basis: Vec<Vec<f64>> = Vec::with_capacity(polys.len());
    for mut v in polys {
        for _ in 0..2 {
            for q in &basis {
                let c = dot(q, &v);
                v.iter_mut().zip(q).for_each(|(vi, qi)| *vi -= c * qi);
            }
        }
        let norm = dot(&v, &v).sqrt();
        if norm > 0.0 {
            v.iter_mut().for_each(|vi| *vi /= norm);
            basis.push(v);
        }
    }
    basis
}

fn project(basis: &[Vec<f64>], y: &[f64]) -> Vec<f64> {
    let mut fit = vec![0.0; y.len()];
    for q in basis {
        let c = dot(q, y);
        fit.iter_mut().zip(q).for_each(|(f, qi)| *f += c * qi);
    }
    fit
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    /// Lorentzian peaks on a smooth baseline, with small deterministic noise.
    fn spectrum(n: usize, baseline: impl Fn(f64) -> f64) -> (Vec<f64>, Vec<f64>) {
        let peaks = [(0.2, 1000.0), (0.45, 600.0), (0.47, 600.0), (0.8, 1500.0)];
        let base: Vec<f64> = (0..n).map(|i| baseline(i as f64 / n as f64)).collect();
        let y = (0..n)
            .map(|i| {
                let x = i as f64;
                let signal: f64 = peaks
                    .iter()
                    .map(|(c, h)| h / (1.0 + ((x - c * n as f64) / 3.0).powi(2)))
                    .sum();
                let noise = ((i * 7919 % 1000) as f64 / 1000.0 - 0.5) * 2.0;
                base[i] + signal + noise
            })
            .collect();
        (y, base)
    }

    /// Largest baseline error away from the peaks.
    fn worst_error(estimate: &[f64], truth: &[f64]) -> f64 {
        let n = truth.len() as f64;
        let near_peak = |i: usize| {
            [0.2, 0.46, 0.8]
                .iter()
                .any(|c| (i as f64 - c * n).abs() < 0.05 * n)
        };
        (0..truth.len())
            .filter(|&i| !near_peak(i))
            .map(|i| (estimate[i] - truth[i]).abs())
            .fold(0.0, f64::max)
    }

    #[test]
    fn whittaker_matches_dense_solve() {
        let y = [3.0, -1.0, 4.0, 1.0, -5.0, 9.0, 2.0];
        let w = [1.0, 0.0, 0.5, 2.0, 0.0, 1.0, 0.3];
        let lambda = 2.5;
        let n = y.len();
        let mut a = vec![vec![0.0; n + 1]; n];
        for i in 0..n {
            a[i][i] = w[i];
            a[i][n] = w[i] * y[i];
        }
        for r in 0..n - 2 {
            let row = [(r, 1.0), (r + 1, -2.0), (r + 2, 1.0)];
            for &(i, ci) in &row {
                for &(j, cj) in &row {
                    a[i][j] += lambda * ci * cj;
                }
            }
        }
        for p in 0..n {
            let pivot = a[p].clone();
            for row in &mut a[p + 1..] {
                let f = row[p] / pivot[p];
                row[p..]
                    .iter_mut()
                    .zip(&pivot[p..])
                    .for_each(|(x, y)| *x -= f * y);
            }
        }
        let mut expected = vec![0.0; n];
        for i in (0..n).rev() {
            let s: f64 = (i + 1..n).map(|j| a[i][j] * expected[j]).sum();
            expected[i] = (a[i][n] - s) / a[i][i];
        }
        for (z, e) in whittaker(&y, &w, lambda).iter().zip(&expected) {
            assert!((z - e).abs() < 1e-10, "{z} != {e}");
        }
    }

    #[test]
    fn airpls_follows_rolling_baseline() {
        let (y, truth) = spectrum(4000, |x| 60.0 * (3.0 * PI * x).sin() + 40.0 * x * x);
        let estimate = airpls(&y, 1e7);
        assert!(worst_error(&estimate, &truth) < 6.0);
        let peak = y[3200] - estimate[3200];
        assert!((peak - 1500.0).abs() < 30.0, "peak height {peak}");
    }

    #[test]
    fn airpls_is_independent_of_zero_filling() {
        let (y, _) = spectrum(2000, |x| 50.0 * (2.0 * PI * x).cos());
        let (y2, _) = spectrum(4000, |x| 50.0 * (2.0 * PI * x).cos());
        let (a, b) = (airpls(&y, 1e7), airpls(&y2, 1e7));
        let worst = (0..2000)
            .map(|i| (a[i] - b[2 * i]).abs())
            .fold(0.0, f64::max);
        assert!(worst < 8.0, "{worst}");
    }

    #[test]
    fn cobas_spans_signals_and_follows_the_baseline() {
        let (y, truth) = spectrum(8192, |x| 60.0 * (3.0 * PI * x).sin() + 40.0 * x);
        let estimate = cobas(&y, 1e5, 20);
        assert!(worst_error(&estimate, &truth) < 4.0);
    }

    #[test]
    fn cobas_keeps_more_of_a_broad_line_than_airpls() {
        let n = 8000;
        let centre = n as f64 / 2.0;
        let width = 40.0;
        let base: Vec<f64> = (0..n)
            .map(|i| {
                let x = i as f64 / n as f64;
                40.0 * (2.5 * PI * x).sin() + 30.0 * x * x
            })
            .collect();
        let line: Vec<f64> = (0..n)
            .map(|i| 500.0 / (1.0 + ((i as f64 - centre) / width).powi(2)))
            .collect();
        let y: Vec<f64> = (0..n)
            .map(|i| base[i] + line[i] + ((i * 7919 % 1000) as f64 / 1000.0 - 0.5) * 2.0)
            .collect();
        let from = centre as usize - 6 * width as usize;
        let to = centre as usize + 6 * width as usize;
        let area = |baseline: &[f64]| {
            y[from..to]
                .iter()
                .zip(&baseline[from..to])
                .map(|(v, b)| v - b)
                .sum::<f64>()
        };
        let truth = line[from..to].iter().sum::<f64>();
        let airpls_loss = (area(&airpls(&y, 1e7)) - truth).abs();
        let cobas_loss = (area(&cobas(&y, 1e5, 160)) - truth).abs();
        assert!(
            cobas_loss < airpls_loss,
            "cobas {cobas_loss}, airPLS {airpls_loss}"
        );
    }

    #[test]
    fn polynomial_recovers_cubic_baseline() {
        let (y, truth) = spectrum(3000, |x| 80.0 - 120.0 * x + 90.0 * x.powi(3));
        let estimate = polynomial(&y, 3);
        assert!(worst_error(&estimate, &truth) < 4.0);
    }

    #[test]
    fn anchors_average_the_span_or_take_a_free_value() {
        let y = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let a = anchors(&y, &[(5, None), (3, None), (0, Some(9.0)), (3, None)], 1);
        assert_eq!(a, vec![(0.0, 9.0), (3.0, 3.0), (5.0, 5.0)]);
        // At the end the window is cut short rather than wrapped.
        assert_eq!(anchors(&y, &[(6, None)], 2), vec![(6.0, 5.0)]);
        assert_eq!(anchors(&y, &[(2, None)], 0), vec![(2.0, 2.0)]);
    }

    #[test]
    fn segments_and_spline_pass_through_their_points_and_hold_beyond() {
        let a = [(10.0, 1.0), (40.0, 5.0), (45.0, -2.0), (90.0, 3.0)];
        for through in [Through::Segments, Through::Spline] {
            let b = through_points(100, &a, through);
            for (x, v) in a {
                assert!((b[x as usize] - v).abs() < 1e-9, "{through:?} at {x}");
            }
            assert_eq!(b[0], 1.0);
            assert_eq!(b[99], 3.0);
        }
        let s = through_points(100, &a, Through::Segments);
        assert!((s[25] - 3.0).abs() < 1e-12);
        // Uneven spacing: the spline bends between close points but does not run
        // away across the wide gaps either side of them.
        let spline = through_points(100, &a, Through::Spline);
        assert!(spline.iter().all(|v| v.abs() < 20.0));
    }

    #[test]
    fn polynomial_and_whittaker_fit_their_points() {
        // On a straight line both reproduce it; with fewer points than the order
        // asks for, the order drops to fit.
        let a: Vec<(f64, f64)> = [5.0, 30.0, 60.0, 95.0]
            .iter()
            .map(|&x| (x, 2.0 + 0.1 * x))
            .collect();
        for through in [Through::Polynomial(9), Through::Whittaker(1e5)] {
            let b = through_points(100, &a, through);
            for (i, v) in b.iter().enumerate() {
                assert!(
                    (v - (2.0 + 0.1 * i as f64)).abs() < 1e-6,
                    "{through:?} at {i}: {v}"
                );
            }
        }
        // Stiff, on few points of many: it stays the line through them, to the
        // half a grid step (32 points here) each point is moved onto the grid.
        let n = 131_072;
        let a: Vec<(f64, f64)> = (0..30)
            .map(|k| (k as f64 * 4000.0 + 500.0, 1.0 + k as f64))
            .collect();
        for lambda in [1e5, 1e8, 1e11] {
            let b = through_points(n, &a, Through::Whittaker(lambda));
            for &(x, v) in &a {
                assert!(
                    (b[x as usize] - v).abs() < 0.01,
                    "{lambda:e} at {x}: {}",
                    b[x as usize]
                );
            }
        }
        assert_eq!(through_points(5, &[], Through::Spline), vec![0.0; 5]);
        assert_eq!(
            through_points(3, &[(1.0, 4.0)], Through::Polynomial(3)),
            vec![4.0; 3]
        );
    }

    #[test]
    fn proposed_points_stay_off_the_lines() {
        let (y, _) = spectrum(8192, |x| 30.0 * x);
        let points = propose_points(&y, 1e5, 20, 50);
        assert!(points.len() > 20, "{}", points.len());
        let n = y.len() as f64;
        for p in points {
            for c in [0.2, 0.45, 0.47, 0.8] {
                assert!((p as f64 - c * n).abs() > 15.0, "{p} on the line at {c}");
            }
        }
    }

    /// Noise of unit size, a bend at the right end that falls 300 times it over
    /// the last 3 % (as the digital filter bends a Bruker spectrum), and a line
    /// in the middle.
    fn bent(n: usize, line_at: f64) -> (Vec<f64>, Vec<f64>) {
        let start = (n as f64 * 0.97) as usize;
        let bend: Vec<f64> = (0..n)
            .map(|i| {
                let t = i.saturating_sub(start) as f64 / (n - start) as f64;
                -300.0 * t * t
            })
            .collect();
        let line = |i: usize, c: f64| 2000.0 / (1.0 + ((i as f64 - c * n as f64) / 4.0).powi(2));
        let y = (0..n)
            .map(|i| {
                bend[i] + line(i, 0.5) + line(i, line_at) + ((i * 7919 % 1000) as f64 / 500.0 - 1.0)
            })
            .collect();
        (y, bend)
    }

    #[test]
    fn a_smooth_bend_at_the_end_is_baseline_and_a_line_there_is_not() {
        let n = 32_768;
        let (y, bend) = bent(n, 0.3);
        let b = cobas(&y, 1e5, 20);
        let worst = (n * 97 / 100..n)
            .map(|i| (b[i] - bend[i]).abs())
            .fold(0.0, f64::max);
        assert!(worst < 15.0, "bend followed to {worst}");
        // A line in the last stretch keeps its height.
        let (y, _) = bent(n, 0.99);
        let b = cobas(&y, 1e5, 20);
        let at = (0.99 * n as f64) as usize;
        assert!(
            y[at] - b[at] > 1500.0,
            "line flattened to {}",
            y[at] - b[at]
        );
    }

    #[test]
    fn a_broad_line_keeps_its_wings() {
        // A line 100 points wide on a flat baseline, with noise.
        let n = 16_384;
        let line = |i: usize| 1000.0 / (1.0 + ((i as f64 - 8000.0) / 50.0).powi(2));
        let noisy: Vec<f64> = (0..n)
            .map(|i| line(i) + ((i * 7919 % 1000) as f64 / 500.0 - 1.0))
            .collect();
        assert!((tallest_width(&noisy) as i64 - 100).abs() <= 2);
        let truth: f64 = (0..n).map(line).sum();
        let kept = |dilation: usize| {
            let b = cobas(&noisy, 1e5, dilation);
            noisy.iter().zip(&b).map(|(y, b)| y - b).sum::<f64>() / truth
        };
        // A narrow filter takes the wings off; one as wide as the rule asks keeps them.
        assert!(kept(8) < 0.8, "narrow kept {}", kept(8));
        let wide = (BROAD_LINE_FILTER * 100.0) as usize;
        assert!(kept(wide) > 0.9, "wide kept {}", kept(wide));
    }

    #[test]
    fn a_weaker_broad_line_beside_a_narrow_line_keeps_its_area() {
        let n = 16_384;
        let line = |i: usize, centre: f64, height: f64, half_width: f64| {
            height / (1.0 + ((i as f64 - centre) / half_width).powi(2))
        };
        for sign in [-1.0, 1.0] {
            let signal: Vec<f64> = (0..n)
                .map(|i| line(i, 4000.0, 1000.0, 3.0) + line(i, 10_000.0, sign * 100.0, 50.0))
                .collect();
            let y: Vec<f64> = (0..n)
                .map(|i| 10.0 + signal[i] + ((i * 7919 % 1000) as f64 / 500.0 - 1.0))
                .collect();
            // The weak line sets the required dilation, even beside a taller solvent line.
            let dilation = (BROAD_LINE_FILTER * 100.0).round() as usize;
            let baseline = cobas(&y, 1e5, dilation);
            let region = 8500..11_500;
            let truth: f64 = signal[region.clone()].iter().sum();
            let measured: f64 = region.map(|i| y[i] - baseline[i]).sum();
            let area_error = ((measured - truth) / truth).abs();
            assert!(area_error < 0.05, "sign {sign}: area error {area_error}");
            for quiet in [1000..2000, 13_000..14_000] {
                let rms = (quiet
                    .clone()
                    .map(|i| (baseline[i] - 10.0).powi(2))
                    .sum::<f64>()
                    / quiet.len() as f64)
                    .sqrt();
                assert!(rms < 0.1, "sign {sign}: quiet baseline RMS {rms}");
            }
        }
    }

    #[test]
    fn automatic_width_ignores_offsets_noise_and_slow_drift() {
        let n = 16_384;
        let axis = crate::PpmAxis {
            first: 16.0,
            delta: -16.0 / n as f64,
        };
        let noise = |i: usize| (i * 7919 % 1000) as f64 / 500.0 - 1.0;
        for drift in [0.0, 20.0] {
            let y: Vec<f64> = (0..n)
                .map(|i| 10.0 + drift * (2.0 * PI * i as f64 / n as f64).sin() + noise(i))
                .collect();
            assert_eq!(signal_width(&y, &axis), 0.0, "drift {drift}");
        }
        let (end_bend, _) = bent(n, 0.3);
        assert!(signal_width(&end_bend, &axis) < 12.0);
        // Broader than the proton peak picker's limit: the signed tallest-line safeguard remains.
        for sign in [-1.0, 1.0] {
            let y: Vec<f64> = (0..n)
                .map(|i| {
                    10.0 + sign * 1000.0 / (1.0 + ((i as f64 - 8000.0) / 400.0).powi(2)) + noise(i)
                })
                .collect();
            let width = signal_width(&y, &axis);
            assert!((width - 800.0).abs() < 15.0, "sign {sign}: width {width}");
        }
    }

    #[test]
    fn short_or_empty_input() {
        assert_eq!(airpls(&[1.0, 2.0], 1e5), vec![0.0, 0.0]);
        assert_eq!(airpls(&[0.0; 10], 1e5), vec![0.0; 10]);
        assert_eq!(cobas(&[1.0, 2.0], 1e5, 8), vec![0.0, 0.0]);
        assert!(polynomial(&[], 3).is_empty());
        assert_eq!(polynomial(&[5.0], 3), vec![5.0]);
    }
}
