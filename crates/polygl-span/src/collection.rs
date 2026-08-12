use crate::{Diagnostic, Fixability, RenderError, Severity, SourceFile};

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
        debug_assert_eq!(diagnostic.severity, diagnostic.code.metadata().severity);
        debug_assert!(
            diagnostic.code.metadata().fixability != Fixability::Required
                || diagnostic.suggestion.is_some(),
            "{} requires a rewrite suggestion",
            diagnostic.code
        );
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

    pub fn render(&self, source: &SourceFile) -> Result<String, RenderError> {
        let rendered = self
            .entries
            .iter()
            .map(|diagnostic| diagnostic.render(source))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rendered.join("\n"))
    }
}
