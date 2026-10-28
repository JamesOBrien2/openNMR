//! Filling in a non-uniformly sampled dimension by iterative soft thresholding
//! (IST; Hyberts, Milbradt, Wagner, Arthanari & Wagner, J. Biomol. NMR 52,
//! 315-327, 2012), which is the reconstruction recorded for every
//! non-uniformly sampled 2D in testdata (`Mdd_mod = cs`, `Mdd_CsALG = ist`).

use std::sync::Arc;

use rustfft::{Fft, FftPlanner};

use crate::Complex64;

/// Each iteration keeps what stands above this share of the largest component
/// still unexplained. Over the recorded 200 iterations 0.9 matched the stored
/// spectra of the five non-uniformly sampled 2Ds in testdata at 0.94-0.98; 0.98, the
/// gentler step often quoted, had not converged by then (0.87-0.92), and 0.5
/// was within 0.01 of 0.9 on all but one.
const FRACTION: f64 = 0.9;

/// Forward and inverse transforms of one grid size, planned once and used for
/// every column and every iteration.
pub struct Plan {
    forward: Arc<dyn Fft<f64>>,
    inverse: Arc<dyn Fft<f64>>,
    grid: usize,
}

impl Plan {
    pub fn new(grid: usize) -> Self {
        let mut planner = FftPlanner::new();
        Self {
            forward: planner.plan_fft_forward(grid),
            inverse: planner.plan_fft_inverse(grid),
            grid,
        }
    }

    /// The whole grid from `values` acquired at `points`, in `iterations`.
    ///
    /// Each iteration transforms what the spectrum so far does not explain at
    /// the acquired points, keeps of it only what stands above a threshold
    /// (`FRACTION` of its largest component), shrunk by that threshold, and
    /// adds it to the spectrum. The acquired points are put back as they were
    /// at the end. The recorded virtual echo (`Mdd_CsVE`), which mirrors a
    /// phase-corrected FID to double what is known, is left out: it needs the
    /// F1 phase before the transform, and a magnitude-mode COSY has none.
    pub fn reconstruct(
        &self,
        points: &[usize],
        values: &[Complex64],
        iterations: usize,
    ) -> Vec<Complex64> {
        let n = self.grid;
        let mut residual = vec![Complex64::ZERO; n];
        for (&k, &v) in points.iter().zip(values) {
            residual[k] = v;
        }
        let mut spectrum = vec![Complex64::ZERO; n];
        let mut work = residual.clone();
        self.forward.process(&mut work);
        for _ in 0..iterations {
            let threshold = FRACTION * work.iter().map(|z| z.norm()).fold(0.0, f64::max);
            for (s, z) in spectrum.iter_mut().zip(&work) {
                let size = z.norm();
                if size > threshold {
                    *s += z * (1.0 - threshold / size);
                }
            }
            // What the spectrum so far leaves unexplained at the acquired points.
            work.copy_from_slice(&spectrum);
            self.inverse.process(&mut work);
            residual.iter_mut().for_each(|z| *z = Complex64::ZERO);
            for (&k, &v) in points.iter().zip(values) {
                residual[k] = v - work[k] / n as f64;
            }
            work.copy_from_slice(&residual);
            self.forward.process(&mut work);
        }
        let mut fid = spectrum;
        self.inverse.process(&mut fid);
        fid.iter_mut().for_each(|z| *z /= n as f64);
        for (&k, &v) in points.iter().zip(values) {
            fid[k] = v;
        }
        fid
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    /// Two lines exactly on the grid's frequencies: as sparse as a spectrum gets.
    fn fid() -> Vec<Complex64> {
        (0..256)
            .map(|k| {
                let t = 2.0 * PI * k as f64 / 256.0;
                Complex64::from_polar(1.0, 28.0 * t) + Complex64::from_polar(0.5, -61.0 * t)
            })
            .collect()
    }

    /// About a quarter of the grid at random, point 0 included. A schedule
    /// with a pattern to it aliases coherently, which no reconstruction undoes.
    fn schedule() -> Vec<usize> {
        let mut state = 0x2545_f491_4f6c_dd1du64;
        (0..256usize)
            .filter(|&k| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                k == 0 || state.is_multiple_of(4)
            })
            .collect()
    }

    #[test]
    fn fills_in_a_sparse_spectrum_from_a_quarter_of_it() {
        let whole = fid();
        let points = schedule();
        assert!(points.len() < 80);
        let values: Vec<Complex64> = points.iter().map(|&k| whole[k]).collect();
        let filled = Plan::new(256).reconstruct(&points, &values, 200);
        for (&k, v) in points.iter().zip(&values) {
            assert_eq!(filled[k], *v, "an acquired point changed");
        }
        let worst = filled
            .iter()
            .zip(&whole)
            .map(|(a, b)| (a - b).norm())
            .fold(0.0, f64::max);
        // Soft thresholding shrinks what it keeps a little: within 2 % of the
        // tallest line after 200 iterations.
        assert!(worst < 0.03, "worst {worst:e}");
    }
}
