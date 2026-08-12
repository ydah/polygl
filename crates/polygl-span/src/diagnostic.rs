use ariadne::{Config, IndexType, Label as AriadneLabel, Report, ReportKind, sources};
use std::ops::Range;

use crate::{DiagnosticCode, RenderError, SourceFile, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

impl Label {
    #[must_use]
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub span: Span,
    pub replacement: Option<String>,
    pub message: String,
    pub applicability: Applicability,
}

/// How safely a diagnostic fix can be applied without human review.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applicability {
    MachineApplicable,
    MaybeIncorrect,
    HasPlaceholders,
}

impl Applicability {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MachineApplicable => "machine-applicable",
            Self::MaybeIncorrect => "maybe-incorrect",
            Self::HasPlaceholders => "has-placeholders",
        }
    }
}

/// Zero-based UTF-16 position used by structured editor diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagnosticPosition {
    pub line: usize,
    pub character: usize,
}

/// A source range carrying both canonical bytes and editor coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagnosticRange {
    pub byte_start: usize,
    pub byte_end: usize,
    pub start: DiagnosticPosition,
    pub end: DiagnosticPosition,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructuredLabel {
    pub range: DiagnosticRange,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructuredSuggestion {
    pub range: DiagnosticRange,
    pub replacement: Option<String>,
    pub message: String,
    pub applicability: Applicability,
}

/// Renderer-independent diagnostic data for JSON, SARIF, and LSP adapters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructuredDiagnostic {
    pub severity: Severity,
    pub code: DiagnosticCode,
    pub message: String,
    pub source: String,
    pub primary_range: DiagnosticRange,
    pub labels: Vec<StructuredLabel>,
    pub notes: Vec<String>,
    pub suggestions: Vec<StructuredSuggestion>,
}

impl Suggestion {
    /// A complete textual edit which is safe to apply automatically.
    #[must_use]
    pub fn new(span: Span, replacement: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            span,
            replacement: Some(replacement.into()),
            message: message.into(),
            applicability: Applicability::MachineApplicable,
        }
    }

    /// A human-applicable rewrite for cases without one safe textual edit.
    #[must_use]
    pub fn rewrite(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            replacement: None,
            message: message.into(),
            applicability: Applicability::MaybeIncorrect,
        }
    }

    /// A textual edit whose replacement still contains user-filled fields.
    #[must_use]
    pub fn with_placeholders(
        span: Span,
        replacement: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            span,
            replacement: Some(replacement.into()),
            message: message.into(),
            applicability: Applicability::HasPlaceholders,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagnosticCode,
    pub message: String,
    pub primary_span: Span,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub suggestions: Vec<Suggestion>,
}

impl Diagnostic {
    #[must_use]
    pub fn new(
        severity: Severity,
        code: impl Into<DiagnosticCode>,
        message: impl Into<String>,
        primary_span: Span,
    ) -> Self {
        let code = code.into();
        debug_assert_eq!(severity, code.metadata().severity);
        Self {
            severity,
            code,
            message: message.into(),
            primary_span,
            labels: Vec::new(),
            notes: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_label(mut self, label: Label) -> Self {
        self.labels.push(label);
        self
    }

    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    #[must_use]
    pub fn with_suggestion(mut self, suggestion: Suggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    pub fn render(&self, source: &SourceFile) -> Result<String, RenderError> {
        self.primary_span.validate_for(source)?;
        for label in &self.labels {
            label.span.validate_for(source)?;
        }
        for suggestion in &self.suggestions {
            suggestion.span.validate_for(source)?;
        }

        let kind = match self.severity {
            Severity::Error => ReportKind::Error,
            Severity::Warning => ReportKind::Warning,
            Severity::Note => ReportKind::Advice,
        };
        let needs_eof_sentinel = self.has_empty_eof_span(source);
        let render_range = |span| Self::render_range(span, source, needs_eof_sentinel);
        let name = source.name().to_owned();
        let mut builder = Report::build(kind, (name.clone(), render_range(self.primary_span)))
            .with_code(self.code)
            .with_message(&self.message)
            .with_config(
                Config::default()
                    .with_color(false)
                    .with_index_type(IndexType::Byte),
            )
            .with_label(
                AriadneLabel::new((name.clone(), render_range(self.primary_span)))
                    .with_message(&self.message),
            );

        for label in &self.labels {
            builder.add_label(
                AriadneLabel::new((name.clone(), render_range(label.span)))
                    .with_message(&label.message),
            );
        }
        for note in &self.notes {
            builder.add_note(note);
        }
        for suggestion in &self.suggestions {
            builder.add_label(
                AriadneLabel::new((name.clone(), render_range(suggestion.span)))
                    .with_message(&suggestion.message),
            );
            match suggestion.replacement.as_deref() {
                None => builder.add_help(&suggestion.message),
                Some("") => {
                    builder.add_help(format!("{}: remove selected text", suggestion.message));
                }
                Some(replacement) => {
                    builder.add_help(format!(
                        "{}: replace with `{replacement}`",
                        suggestion.message
                    ));
                }
            }
        }

        let mut output = Vec::new();
        let mut rendered_source = source.text().to_owned();
        if needs_eof_sentinel {
            rendered_source.push(' ');
        }
        builder
            .finish()
            .write(sources([(name, rendered_source)]), &mut output)?;
        String::from_utf8(output).map_err(RenderError::InvalidOutput)
    }

    pub fn structured(&self, source: &SourceFile) -> Result<StructuredDiagnostic, RenderError> {
        let primary_range = structured_range(self.primary_span, source)?;
        let labels = self
            .labels
            .iter()
            .map(|label| {
                Ok(StructuredLabel {
                    range: structured_range(label.span, source)?,
                    message: label.message.clone(),
                })
            })
            .collect::<Result<Vec<_>, RenderError>>()?;
        let suggestions = self
            .suggestions
            .iter()
            .map(|suggestion| {
                Ok(StructuredSuggestion {
                    range: structured_range(suggestion.span, source)?,
                    replacement: suggestion.replacement.clone(),
                    message: suggestion.message.clone(),
                    applicability: suggestion.applicability,
                })
            })
            .collect::<Result<Vec<_>, RenderError>>()?;
        Ok(StructuredDiagnostic {
            severity: self.severity,
            code: self.code,
            message: self.message.clone(),
            source: source.name().to_owned(),
            primary_range,
            labels,
            notes: self.notes.clone(),
            suggestions,
        })
    }

    fn has_empty_eof_span(&self, source: &SourceFile) -> bool {
        let is_empty_eof = |span: Span| span.is_empty() && span.end() == source.len();
        is_empty_eof(self.primary_span)
            || self.labels.iter().any(|label| is_empty_eof(label.span))
            || self
                .suggestions
                .iter()
                .any(|suggestion| is_empty_eof(suggestion.span))
    }

    fn render_range(span: Span, source: &SourceFile, has_eof_sentinel: bool) -> Range<usize> {
        if has_eof_sentinel && span.is_empty() && span.end() == source.len() {
            span.start()..span.end() + 1
        } else {
            span.range()
        }
    }
}

fn structured_range(span: Span, source: &SourceFile) -> Result<DiagnosticRange, RenderError> {
    span.validate_for(source)?;
    let start = source.position(span.start())?;
    let end = source.position(span.end())?;
    Ok(DiagnosticRange {
        byte_start: span.start(),
        byte_end: span.end(),
        start: DiagnosticPosition {
            line: start.line - 1,
            character: start.utf16_column - 1,
        },
        end: DiagnosticPosition {
            line: end.line - 1,
            character: end.utf16_column - 1,
        },
    })
}
