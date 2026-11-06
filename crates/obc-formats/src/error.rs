use std::path::PathBuf;

/// Error from reading or parsing an original data file.
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("missing data file {0}")]
    Missing(String),
    #[error("{what}: truncated (need {need} bytes at offset {offset}, have {have})")]
    Truncated { what: &'static str, offset: usize, need: usize, have: usize },
    #[error("{what}: {msg}")]
    Invalid { what: &'static str, msg: String },
}

pub type Result<T> = std::result::Result<T, FormatError>;

pub(crate) fn invalid(what: &'static str, msg: impl Into<String>) -> FormatError {
    FormatError::Invalid { what, msg: msg.into() }
}
