use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("no Bruker experiment found in {0}")]
    NoExperiment(PathBuf),
    #[error("parameter {0} is missing")]
    MissingParameter(String),
    #[error("parameter {key} has an invalid value: {value:?}")]
    InvalidParameter { key: String, value: String },
    #[error("{path} holds {found} data points but TD requires {expected}")]
    Truncated {
        path: PathBuf,
        expected: usize,
        found: usize,
    },
    #[error("unsupported data: {0}")]
    Unsupported(String),
    #[error("molfile: {0}")]
    Molfile(String),
    #[error("{operation} must come {} the Fourier transform", if *before { "before" } else { "after" })]
    Order {
        operation: &'static str,
        before: bool,
    },
}

pub type Result<T> = std::result::Result<T, Error>;
