use std::error::Error;
use std::fmt;
use std::str::Utf8Error;

use crate::{Span, SpanError};

/// Stable identifier for one source file within a compilation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceId(u32);

impl SourceId {
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// A one-based position derived from a canonical byte offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourcePosition {
    pub line: usize,
    pub byte_column: usize,
    pub scalar_column: usize,
    pub utf16_column: usize,
}

/// Owned UTF-8 source plus an index of line-start byte offsets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    id: SourceId,
    name: String,
    text: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    #[must_use]
    pub fn new(id: SourceId, name: impl Into<String>, text: impl Into<String>) -> Self {
        let text = text.into();
        let mut line_starts = vec![0];
        line_starts.extend(
            text.bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
        );
        Self {
            id,
            name: name.into(),
            text,
            line_starts,
        }
    }

    pub fn from_bytes(
        id: SourceId,
        name: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Result<Self, SourceError> {
        let text = String::from_utf8(bytes).map_err(|error| SourceError {
            utf8_error: error.utf8_error(),
        })?;
        Ok(Self::new(id, name, text))
    }

    #[must_use]
    pub const fn id(&self) -> SourceId {
        self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.text.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn span(&self, start: usize, end: usize) -> Result<Span, SpanError> {
        let span = Span::new(self.id, start, end)?;
        span.validate_for(self)?;
        Ok(span)
    }

    pub fn position(&self, offset: usize) -> Result<SourcePosition, SpanError> {
        self.validate_offset(offset)?;
        let line_index = self
            .line_starts
            .partition_point(|line_start| *line_start <= offset)
            .saturating_sub(1);
        let line_start = self.line_starts[line_index];
        let prefix = &self.text[line_start..offset];
        Ok(SourcePosition {
            line: line_index + 1,
            byte_column: offset - line_start + 1,
            scalar_column: prefix.chars().count() + 1,
            utf16_column: prefix.encode_utf16().count() + 1,
        })
    }

    pub(crate) fn validate_offset(&self, offset: usize) -> Result<(), SpanError> {
        if offset > self.text.len() {
            return Err(SpanError::OutOfBounds {
                offset,
                source_len: self.text.len(),
            });
        }
        if !self.text.is_char_boundary(offset) {
            return Err(SpanError::InvalidUtf8Boundary { offset });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceError {
    utf8_error: Utf8Error,
}

impl SourceError {
    #[must_use]
    pub const fn utf8_error(self) -> Utf8Error {
        self.utf8_error
    }
}

impl fmt::Display for SourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "source is not valid UTF-8: {}", self.utf8_error)
    }
}

impl Error for SourceError {}

#[cfg(test)]
mod tests {
    use super::{SourceFile, SourceId, SourcePosition};

    #[test]
    fn indexes_crlf_and_unicode_from_byte_offsets() {
        let source = SourceFile::new(SourceId::new(7), "unicode.rb", "α\r\nβ");

        assert_eq!(
            source.position(2).unwrap(),
            SourcePosition {
                line: 1,
                byte_column: 3,
                scalar_column: 2,
                utf16_column: 2,
            }
        );
        assert_eq!(
            source.position(4).unwrap(),
            SourcePosition {
                line: 2,
                byte_column: 1,
                scalar_column: 1,
                utf16_column: 1,
            }
        );
        assert_eq!(source.position(source.len()).unwrap().line, 2);
    }

    #[test]
    fn rejects_invalid_utf8_bytes_and_mid_scalar_offsets() {
        assert!(SourceFile::from_bytes(SourceId::new(0), "bad.rb", vec![0xff]).is_err());
        let source = SourceFile::new(SourceId::new(0), "unicode.rb", "α");
        assert!(source.position(1).is_err());
    }
}
