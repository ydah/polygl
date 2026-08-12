use std::error::Error;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum ConformanceError {
    Io(io::Error),
    InvalidManifest(String),
    InvalidName(String),
    InvalidFrame(String),
    FrameMismatch {
        expected_renderer: String,
        actual_renderer: String,
    },
    SnapshotMismatch {
        layer: &'static str,
        case: String,
        subject: String,
    },
    Compile {
        case: String,
        message: String,
    },
    DuplicateLanguage(String),
    NotEnoughNeutralPrograms,
}

impl fmt::Display for ConformanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::InvalidManifest(message) => {
                write!(formatter, "invalid conformance manifest: {message}")
            }
            Self::InvalidName(name) => write!(formatter, "invalid conformance name `{name}`"),
            Self::InvalidFrame(message) => write!(formatter, "invalid frame: {message}"),
            Self::FrameMismatch {
                expected_renderer,
                actual_renderer,
            } => write!(
                formatter,
                "L1 frame mismatch for renderer `{expected_renderer}` vs `{actual_renderer}`"
            ),
            Self::SnapshotMismatch {
                layer,
                case,
                subject,
            } => write!(
                formatter,
                "{layer} snapshot mismatch for case `{case}` and `{subject}`"
            ),
            Self::Compile { case, message } => {
                write!(
                    formatter,
                    "failed to compile conformance case `{case}`: {message}"
                )
            }
            Self::DuplicateLanguage(language) => {
                write!(
                    formatter,
                    "L3 language `{language}` is registered more than once"
                )
            }
            Self::NotEnoughNeutralPrograms => {
                formatter.write_str("L3 comparison requires at least two programs")
            }
        }
    }
}

impl Error for ConformanceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for ConformanceError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
