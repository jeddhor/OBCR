//! Parsers for every original Operation Body Count file (DESIGN §7.1).
//!
//! Every parser is a pure function from bytes to typed values. Malformed input
//! produces a [`FormatError`], never a panic. Formats are specified in
//! `docs/re/DATA_FORMATS.md`.

pub mod audio;
mod bytes;
pub mod config;
pub mod datadir;
mod error;
pub mod exe;
pub mod flc;
pub mod maps;
pub mod release;
pub mod vgagraph;
pub mod vswap;

pub use datadir::DataDir;
pub use error::{FormatError, Result};
