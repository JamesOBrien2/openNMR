//! Display decimation for 1D traces.

use std::ops::Range;

/// Reduces `data[range]` to the minimum and maximum of each of `columns` equal
/// slices, kept in index order, so no peak disappears at any zoom level.
///
/// Returns interleaved `x, y` pairs with `x` relative to `range.start`, which keeps
/// coordinates small enough for exact `f32` on the GPU. With two or fewer points
/// per column the points are returned as they are.
pub fn min_max(data: &[f64], range: Range<usize>, columns: usize) -> Vec<f32> {
    let end = range.end.min(data.len());
    let slice = &data[range.start.min(end)..end];
    let n = slice.len();
    if columns == 0 || n <= 2 * columns {
        return slice
            .iter()
            .enumerate()
            .flat_map(|(i, &y)| [i as f32, y as f32])
            .collect();
    }
    let mut out = Vec::with_capacity(4 * columns);
    for c in 0..columns {
        let start = c * n / columns;
        let chunk = &slice[start..(c + 1) * n / columns];
        let (mut lo, mut hi) = (0, 0);
        for (i, &y) in chunk.iter().enumerate() {
            if y < chunk[lo] {
                lo = i;
            }
            if y > chunk[hi] {
                hi = i;
            }
        }
        let (first, second) = (lo.min(hi), lo.max(hi));
        out.extend([
            (start + first) as f32,
            chunk[first] as f32,
            (start + second) as f32,
            chunk[second] as f32,
        ]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_single_point_spike() {
        let mut data = vec![0.0; 10_000];
        data[4321] = 5.0;
        data[7000] = -3.0;
        let xy = min_max(&data, 0..data.len(), 100);
        assert_eq!(xy.len(), 400);
        let ys: Vec<f32> = xy.chunks(2).map(|p| p[1]).collect();
        assert!(ys.contains(&5.0) && ys.contains(&-3.0));
        let spike = xy.chunks(2).find(|p| p[1] == 5.0).unwrap();
        assert_eq!(spike[0], 4321.0);
    }

    #[test]
    fn x_is_increasing_and_relative_to_start() {
        let data: Vec<f64> = (0..1000).map(|i| ((i * 7919) % 101) as f64).collect();
        let xy = min_max(&data, 200..800, 50);
        let xs: Vec<f32> = xy.chunks(2).map(|p| p[0]).collect();
        assert!(xs.windows(2).all(|w| w[0] <= w[1]));
        assert!(xs[0] >= 0.0 && *xs.last().unwrap() < 600.0);
    }

    #[test]
    fn returns_points_when_zoomed_in_and_clamps_range() {
        let data = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(min_max(&data, 2..10, 100), vec![0.0, 3.0, 1.0, 4.0]);
        assert!(min_max(&data, 9..12, 100).is_empty());
    }
}
