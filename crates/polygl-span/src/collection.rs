use std::error::Error;
use std::fmt;

use crate::{Diagnostic, Fixability, RenderError, Severity, SourceFile, StructuredDiagnostic};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticInvariantError {
    SeverityMismatch,
    MissingRequiredFix,
}

impl fmt::Display for DiagnosticInvariantError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SeverityMismatch => "diagnostic severity does not match its registered code",
            Self::MissingRequiredFix => "diagnostic code requires at least one suggested fix",
        })
    }
}

impl Error for DiagnosticInvariantError {}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diagnostics {
    entries: Vec<Diagnostic>,
}

impl Diagnostics {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        if !self.entries.contains(&diagnostic) {
            self.entries.push(diagnostic);
        }
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Diagnostic> {
        self.entries.iter()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.entries
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    }

    /// Validates the structural contract at an adapter/compiler trust boundary.
    pub fn validate(&self) -> Result<(), DiagnosticInvariantError> {
        for diagnostic in &self.entries {
            if diagnostic.severity != diagnostic.code.metadata().severity {
                return Err(DiagnosticInvariantError::SeverityMismatch);
            }
            if diagnostic.code.metadata().fixability == Fixability::Required
                && diagnostic.suggestions.is_empty()
            {
                return Err(DiagnosticInvariantError::MissingRequiredFix);
            }
        }
        Ok(())
    }

    pub fn render(&self, source: &SourceFile) -> Result<String, RenderError> {
        self.render_with_color(source, false)
    }

    pub fn render_with_color(
        &self,
        source: &SourceFile,
        color: bool,
    ) -> Result<String, RenderError> {
        let rendered = self
            .entries
            .iter()
            .map(|diagnostic| diagnostic.render_with_color(source, color))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rendered.join("\n"))
    }

    pub fn structured(
        &self,
        source: &SourceFile,
    ) -> Result<Vec<StructuredDiagnostic>, RenderError> {
        self.entries
            .iter()
            .map(|diagnostic| diagnostic.structured(source))
            .collect()
    }
}
