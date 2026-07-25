use ariadne::{Config, IndexType, Label as AriadneLabel, Report, ReportKind, sources};
use std::ops::Range;

use crate::{RenderError, SourceFile, Span};

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
    pub replacement: String,
    pub message: String,
}

impl Suggestion {
    #[must_use]
    pub fn new(span: Span, replacement: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            span,
            replacement: replacement.into(),
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub primary_span: Span,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub suggestion: Option<Suggestion>,
}

impl Diagnostic {
    #[must_use]
    pub fn new(
        severity: Severity,
        code: impl Into<String>,
        message: impl Into<String>,
        primary_span: Span,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            primary_span,
            labels: Vec::new(),
            notes: Vec::new(),
            suggestion: None,
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
        self.suggestion = Some(suggestion);
        self
    }

    pub fn render(&self, source: &SourceFile) -> Result<String, RenderError> {
        self.primary_span.validate_for(source)?;
        for label in &self.labels {
            label.span.validate_for(source)?;
        }
        if let Some(suggestion) = &self.suggestion {
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
            .with_code(&self.code)
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
        if let Some(suggestion) = &self.suggestion {
            builder.add_label(
                AriadneLabel::new((name.clone(), render_range(suggestion.span)))
                    .with_message(&suggestion.message),
            );
            builder.add_help(format!(
                "{}: replace with `{}`",
                suggestion.message, suggestion.replacement
            ));
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

    fn has_empty_eof_span(&self, source: &SourceFile) -> bool {
        let is_empty_eof = |span: Span| span.is_empty() && span.end() == source.len();
        is_empty_eof(self.primary_span)
            || self.labels.iter().any(|label| is_empty_eof(label.span))
            || self
                .suggestion
                .as_ref()
                .is_some_and(|suggestion| is_empty_eof(suggestion.span))
    }

    fn render_range(span: Span, source: &SourceFile, has_eof_sentinel: bool) -> Range<usize> {
        if has_eof_sentinel && span.is_empty() && span.end() == source.len() {
            span.start()..span.end() + 1
        } else {
            span.range()
        }
    }
}
