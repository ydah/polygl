use std::error::Error;
use std::fmt;

use polygl_span::{SourceId, SpanError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmitError {
    DuplicateSource(SourceId),
    MissingSource(SourceId),
    InvalidSpan(SpanError),
    InvalidProgram(&'static str),
}

impl fmt::Display for EmitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateSource(source) => {
                write!(formatter, "source id {} is registered twice", source.raw())
            }
            Self::MissingSource(source) => {
                write!(formatter, "source id {} is not registered", source.raw())
            }
            Self::InvalidSpan(error) => error.fmt(formatter),
            Self::InvalidProgram(reason) => write!(formatter, "invalid Host LIR: {reason}"),
        }
    }
}

impl Error for EmitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidSpan(error) => Some(error),
            Self::DuplicateSource(_) | Self::MissingSource(_) | Self::InvalidProgram(_) => None,
        }
    }
}
