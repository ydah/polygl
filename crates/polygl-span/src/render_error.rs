use std::error::Error;
use std::fmt;
use std::io;
use std::string::FromUtf8Error;

use crate::SpanError;

#[derive(Debug)]
pub enum RenderError {
    InvalidSpan(SpanError),
    Io(io::Error),
    InvalidOutput(FromUtf8Error),
}

impl fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSpan(error) => error.fmt(formatter),
            Self::Io(error) => error.fmt(formatter),
            Self::InvalidOutput(error) => error.fmt(formatter),
        }
    }
}

impl Error for RenderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidSpan(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::InvalidOutput(error) => Some(error),
        }
    }
}

impl From<SpanError> for RenderError {
    fn from(error: SpanError) -> Self {
        Self::InvalidSpan(error)
    }
}

impl From<io::Error> for RenderError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
