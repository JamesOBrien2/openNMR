use crate::Complex64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    Time,
    Frequency,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Metadata {
    pub nucleus: String,
    pub sw_hz: f64,
    /// Observe frequency in MHz (Bruker `SF`, which includes `SR`).
    pub obs_mhz: f64,
    /// Carrier offset from `obs_mhz` in Hz.
    pub carrier_hz: f64,
    /// Digital filter group delay in points; 0 once removed.
    pub group_delay: f64,
    /// Solvent the sample was made up in (Bruker `SOLVENT`).
    pub solvent: Option<String>,
    /// Phase the spectrometer recorded (Bruker `PHC0`, `PHC1`) in degrees, with
    /// the pivot at the first point, when the dataset has been phased.
    pub phase: Option<[f64; 2]>,
    /// The dataset stores the quadrature pair with the sign opposite to the one
    /// nmrglue's Fourier transform expects, so the FID has to be conjugated or
    /// the spectrum comes out mirrored about the carrier.
    pub conjugate: bool,
    /// The experiment gives lines of both signs (APT, DEPT, J-modulated), so a
    /// line pointing down is not a phase error.
    pub signed: bool,
    /// Points of the recorded spectrum (`SI`), which says how far the FID was
    /// zero filled.
    pub size: Option<usize>,
    /// Exponential line broadening recorded in Hz (`LB` with `WDW = 1`), or 0
    /// when no window was applied (`WDW = 0`); `None` for any other window.
    pub line_broadening: Option<f64>,
    /// The digital filter's delay is to come off with the Fourier transform,
    /// as a first-order phase ([`crate::process::Operation::DigitalFilter`]).
    pub delay_at_ft: bool,
}

#[derive(Debug, Clone)]
pub struct Spectrum {
    pub data: Vec<Complex64>,
    pub meta: Metadata,
    pub domain: Domain,
}

impl Spectrum {
    pub fn ppm_axis(&self) -> PpmAxis {
        PpmAxis::new(&self.meta, self.data.len())
    }
}

/// How the indirect dimension of a 2D experiment was acquired (Bruker
/// `FnMODE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quadrature {
    /// One row per increment, magnitude mode (COSY, HMBC).
    Qf,
    /// Echo and antiecho rows alternate (gradient-selected HSQC).
    EchoAntiecho,
}

/// A 2D dataset: one row per F1 point, each a series along F2.
#[derive(Debug, Clone)]
pub struct Spectrum2D {
    pub rows: Vec<Vec<Complex64>>,
    pub f2: Metadata,
    pub f1: Metadata,
    /// Domain of F2 and of F1.
    pub domain: [Domain; 2],
    pub quadrature: Quadrature,
    /// The rows have been made into complex series along F1, so the imaginary
    /// part no longer belongs to F2.
    pub along_f1: bool,
    /// Window recorded in F2 and F1 (`WDW`), as an operation.
    pub windows: [Option<crate::process::Operation>; 2],
    /// Complex points of each F2 FID recorded (`TDeff`), when fewer than were
    /// acquired.
    pub f2_points: Option<usize>,
    /// F1 is recorded in magnitude (`PH_mod` 2).
    pub magnitude: bool,
    /// The linear prediction recorded for F1, as an operation.
    pub f1_prediction: Option<crate::process::Operation>,
    /// Non-uniform sampling still to be filled in: where along F1 each
    /// acquired increment belongs. Taken out once the grid is reconstructed.
    pub nus: Option<Nus>,
}

/// Which increments of a non-uniformly sampled F1 were acquired.
#[derive(Debug, Clone, PartialEq)]
pub struct Nus {
    /// For each F1 point acquired, in the order acquired, its place on the grid.
    pub points: Vec<usize>,
    /// Complex points along F1 had every increment been acquired.
    pub grid: usize,
}

impl Spectrum2D {
    /// Axes of F2 (columns) and F1 (rows).
    pub fn ppm_axes(&self) -> [PpmAxis; 2] {
        let cols = self.rows.first().map_or(0, Vec::len);
        [
            PpmAxis::new(&self.f2, cols),
            PpmAxis::new(&self.f1, self.rows.len()),
        ]
    }
}

/// Linear point ↔ ppm mapping, as nmrglue's `fileiobase.unit_conversion`.
/// The first point has the highest shift.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PpmAxis {
    pub first: f64,
    pub delta: f64,
}

impl PpmAxis {
    pub fn new(meta: &Metadata, size: usize) -> Self {
        let n = size as f64;
        let delta = -meta.sw_hz / (n * meta.obs_mhz);
        let first = meta.carrier_hz / meta.obs_mhz - delta * n / 2.0;
        Self { first, delta }
    }

    pub fn ppm(&self, index: f64) -> f64 {
        self.first + index * self.delta
    }

    pub fn index(&self, ppm: f64) -> f64 {
        (ppm - self.first) / self.delta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_spans_sw_centred_on_carrier() {
        let meta = Metadata {
            nucleus: "1H".into(),
            sw_hz: 8000.0,
            obs_mhz: 400.0,
            carrier_hz: 1880.0,
            group_delay: 0.0,
            solvent: None,
            phase: None,
            conjugate: false,
            signed: false,
            size: None,
            line_broadening: None,
            delay_at_ft: false,
        };
        let axis = PpmAxis::new(&meta, 1000);
        assert!((axis.ppm(500.0) - 4.7).abs() < 1e-12);
        assert!((axis.ppm(0.0) - axis.ppm(1000.0) - 20.0).abs() < 1e-12);
        assert!((axis.index(axis.ppm(123.0)) - 123.0).abs() < 1e-9);
    }
}
