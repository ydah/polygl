//! Perl source to Common Core HIR adapter backed by Tree-sitter.

mod annotation;
mod lowerer;

use polygl_adapter_api::{FeatureTag, LanguageAdapter, LowerCtx};
use polygl_adapter_treesitter_util::recovery_diagnostics;
use polygl_hir::Module;
use polygl_span::{Diagnostic, Diagnostics, Severity, SourceFile};
use tree_sitter::Parser;

use crate::annotation::Annotations;
use crate::lowerer::Lowerer;

const CAPABILITIES: &[FeatureTag] = &[
    FeatureTag::Core,
    FeatureTag::Tier1,
    FeatureTag::Tier2,
    FeatureTag::Arrays,
    FeatureTag::Maps,
    FeatureTag::Classes,
    FeatureTag::Meshes,
    FeatureTag::SceneNodes,
    FeatureTag::Cameras,
    FeatureTag::Textures,
    FeatureTag::Shaders,
];

#[derive(Clone, Copy, Debug, Default)]
pub struct PerlAdapter;

impl LanguageAdapter for PerlAdapter {
    fn id(&self) -> &'static str {
        "perl"
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["pl"]
    }

    fn lower(
        &self,
        source: &SourceFile,
        context: &mut LowerCtx<'_>,
    ) -> Result<Module, Diagnostics> {
        let mut parser = Parser::new();
        parser
            .set_language(&ts_parser_perl::LANGUAGE.into())
            .expect("the pinned Perl grammar must match Tree-sitter");
        let Some(tree) = parser.parse(source.text(), None) else {
            let mut diagnostics = Diagnostics::new();
            let span = source
                .span(0, source.len())
                .expect("the complete source range is valid");
            diagnostics.push(Diagnostic::new(
                Severity::Error,
                "E0100",
                "Tree-sitter did not return a Perl syntax tree",
                span,
            ));
            return Err(diagnostics);
        };
        let diagnostics = recovery_diagnostics(source, tree.root_node());
        if diagnostics.has_errors() {
            return Err(diagnostics);
        }

        let annotations = Annotations::parse(source)?;
        Lowerer::new(source, context, annotations).lower_program(tree.root_node())
    }

    fn capabilities(&self) -> &'static [FeatureTag] {
        CAPABILITIES
    }
}

#[cfg(test)]
mod tests;
