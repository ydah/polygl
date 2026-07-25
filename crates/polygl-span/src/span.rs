use std::error::Error;
use std::fmt;
use std::ops::Range;

use crate::{SourceFile, SourceId};

/// A validated half-open byte range in one source file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    source: SourceId,
    start: usize,
    end: usize,
}

impl Span {
    pub fn new(source: SourceId, start: usize, end: usize) -> Result<Self, SpanError> {
        if start > end {
            return Err(SpanError::Reversed { start, end });
        }
        Ok(Self { source, start, end })
    }

    #[must_use]
    pub const fn source(self) -> SourceId {
        self.source
    }

    #[must_use]
    pub const fn start(self) -> usize {
        self.start
    }

    #[must_use]
    pub const fn end(self) -> usize {
        self.end
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.end - self.start
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    #[must_use]
    pub const fn range(self) -> Range<usize> {
        self.start..self.end
    }

    pub fn validate_for(self, source: &SourceFile) -> Result<(), SpanError> {
        if self.source != source.id() {
            return Err(SpanError::SourceMismatch {
                expected: source.id(),
                actual: self.source,
            });
        }
        source.validate_offset(self.start)?;
        source.validate_offset(self.end)
    }

    pub fn merge(self, other: Self) -> Result<Self, SpanError> {
        if self.source != other.source {
            return Err(SpanError::SourceMismatch {
                expected: self.source,
                actual: other.source,
            });
        }
        Ok(Self {
            source: self.source,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpanError {
    Reversed {
        start: usize,
        end: usize,
    },
    SourceMismatch {
        expected: SourceId,
        actual: SourceId,
    },
    OutOfBounds {
        offset: usize,
        source_len: usize,
    },
    InvalidUtf8Boundary {
        offset: usize,
    },
}

impl fmt::Display for SpanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reversed { start, end } => {
                write!(formatter, "span start {start} is after end {end}")
            }
            Self::SourceMismatch { expected, actual } => write!(
                formatter,
                "span belongs to source {}, expected {}",
                actual.raw(),
                expected.raw()
            ),
            Self::OutOfBounds { offset, source_len } => {
                write!(
                    formatter,
                    "byte offset {offset} exceeds source length {source_len}"
                )
            }
            Self::InvalidUtf8Boundary { offset } => {
                write!(formatter, "byte offset {offset} is not a UTF-8 boundary")
            }
        }
    }
}

impl Error for SpanError {}

#[cfg(test)]
mod tests {
    use crate::{SourceFile, SourceId, Span, SpanError};

    #[test]
    fn validates_empty_eof_and_rejects_invalid_ranges() {
        let source = SourceFile::new(SourceId::new(1), "main.rb", "α");
        assert!(source.span(source.len(), source.len()).unwrap().is_empty());
        assert_eq!(
            Span::new(source.id(), 2, 1),
            Err(SpanError::Reversed { start: 2, end: 1 })
        );
        assert!(
            Span::new(source.id(), 0, 3)
                .unwrap()
                .validate_for(&source)
                .is_err()
        );
        assert!(
            Span::new(source.id(), 0, 1)
                .unwrap()
                .validate_for(&source)
                .is_err()
        );
    }

    #[test]
    fn merges_only_spans_from_the_same_source() {
        let left = Span::new(SourceId::new(1), 2, 4).unwrap();
        let right = Span::new(SourceId::new(1), 0, 3).unwrap();
        assert_eq!(left.merge(right).unwrap().range(), 0..4);
        let other = Span::new(SourceId::new(2), 0, 1).unwrap();
        assert!(left.merge(other).is_err());
    }
}
