//! Error type shared across the crate.

use std::fmt;

/// Result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors produced while loading property definitions or decoding an SPB file.
#[derive(Debug)]
pub enum Error {
    /// An underlying I/O failure (reading inputs, writing outputs).
    Io(std::io::Error),
    /// The property definition set could not be loaded.
    Propdefs(String),
    /// The SPB byte stream was malformed or used an unsupported feature.
    Format(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(err) => write!(f, "{err}"),
            Error::Propdefs(msg) => write!(f, "{msg}"),
            Error::Format(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

/// Build a [`Error::Format`] from a format string.
macro_rules! format_err {
    ($($arg:tt)*) => { $crate::error::Error::Format(format!($($arg)*)) };
}
pub(crate) use format_err;
