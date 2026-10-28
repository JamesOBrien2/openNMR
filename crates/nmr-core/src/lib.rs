//! Reading, processing and analysis of NMR spectra.

pub mod analysis;
pub mod baseline;
pub mod bruker;
pub mod contour;
pub mod decimate;
pub mod deconvolution;
mod error;
pub mod fit;
pub mod molecule;
pub mod nus;
pub mod phase;
pub mod predict;
pub mod process;
pub mod process2d;
mod spectrum;

pub use error::{Error, Result};
pub use rustfft::num_complex::Complex64;
pub use spectrum::{Domain, Metadata, Nus, PpmAxis, Quadrature, Spectrum, Spectrum2D};
