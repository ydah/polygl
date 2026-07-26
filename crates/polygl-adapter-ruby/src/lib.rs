//! Ruby source to HIR adapter backed by Prism.

mod annotation;
mod class;
mod expression;
mod item;
mod literal;
mod lowerer;
mod operator;
mod statement;

use polygl_adapter_api::{FeatureTag, LanguageAdapter, LowerCtx};
use polygl_hir::Module;
use polygl_span::{Diagnostic, Diagnostics, Severity, SourceFile};

use crate::annotation::parse_annotations;
use crate::lowerer::Lowerer;

const CAPABILITIES: &[FeatureTag] = &[
    FeatureTag::Core,
    FeatureTag::Tier1,
    FeatureTag::Tier2,
    FeatureTag::Arrays,
    FeatureTag::Maps,
    FeatureTag::Classes,
    FeatureTag::TimesBlockSugar,
    FeatureTag::EachBlockSugar,
    FeatureTag::TruthinessSugar,
    FeatureTag::Shaders,
];

#[derive(Clone, Copy, Debug, Default)]
pub struct RubyAdapter;

impl LanguageAdapter for RubyAdapter {
    fn id(&self) -> &'static str {
        "ruby"
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["rb"]
    }

    fn lower(
        &self,
        source: &SourceFile,
        context: &mut LowerCtx<'_>,
    ) -> Result<Module, Diagnostics> {
        let parsed = ruby_prism::parse(source.text().as_bytes());
        let mut diagnostics = Diagnostics::new();
        for error in parsed.errors() {
            let location = error.location();
            let span = source
                .span(location.start_offset(), location.end_offset())
                .expect("Prism diagnostics must use source byte boundaries");
            diagnostics.push(Diagnostic::new(
                Severity::Error,
                "E0100",
                error.message(),
                span,
            ));
        }
        if diagnostics.has_errors() {
            return Err(diagnostics);
        }

        let annotations = parse_annotations(source, &parsed, &mut diagnostics);
        if diagnostics.has_errors() {
            return Err(diagnostics);
        }
        let program = parsed
            .node()
            .as_program_node()
            .expect("Prism parse roots are program nodes");
        Lowerer::new(source, context, annotations).lower_program(&program)
    }

    fn capabilities(&self) -> &'static [FeatureTag] {
        CAPABILITIES
    }
}

#[cfg(test)]
mod tests;
