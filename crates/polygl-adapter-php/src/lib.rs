//! PHP source to Common Core HIR adapter backed by Mago.

mod annotation;
mod class;
mod expression;
mod item;
mod lowerer;
mod statement;

use mago_allocator::LocalArena;
use mago_database::file::FileId;
use mago_span::HasSpan;
use mago_syntax::parser::parse_file_content;
use polygl_adapter_api::{FeatureTag, LanguageAdapter, LowerCtx};
use polygl_hir::Module;
use polygl_span::{Diagnostic, Diagnostics, Severity, SourceFile};

use crate::lowerer::Lowerer;

const CAPABILITIES: &[FeatureTag] = &[
    FeatureTag::Core,
    FeatureTag::Tier1,
    FeatureTag::Tier2,
    FeatureTag::Arrays,
    FeatureTag::Maps,
    FeatureTag::Classes,
    FeatureTag::Shaders,
];

#[derive(Clone, Copy, Debug, Default)]
pub struct PhpAdapter;

impl LanguageAdapter for PhpAdapter {
    fn id(&self) -> &'static str {
        "php"
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["php"]
    }

    fn lower(
        &self,
        source: &SourceFile,
        context: &mut LowerCtx<'_>,
    ) -> Result<Module, Diagnostics> {
        let arena = LocalArena::new();
        let program = parse_file_content(&arena, FileId::zero(), source.text().as_bytes());
        let mut diagnostics = Diagnostics::new();
        for error in program.errors {
            let span = source
                .span(error.start_offset() as usize, error.end_offset() as usize)
                .expect("Mago diagnostics must use source byte boundaries");
            diagnostics.push(Diagnostic::new(
                Severity::Error,
                "E0100",
                error.to_string(),
                span,
            ));
        }
        if diagnostics.has_errors() {
            return Err(diagnostics);
        }
        let annotations = annotation::parse_annotations(source, program, &mut diagnostics);
        Lowerer::new(source, context, annotations, diagnostics).lower_program(program)
    }

    fn capabilities(&self) -> &'static [FeatureTag] {
        CAPABILITIES
    }
}

#[cfg(test)]
mod tests;
