use std::collections::HashMap;

use polygl_span::{SourceFile, SourceId, Span};

use crate::EmitError;

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub(crate) struct SourceCatalog<'source> {
    sources: &'source [SourceFile],
    indices: HashMap<SourceId, usize>,
}

impl<'source> SourceCatalog<'source> {
    pub(crate) fn new(sources: &'source [SourceFile]) -> Result<Self, EmitError> {
        let mut indices = HashMap::new();
        for (index, source) in sources.iter().enumerate() {
            if indices.insert(source.id(), index).is_some() {
                return Err(EmitError::DuplicateSource(source.id()));
            }
        }
        Ok(Self { sources, indices })
    }

    pub(crate) fn locate(&self, span: Span) -> Result<SourceLocation<'source>, EmitError> {
        let index = *self
            .indices
            .get(&span.source())
            .ok_or(EmitError::MissingSource(span.source()))?;
        let source = &self.sources[index];
        span.validate_for(source).map_err(EmitError::InvalidSpan)?;
        let position = source
            .position(span.start())
            .map_err(EmitError::InvalidSpan)?;
        Ok(SourceLocation {
            index,
            source,
            line: position.line,
            utf16_column: position.utf16_column,
        })
    }

    pub(crate) fn sources(&self) -> &'source [SourceFile] {
        self.sources
    }
}

pub(crate) struct SourceLocation<'source> {
    pub(crate) index: usize,
    pub(crate) source: &'source SourceFile,
    pub(crate) line: usize,
    pub(crate) utf16_column: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Mapping {
    generated_line: usize,
    generated_column: usize,
    span: Span,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SourceMapBuilder {
    mappings: Vec<Mapping>,
}

impl SourceMapBuilder {
    pub(crate) fn add(&mut self, generated_line: usize, generated_column: usize, span: Span) {
        if self.mappings.last().is_some_and(|mapping| {
            mapping.generated_line == generated_line
                && mapping.generated_column == generated_column
                && mapping.span == span
        }) {
            return;
        }
        self.mappings.push(Mapping {
            generated_line,
            generated_column,
            span,
        });
    }

    pub(crate) fn to_json(
        &self,
        output_name: &str,
        generated_line_offset: usize,
        catalog: &SourceCatalog<'_>,
        include_sources_content: bool,
    ) -> Result<String, EmitError> {
        let mappings = self.encode(generated_line_offset, catalog)?;
        let sources = catalog
            .sources()
            .iter()
            .map(SourceFile::name)
            .collect::<Vec<_>>();
        let mut source_map = serde_json::json!({
            "version": 3,
            "file": output_name,
            "sources": sources,
            "names": Vec::<String>::new(),
            "mappings": mappings,
        });
        if include_sources_content {
            source_map["sourcesContent"] = serde_json::json!(
                catalog
                    .sources()
                    .iter()
                    .map(SourceFile::text)
                    .collect::<Vec<_>>()
            );
        }
        Ok(source_map.to_string())
    }

    fn encode(
        &self,
        generated_line_offset: usize,
        catalog: &SourceCatalog<'_>,
    ) -> Result<String, EmitError> {
        let mut output = String::new();
        let mut current_line = 0;
        let mut previous_source = 0_i64;
        let mut previous_original_line = 0_i64;
        let mut previous_original_column = 0_i64;
        let mut previous_generated_column = 0_i64;
        let mut first_segment = true;

        for mapping in &self.mappings {
            let generated_line = mapping.generated_line + generated_line_offset;
            while current_line < generated_line {
                output.push(';');
                current_line += 1;
                first_segment = true;
                previous_generated_column = 0;
            }
            if !first_segment {
                output.push(',');
            }
            first_segment = false;

            let location = catalog.locate(mapping.span)?;
            let source = i64::try_from(location.index).expect("source indices fit in i64");
            let original_line =
                i64::try_from(location.line - 1).expect("source line numbers fit in i64");
            let original_column =
                i64::try_from(location.utf16_column - 1).expect("source columns fit in i64");

            let generated_column =
                i64::try_from(mapping.generated_column).expect("generated columns fit in i64");
            encode_vlq(generated_column - previous_generated_column, &mut output);
            encode_vlq(source - previous_source, &mut output);
            encode_vlq(original_line - previous_original_line, &mut output);
            encode_vlq(original_column - previous_original_column, &mut output);

            previous_source = source;
            previous_original_line = original_line;
            previous_original_column = original_column;
            previous_generated_column = generated_column;
        }

        Ok(output)
    }
}

fn encode_vlq(value: i64, output: &mut String) {
    let mut value = if value < 0 {
        ((-value) << 1) | 1
    } else {
        value << 1
    };
    loop {
        let mut digit = u8::try_from(value & 31).expect("VLQ digit is five bits");
        value >>= 5;
        if value != 0 {
            digit |= 32;
        }
        output.push(char::from(BASE64[usize::from(digit)]));
        if value == 0 {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use polygl_span::{SourceFile, SourceId};

    use super::{SourceCatalog, SourceMapBuilder, encode_vlq};

    #[test]
    fn encodes_base64_vlq_signed_values() {
        let mut encoded = String::new();
        for value in [0, 1, -1, 16, -16] {
            encode_vlq(value, &mut encoded);
            encoded.push(' ');
        }
        assert_eq!(encoded, "A C D gB hB ");
    }

    #[test]
    fn encodes_generated_deltas_and_utf16_source_columns() {
        let source = SourceFile::new(SourceId::new(1), "unicode.rb", "😀x");
        let mut mappings = SourceMapBuilder::default();
        mappings.add(0, 0, source.span(0, 4).unwrap());
        mappings.add(0, 5, source.span(4, 5).unwrap());
        let catalog = SourceCatalog::new(std::slice::from_ref(&source)).unwrap();

        assert_eq!(mappings.encode(0, &catalog).unwrap(), "AAAA,KAAE");
    }
}
