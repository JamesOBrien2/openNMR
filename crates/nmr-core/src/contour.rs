//! Contour lines of a 2D spectrum by marching squares.

use rayon::prelude::*;

/// [`segments`] at each of `levels`, worked out side by side on every core.
pub fn at_levels(z: &[f64], cols: usize, levels: &[f64]) -> Vec<Vec<f32>> {
    levels
        .par_iter()
        .map(|&level| segments(z, cols, level))
        .collect()
}

/// Line segments where `z` (row-major, `cols` wide) crosses `level`, as flat
/// `x0, y0, x1, y1` in point coordinates: column and row, fractional between
/// points. A cell whose opposite corners lie on either side of the level (a
/// saddle) is split by its mean, so a ridge between two peaks is drawn as one
/// line or two consistently.
pub fn segments(z: &[f64], cols: usize, level: f64) -> Vec<f32> {
    let mut out = Vec::new();
    if cols < 2 {
        return out;
    }
    let rows = z.len() / cols;
    for r in 0..rows.saturating_sub(1) {
        for c in 0..cols - 1 {
            let a = z[r * cols + c];
            let b = z[r * cols + c + 1];
            let d = z[(r + 1) * cols + c];
            let e = z[(r + 1) * cols + c + 1];
            let case = usize::from(a >= level)
                | usize::from(b >= level) << 1
                | usize::from(e >= level) << 2
                | usize::from(d >= level) << 3;
            if case == 0 || case == 15 {
                continue;
            }
            let (x, y) = (c as f64, r as f64);
            let at = |p: f64, q: f64| (level - p) / (q - p);
            let top = || (x + at(a, b), y);
            let right = || (x + 1.0, y + at(b, e));
            let bottom = || (x + at(d, e), y + 1.0);
            let left = || (x, y + at(a, d));
            let centre_above = (a + b + d + e) / 4.0 >= level;
            let mut line = |p: (f64, f64), q: (f64, f64)| {
                out.extend([p.0 as f32, p.1 as f32, q.0 as f32, q.1 as f32]);
            };
            match case {
                1 | 14 => line(left(), top()),
                2 | 13 => line(top(), right()),
                3 | 12 => line(left(), right()),
                4 | 11 => line(right(), bottom()),
                6 | 9 => line(top(), bottom()),
                7 | 8 => line(left(), bottom()),
                // Top left and bottom right above.
                5 if centre_above => {
                    line(top(), right());
                    line(left(), bottom());
                }
                5 => {
                    line(left(), top());
                    line(right(), bottom());
                }
                // Top right and bottom left above.
                _ if centre_above => {
                    line(left(), top());
                    line(right(), bottom());
                }
                _ => {
                    line(top(), right());
                    line(left(), bottom());
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Gaussian peak of height `h` in the middle of an `n`×`n` grid.
    fn blob(n: usize, h: f64) -> Vec<f64> {
        let mid = (n - 1) as f64 / 2.0;
        (0..n * n)
            .map(|i| {
                let (r, c) = ((i / n) as f64 - mid, (i % n) as f64 - mid);
                h * (-(r * r + c * c) / 50.0).exp()
            })
            .collect()
    }

    #[test]
    fn a_peak_gives_a_closed_ring_at_its_level() {
        let z = blob(41, 1.0);
        let lines = segments(&z, 41, 0.5);
        assert!(!lines.is_empty());
        // Every end lies where the Gaussian is at half height: r² = 50 ln 2.
        let radius = (50.0 * 2f64.ln()).sqrt();
        for p in lines.as_chunks::<2>().0 {
            let (x, y) = (f64::from(p[0]) - 20.0, f64::from(p[1]) - 20.0);
            assert!(((x * x + y * y).sqrt() - radius).abs() < 0.1);
        }
        // Closed: every end is shared by exactly two segments.
        let key = |x: f32, y: f32| ((x * 1e4).round() as i64, (y * 1e4).round() as i64);
        let mut ends = std::collections::HashMap::new();
        for p in lines.as_chunks::<2>().0 {
            *ends.entry(key(p[0], p[1])).or_insert(0) += 1;
        }
        assert!(ends.values().all(|&n| n == 2));
    }

    #[test]
    fn levels_beyond_the_data_give_nothing() {
        let z = blob(21, 1.0);
        assert!(segments(&z, 21, 2.0).is_empty());
        assert!(segments(&z, 21, -0.5).is_empty());
        // A negative peak shows at a negative level only.
        let neg: Vec<f64> = z.iter().map(|v| -v).collect();
        assert!(segments(&neg, 21, 0.5).is_empty());
        assert!(!segments(&neg, 21, -0.5).is_empty());
    }

    #[test]
    fn a_saddle_is_split_by_its_mean() {
        // Top left and bottom right above: joined when the middle is above too.
        let joined = segments(&[1.0, 0.0, 0.0, 1.0], 2, 0.4);
        assert_eq!(joined, vec![0.6, 0.0, 1.0, 0.4, 0.0, 0.6, 0.4, 1.0]);
        let apart = segments(&[1.0, 0.0, 0.0, 1.0], 2, 0.6);
        assert_eq!(apart.len(), 8);
        assert_eq!(&apart[..4], &[0.0, 0.4, 0.4, 0.0]);
    }
}
