//! 2D processing: the 1D operations applied to every row (F2) or every column
//! (F1), with one step between that makes the rows into series along F1. Like
//! the 1D list, the operations are replayed on a copy of the raw data.

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::process::Operation;
use crate::{Complex64, Domain, Error, Metadata, Quadrature, Result, Spectrum, Spectrum2D, nus};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation2D {
    /// A 1D operation applied to every row, along the direct dimension.
    F2(Operation),
    /// Makes the rows into complex series along F1 as [`Quadrature`] says.
    /// Everything in F2 has to be done before: its imaginary part is used up.
    Quadrature,
    /// Fills in the increments a non-uniformly sampled F1 skipped, by
    /// iterative soft thresholding ([`nus::Plan::reconstruct`]), in place of
    /// the series of increments as acquired. F1 processing waits for it.
    Reconstruct { iterations: usize },
    /// A 1D operation applied to every column, along the indirect dimension.
    F1(Operation),
}

/// The pipeline the recorded parameters describe: in F1 the linear prediction
/// first, if any, and then in each dimension the window, zero filling to `SI`
/// (truncating when `SI` is smaller) and the transform, the recorded phase
/// where there is one, and the magnitude in F1 when it is recorded so. F2 is
/// first cut to the recorded points (`TDeff`), before the window, so a sine
/// bell spans them.
/// Spectra already in the frequency domain get no operations.
pub fn default_2d(raw: &Spectrum2D) -> Vec<Operation2D> {
    if raw.domain == [Domain::Frequency; 2] {
        return Vec::new();
    }
    let mut ops = Vec::new();
    if let Some(size) = raw.f2_points {
        ops.push(Operation2D::F2(Operation::ZeroFill { size }));
    }
    if raw.f2.conjugate {
        ops.push(Operation2D::F2(Operation::Conjugate));
    }
    // Taken off as a first-order phase, as in 1D (see `process::auto_1d`).
    if raw.f2.group_delay > 0.0 {
        ops.push(Operation2D::F2(Operation::DigitalFilter));
    }
    let points = raw
        .f2_points
        .unwrap_or(raw.rows.first().map_or(0, Vec::len));
    ops.extend(dimension(&raw.f2, raw.windows[0].clone(), points).map(Operation2D::F2));
    ops.push(Operation2D::Quadrature);
    let mut points = match raw.quadrature {
        Quadrature::Qf => raw.rows.len(),
        Quadrature::EchoAntiecho => raw.rows.len() / 2,
    };
    if let Some(sampling) = &raw.nus {
        // The recorded count, `Mdd_CsNITER`, on every such 2D in testdata.
        ops.push(Operation2D::Reconstruct { iterations: 200 });
        points = sampling.grid;
    }
    if let Some(prediction) = &raw.f1_prediction {
        if let Operation::LinearPrediction { size, .. } = *prediction {
            points = size;
        }
        ops.push(Operation2D::F1(prediction.clone()));
    }
    ops.extend(dimension(&raw.f1, raw.windows[1].clone(), points).map(Operation2D::F1));
    if raw.magnitude {
        ops.push(Operation2D::F1(Operation::Magnitude));
    }
    ops
}

/// Window, zero filling, transform and phase of one dimension.
fn dimension(
    meta: &Metadata,
    window: Option<Operation>,
    points: usize,
) -> impl Iterator<Item = Operation> {
    let size = meta.size.unwrap_or(points.next_power_of_two() * 2);
    window
        .into_iter()
        .chain([Operation::ZeroFill { size }, Operation::Ft])
        .chain(
            meta.phase
                .map(|[p0, p1]| Operation::Phase { p0, p1, pivot: 0.0 }),
        )
}

/// Replays `ops` on a copy of `raw`.
pub fn process(raw: &Spectrum2D, ops: &[Operation2D]) -> Result<Spectrum2D> {
    let mut s = raw.clone();
    for op in ops {
        apply(op, &mut s)?;
    }
    Ok(s)
}

fn apply(op: &Operation2D, s: &mut Spectrum2D) -> Result<()> {
    match op {
        Operation2D::F2(op) => {
            if s.along_f1 {
                return Err(Error::Order {
                    operation: "F2 processing",
                    before: true,
                });
            }
            s.rows = each(std::mem::take(&mut s.rows), &mut s.f2, &mut s.domain[0], op)?;
        }
        Operation2D::Quadrature => {
            if s.domain[0] != Domain::Frequency {
                return Err(Error::Order {
                    operation: "The F1 quadrature step",
                    before: false,
                });
            }
            if !s.along_f1 {
                s.rows = match s.quadrature {
                    // The series runs the opposite way to F2's: conjugated, the
                    // peaks land where the stored 2rr has them (correlation 0.88
                    // on the COSY and HMBC in testdata, against 0.00 and 0.05).
                    Quadrature::Qf => s
                        .rows
                        .iter()
                        .map(|row| row.iter().map(Complex64::conj).collect())
                        .collect(),
                    Quadrature::EchoAntiecho => s
                        .rows
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|[echo, antiecho]| echo_antiecho(echo, antiecho))
                        .collect(),
                };
            }
            s.along_f1 = true;
        }
        Operation2D::Reconstruct { iterations } => {
            if !s.along_f1 || s.domain[1] != Domain::Time {
                return Err(Error::Unsupported(
                    "filling in skipped increments comes after F2 and before anything in F1".into(),
                ));
            }
            // Taken out, so the grid is filled in once.
            if let Some(sampling) = s.nus.take() {
                let plan = nus::Plan::new(sampling.grid);
                let columns: Vec<Vec<Complex64>> = transpose(&s.rows)
                    .par_iter()
                    .map(|column| plan.reconstruct(&sampling.points, column, *iterations))
                    .collect();
                s.rows = transpose(&columns);
            }
        }
        Operation2D::F1(op) => {
            if !s.along_f1 {
                return Err(Error::Order {
                    operation: "F1 processing",
                    before: false,
                });
            }
            if s.nus.is_some() {
                return Err(Error::Unsupported(
                    "F1 processing before the skipped increments are filled in".into(),
                ));
            }
            let columns = each(transpose(&s.rows), &mut s.f1, &mut s.domain[1], op)?;
            s.rows = transpose(&columns);
        }
    }
    Ok(())
}

/// One F1 point from an echo row and the antiecho row after it, both
/// transformed and phased in F2: the cosine and sine modulation are
/// `−i·(echo + antiecho)` and `echo − antiecho`, each taken where F2 is
/// absorptive. Of the 32 ways to pair the real and imaginary parts of the sum
/// and difference, only this one matches the stored 2rr of the HSQC in
/// testdata with its recorded phase applied (correlation 0.87, the next 0.45).
/// Every HSQC in testdata records an F1 phase near 180°, so this pairing and
/// that phase may both be half a turn from another convention; together they
/// give the stored spectrum.
fn echo_antiecho(echo: &[Complex64], antiecho: &[Complex64]) -> Vec<Complex64> {
    echo.iter()
        .zip(antiecho)
        .map(|(e, a)| Complex64::new((e + a).im, (e - a).re))
        .collect()
}

/// Applies a 1D operation to each line, carrying the metadata and domain it
/// leaves behind (the same for every line).
fn each(
    lines: Vec<Vec<Complex64>>,
    meta: &mut Metadata,
    domain: &mut Domain,
    op: &Operation,
) -> Result<Vec<Vec<Complex64>>> {
    // Lines are independent, so they are shared out over the cores.
    // ponytail: each line plans its own FFT (process::ft); cache a planner if
    // the 4096×1024 benchmark says it matters.
    let done = lines
        .into_par_iter()
        .map(|data| {
            let mut s = Spectrum {
                data,
                meta: meta.clone(),
                domain: *domain,
            };
            op.apply(&mut s)?;
            Ok(s)
        })
        .collect::<Result<Vec<Spectrum>>>()?;
    if let Some(last) = done.last() {
        (*meta, *domain) = (last.meta.clone(), last.domain);
    }
    Ok(done.into_iter().map(|s| s.data).collect())
}

fn transpose(rows: &[Vec<Complex64>]) -> Vec<Vec<Complex64>> {
    let cols = rows.first().map_or(0, Vec::len);
    (0..cols)
        .map(|c| rows.iter().map(|row| row[c]).collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(quadrature: Quadrature) -> Spectrum2D {
        let meta = Metadata {
            nucleus: "1H".into(),
            sw_hz: 1000.0,
            obs_mhz: 100.0,
            carrier_hz: 0.0,
            group_delay: 0.0,
            solvent: None,
            phase: None,
            conjugate: false,
            signed: false,
            size: None,
            line_broadening: None,
            delay_at_ft: false,
        };
        Spectrum2D {
            rows: vec![vec![Complex64::new(1.0, 0.5); 8]; 4],
            f2: meta.clone(),
            f1: meta,
            domain: [Domain::Time; 2],
            quadrature,
            along_f1: false,
            windows: [None, None],
            f2_points: None,
            magnitude: false,
            f1_prediction: None,
            nus: None,
        }
    }

    #[test]
    fn default_2d_transforms_both_dimensions() {
        let qf = raw(Quadrature::Qf);
        let s = process(&qf, &default_2d(&qf)).unwrap();
        assert_eq!(s.domain, [Domain::Frequency; 2]);
        // Twice the points, rounded up to a power of two, in each dimension.
        assert_eq!((s.rows.len(), s.rows[0].len()), (8, 16));

        // Echo-antiecho pairs make one F1 point each.
        let ea = raw(Quadrature::EchoAntiecho);
        let s = process(&ea, &default_2d(&ea)).unwrap();
        assert_eq!((s.rows.len(), s.rows[0].len()), (4, 16));
        assert!(default_2d(&s).is_empty());
    }

    #[test]
    fn dimensions_are_processed_in_order() {
        let s = raw(Quadrature::Qf);
        let f1 = process(&s, &[Operation2D::F1(Operation::Ft)]);
        assert!(matches!(f1, Err(Error::Order { before: false, .. })));
        let early = process(&s, &[Operation2D::Quadrature]);
        assert!(matches!(early, Err(Error::Order { before: false, .. })));
        let late = process(
            &s,
            &[
                Operation2D::F2(Operation::Ft),
                Operation2D::Quadrature,
                Operation2D::F2(Operation::Reverse),
            ],
        );
        assert!(matches!(late, Err(Error::Order { before: true, .. })));
    }
}
