//! Forward linear prediction: a FID cut short by too few increments is carried
//! on by predicting each next point from the `order` before it, as nmrglue's
//! `proc_lp.lp` does with `mode='f'` and `append='after'`.

use crate::Complex64;

/// `data` carried on to `size` points. The coefficients are the least-squares
/// solution over every run of `order + 1` points (nmrglue `find_lpc`), solved
/// through the normal equations rather than an SVD, with a ridge of 1e-10 of
/// their trace so a column of pure zeros does not divide by nothing. Too few
/// points to fit, or nothing to fit, leaves the rest zero.
///
/// A FID that is carried on never grows past the largest point it already
/// has. One that does holds a growing term the reflection in [`stable`]
/// missed, and is predicted again at half the order. Reflection misses them at
/// high orders: rebuilding a polynomial of degree 100 from its roots loses
/// enough precision to put some back outside the circle, and the order of 100
/// some COSYs record grew to 10⁶⁰ of the spectrum, where order 32 matched
/// the stored spectrum at 0.97.
pub fn extend(data: &mut Vec<Complex64>, size: usize, order: usize) {
    let n = data.len();
    if size <= n {
        return;
    }
    let largest = data.iter().map(|z| z.norm()).fold(0.0, f64::max);
    let mut order = order;
    while order > 0 {
        if let Some(extended) = predicted(data, size, order)
            && extended[n..].iter().all(|z| z.norm() <= largest)
        {
            *data = extended;
            return;
        }
        order /= 2;
    }
    data.resize(size, Complex64::ZERO);
}

/// `data` carried on at `order`, or nothing when there is too little to fit.
fn predicted(data: &[Complex64], size: usize, order: usize) -> Option<Vec<Complex64>> {
    let n = data.len();
    if n <= 2 * order {
        return None;
    }
    let a = stable(&coefficients(data, order)?);
    let mut out = data.to_vec();
    out.reserve(size - n);
    for k in n..size {
        let next = a.iter().zip(&out[k - order..k]).map(|(a, x)| a * x).sum();
        out.push(next);
    }
    Some(out)
}

/// `a` such that `x[k] ≈ Σ a[j]·x[k − order + j]`, by least squares.
fn coefficients(x: &[Complex64], order: usize) -> Option<Vec<Complex64>> {
    // Normal equations: (Dᴴ D) a = Dᴴ d, D's rows being the runs before each
    // point. An entry is the one up and to its left with one product dropped
    // from the start and one added at the end, so only the first row is summed
    // in full: built entry by entry, the order of 100 recorded for some COSYs
    // took 12 s on a 2048-column 2D.
    let n = x.len();
    let mut m = vec![vec![Complex64::ZERO; order + 1]; order];
    for j in 0..order {
        m[0][j] = (order..n)
            .map(|k| x[k - order].conj() * x[k - order + j])
            .sum();
    }
    for i in 1..order {
        m[i][0] = m[0][i].conj();
        for j in 1..order {
            m[i][j] = m[i - 1][j - 1] - x[i - 1].conj() * x[j - 1]
                + x[n - order + i - 1].conj() * x[n - order + j - 1];
        }
    }
    for (i, row) in m.iter_mut().enumerate() {
        row[order] = (order..n).map(|k| x[k - order + i].conj() * x[k]).sum();
    }
    let trace: f64 = (0..order).map(|i| m[i][i].re).sum();
    if trace <= 0.0 {
        return None;
    }
    for (i, row) in m.iter_mut().enumerate() {
        row[i] += trace * 1e-10;
    }
    solve(m)
}

/// `a` with every root of its characteristic polynomial outside the unit
/// circle - a signal that would grow without end - reflected inside it, which
/// is what nmrglue's `lp` does by default for forward prediction
/// (`bad_roots='auto'`, `fix_mode='on'`). On the HMBC in testdata, predicted
/// at its recorded order of 32, it raises the match with the stored spectrum
/// from 0.963 to 0.979.
fn stable(a: &[Complex64]) -> Vec<Complex64> {
    let p = a.len();
    // x[k] = Σ a[j]·x[k − p + j] has the characteristic polynomial
    // zᵖ − Σ a[j]·zʲ, monic, lowest power first.
    let mut poly: Vec<Complex64> = a.iter().map(|c| -c).collect();
    poly.push(Complex64::ONE);
    let found = roots(&poly);
    if found.iter().any(|r| !r.is_finite()) {
        return a.to_vec();
    }
    let roots = found
        .into_iter()
        .map(|r| if r.norm() > 1.0 { 1.0 / r.conj() } else { r });
    let mut rebuilt = vec![Complex64::ONE];
    for r in roots {
        // Multiply by (z − r).
        let mut next = vec![Complex64::ZERO; rebuilt.len() + 1];
        for (k, c) in rebuilt.iter().enumerate() {
            next[k + 1] += c;
            next[k] -= r * c;
        }
        rebuilt = next;
    }
    rebuilt[..p].iter().map(|c| -c).collect()
}

/// Roots of a monic polynomial (lowest power first) by Aberth's method,
/// which moves every root at once by Newton's step corrected for the others.
/// Durand-Kerner, tried first, divides by the product of a root's distances to
/// all the others, which at the order of 100 recorded for some COSYs
/// overflowed and gave NaN. The predictor's roots lie near the unit
/// circle, so the search starts on it.
fn roots(poly: &[Complex64]) -> Vec<Complex64> {
    let p = poly.len() - 1;
    // Value and derivative together, by Horner's scheme.
    let horner = |z: Complex64| {
        poly.iter()
            .rev()
            .fold((Complex64::ZERO, Complex64::ZERO), |(v, d), c| {
                (v * z + c, d * z + v)
            })
    };
    let mut r: Vec<Complex64> = (0..p)
        .map(|k| Complex64::from_polar(1.0, 2.0 * std::f64::consts::PI * k as f64 / p as f64 + 0.4))
        .collect();
    for _ in 0..100 {
        let mut moved = 0.0f64;
        for i in 0..p {
            let (value, slope) = horner(r[i]);
            if value.norm() == 0.0 {
                continue;
            }
            let newton = value / slope;
            let repulsion: Complex64 = (0..p)
                .filter(|&j| j != i)
                .map(|j| 1.0 / (r[i] - r[j]))
                .sum();
            let step = newton / (1.0 - newton * repulsion);
            if step.is_finite() {
                r[i] -= step;
                moved = moved.max(step.norm());
            }
        }
        if moved < 1e-12 {
            break;
        }
    }
    r
}

/// Gaussian elimination with partial pivoting on an augmented `n × (n + 1)`.
fn solve(mut m: Vec<Vec<Complex64>>) -> Option<Vec<Complex64>> {
    let n = m.len();
    for col in 0..n {
        let pivot = (col..n).max_by(|&a, &b| m[a][col].norm().total_cmp(&m[b][col].norm()))?;
        if m[pivot][col].norm() == 0.0 {
            return None;
        }
        m.swap(col, pivot);
        let (done, rest) = m.split_at_mut(col + 1);
        let pivot_row = &done[col];
        for row in rest {
            let f = row[col] / pivot_row[col];
            for (target, v) in row[col..].iter_mut().zip(&pivot_row[col..]) {
                *target -= f * v;
            }
        }
    }
    let mut a = vec![Complex64::ZERO; n];
    for i in (0..n).rev() {
        let s: Complex64 = (i + 1..n).map(|j| m[i][j] * a[j]).sum();
        a[i] = (m[i][n] - s) / m[i][i];
    }
    Some(a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    /// Two decaying lines, which an order of 4 describes exactly.
    fn fid(n: usize) -> Vec<Complex64> {
        (0..n)
            .map(|k| {
                let t = k as f64;
                Complex64::from_polar((-t / 40.0).exp(), 2.0 * PI * 0.11 * t)
                    + Complex64::from_polar(0.5 * (-t / 25.0).exp(), -2.0 * PI * 0.31 * t)
            })
            .collect()
    }

    #[test]
    fn predicts_a_sum_of_decaying_lines() {
        let whole = fid(128);
        let mut cut = whole[..48].to_vec();
        extend(&mut cut, 128, 4);
        assert_eq!(cut.len(), 128);
        let worst = cut
            .iter()
            .zip(&whole)
            .map(|(a, b)| (a - b).norm())
            .fold(0.0, f64::max);
        assert!(worst < 1e-6, "worst {worst:e}");
    }

    #[test]
    fn a_growing_signal_is_turned_into_a_decaying_one() {
        // x[k] = 1.1·x[k − 1] grows; its root 1.1 is reflected to 1/1.1.
        let a = stable(&[Complex64::new(1.1, 0.0)]);
        assert!((a[0].re - 1.0 / 1.1).abs() < 1e-12 && a[0].im.abs() < 1e-12);
        // Roots already inside are kept: the two lines of `fid` come back.
        let whole = fid(128);
        let mut cut = whole[..48].to_vec();
        extend(&mut cut, 128, 4);
        assert!((cut[127] - whole[127]).norm() < 1e-6);
    }

    #[test]
    fn a_prediction_that_grows_is_tried_again_lower() {
        // Two lines and a little noise asked for at an order far too high for
        // them: whatever the order ends up, nothing grows past the data.
        let mut data: Vec<Complex64> = fid(256)
            .iter()
            .enumerate()
            .map(|(k, z)| z + Complex64::new(((k * 7919) % 101) as f64 / 1e4, 0.0))
            .collect();
        let largest = data.iter().map(|z| z.norm()).fold(0.0, f64::max);
        extend(&mut data, 512, 100);
        assert!(data.iter().all(|z| z.norm() <= largest));
    }

    #[test]
    fn nothing_to_fit_leaves_zeros() {
        let mut zeros = vec![Complex64::ZERO; 40];
        extend(&mut zeros, 64, 8);
        assert!(zeros.len() == 64 && zeros.iter().all(|z| *z == Complex64::ZERO));
        // Too short to fit an order this high, it is predicted at a lower one.
        let mut short = fid(10);
        extend(&mut short, 20, 8);
        assert!(short[10..].iter().all(|z| z.is_finite()));
    }
}
