use std::error::Error;
use std::fmt;

use polygl_adapter_api::{LanguageAdapter, LowerCtx};
use polygl_adapter_perl::PerlAdapter;
use polygl_adapter_php::PhpAdapter;
use polygl_adapter_ruby::RubyAdapter;
use polygl_backend_glsl::{GlslArtifacts, GlslBackend};
use polygl_backend_js::{Artifacts, BuildMode, JavaScriptBackend, SourceMapMode};
use polygl_lir::AssetReference;
use polygl_span::{
    Diagnostic, DiagnosticCode, DiagnosticInvariantError, Diagnostics, Severity, SourceFile,
};
use polygl_types::TypedModule;

use crate::{AdapterRegistry, BuiltinTable};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompileOptions {
    pub mode: BuildMode,
    pub source_map: SourceMapMode,
    pub sources_content: bool,
}

impl CompileOptions {
    #[must_use]
    pub const fn check() -> Self {
        Self {
            mode: BuildMode::Debug,
            source_map: SourceMapMode::None,
            sources_content: false,
        }
    }
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            mode: BuildMode::Debug,
            source_map: SourceMapMode::External,
            sources_content: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FrontendOutput {
    pub typed: TypedModule,
}

#[derive(Clone, Debug)]
pub struct CompileOutput {
    pub typed: TypedModule,
    pub javascript: Artifacts,
    pub shaders: GlslArtifacts,
    pub assets: Vec<AssetReference>,
    pub warnings: Diagnostics,
}

pub struct Compiler<'adapter> {
    adapters: AdapterRegistry<'adapter>,
}

impl<'adapter> Compiler<'adapter> {
    #[must_use]
    pub const fn new(adapters: AdapterRegistry<'adapter>) -> Self {
        Self { adapters }
    }

    #[must_use]
    pub const fn adapters(&self) -> &AdapterRegistry<'adapter> {
        &self.adapters
    }

    pub fn analyze(
        &self,
        source: &SourceFile,
        language_id: &str,
    ) -> Result<FrontendOutput, CompileError> {
        let adapter = self
            .adapters
            .by_id(language_id)
            .ok_or_else(|| CompileError::UnsupportedLanguage(language_id.to_owned()))?;
        self.analyze_with_adapter(source, adapter)
    }

    pub fn analyze_with_adapter(
        &self,
        source: &SourceFile,
        adapter: &dyn LanguageAdapter,
    ) -> Result<FrontendOutput, CompileError> {
        let mut context = LowerCtx::new(&BuiltinTable);
        let hir = adapter
            .lower(source, &mut context)
            .map_err(|diagnostics| validated_diagnostics("adapter", diagnostics))?;
        let typed = polygl_types::analyze(&hir)
            .map_err(|diagnostics| validated_diagnostics("type analyzer", diagnostics))?;
        Ok(FrontendOutput { typed })
    }

    pub fn compile(
        &self,
        source: &SourceFile,
        language_id: &str,
        options: CompileOptions,
    ) -> Result<CompileOutput, CompileError> {
        validate_options(source, options)?;
        let frontend = self.analyze(source, language_id)?;
        self.compile_typed(source, frontend.typed, options)
    }

    fn compile_typed(
        &self,
        source: &SourceFile,
        typed: TypedModule,
        options: CompileOptions,
    ) -> Result<CompileOutput, CompileError> {
        let lir = polygl_lir::lower(&typed);
        let split = polygl_lir::split(&lir)
            .map_err(|diagnostics| validated_split_diagnostics("LIR split", diagnostics))?;
        split
            .warnings
            .validate()
            .map_err(|reason| CompileError::InvalidDiagnostics {
                producer: "LIR split",
                reason,
            })?;
        let javascript = JavaScriptBackend::new(options.mode)
            .with_source_map_mode(options.source_map)
            .with_sources_content(options.sources_content)
            .generate(&split.host, std::slice::from_ref(source))
            .map_err(CompileError::JavaScript)?;
        let shaders = GlslBackend::new()
            .generate(&split.gpu)
            .map_err(CompileError::Glsl)?;
        Ok(CompileOutput {
            typed,
            javascript,
            shaders,
            assets: split.assets,
            warnings: split.warnings,
        })
    }
}

fn validate_options(source: &SourceFile, options: CompileOptions) -> Result<(), CompileError> {
    if options.sources_content && options.source_map == SourceMapMode::None {
        let span = source
            .span(0, 0)
            .expect("the start of every source is a valid empty span");
        let mut diagnostics = Diagnostics::new();
        diagnostics.push(
            Diagnostic::new(
                Severity::Error,
                DiagnosticCode::E0001,
                "sources_content requires an external or inline source map",
                span,
            )
            .with_note("select SourceMapMode::External or SourceMapMode::Inline, or disable sources_content"),
        );
        return Err(CompileError::Configuration(diagnostics));
    }
    Ok(())
}

impl Compiler<'static> {
    /// Creates the standard compiler with the bundled Ruby, PHP, and Perl adapters.
    #[must_use]
    pub fn standard() -> Self {
        let adapters = AdapterRegistry::from_adapters([
            &RubyAdapter as &'static dyn LanguageAdapter,
            &PhpAdapter as &'static dyn LanguageAdapter,
            &PerlAdapter as &'static dyn LanguageAdapter,
        ])
        .expect("bundled adapters must have a valid, unambiguous registry");
        Self::new(adapters)
    }
}

#[derive(Debug)]
pub enum CompileError {
    UnsupportedLanguage(String),
    Configuration(Diagnostics),
    Frontend(Diagnostics),
    Split(Diagnostics),
    JavaScript(polygl_backend_js::EmitError),
    Glsl(polygl_backend_glsl::EmitError),
    InvalidDiagnostics {
        producer: &'static str,
        reason: DiagnosticInvariantError,
    },
}

impl CompileError {
    #[must_use]
    pub const fn diagnostics(&self) -> Option<&Diagnostics> {
        match self {
            Self::Configuration(diagnostics)
            | Self::Frontend(diagnostics)
            | Self::Split(diagnostics) => Some(diagnostics),
            Self::UnsupportedLanguage(_)
            | Self::JavaScript(_)
            | Self::Glsl(_)
            | Self::InvalidDiagnostics { .. } => None,
        }
    }

    #[must_use]
    pub fn render(&self, source: &SourceFile) -> String {
        match self.diagnostics() {
            Some(diagnostics) => diagnostics
                .render(source)
                .unwrap_or_else(|error| format!("failed to render diagnostics: {error}")),
            None => self.to_string(),
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedLanguage(language) => {
                write!(formatter, "unsupported source language `{language}`")
            }
            Self::Configuration(_) => formatter.write_str("compiler configuration is invalid"),
            Self::Frontend(_) => formatter.write_str("source lowering or type analysis failed"),
            Self::Split(_) => formatter.write_str("Host/GPU boundary validation failed"),
            Self::JavaScript(error) => write!(formatter, "JavaScript generation failed: {error}"),
            Self::Glsl(error) => write!(formatter, "GLSL generation failed: {error}"),
            Self::InvalidDiagnostics { producer, reason } => {
                write!(
                    formatter,
                    "{producer} produced an invalid diagnostic: {reason}"
                )
            }
        }
    }
}

impl Error for CompileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::JavaScript(error) => Some(error),
            Self::Glsl(error) => Some(error),
            Self::UnsupportedLanguage(_)
            | Self::Configuration(_)
            | Self::Frontend(_)
            | Self::Split(_) => None,
            Self::InvalidDiagnostics { reason, .. } => Some(reason),
        }
    }
}

fn validated_diagnostics(producer: &'static str, diagnostics: Diagnostics) -> CompileError {
    match diagnostics.validate() {
        Ok(()) => CompileError::Frontend(diagnostics),
        Err(reason) => CompileError::InvalidDiagnostics { producer, reason },
    }
}

fn validated_split_diagnostics(producer: &'static str, diagnostics: Diagnostics) -> CompileError {
    match diagnostics.validate() {
        Ok(()) => CompileError::Split(diagnostics),
        Err(reason) => CompileError::InvalidDiagnostics { producer, reason },
    }
}

#[cfg(test)]
mod tests {
    use polygl_adapter_api::{FeatureTag, LanguageAdapter, LowerCtx};
    use polygl_hir::HirBuilder;
    use polygl_span::{Diagnostic, Diagnostics, Severity, SourceFile, SourceId};

    use super::{CompileError, CompileOptions, Compiler};
    use crate::AdapterRegistry;

    struct EmptyAdapter;

    impl LanguageAdapter for EmptyAdapter {
        fn id(&self) -> &'static str {
            "empty"
        }

        fn file_extensions(&self) -> &'static [&'static str] {
            &["empty"]
        }

        fn lower(
            &self,
            source: &SourceFile,
            _context: &mut LowerCtx<'_>,
        ) -> Result<polygl_hir::Module, Diagnostics> {
            let span = source.span(0, source.len()).unwrap();
            Ok(HirBuilder::new(span).module(Vec::new()))
        }

        fn capabilities(&self) -> &'static [FeatureTag] {
            &[FeatureTag::Core]
        }
    }

    static EMPTY: EmptyAdapter = EmptyAdapter;

    struct InvalidDiagnosticAdapter;

    impl LanguageAdapter for InvalidDiagnosticAdapter {
        fn id(&self) -> &'static str {
            "invalid"
        }

        fn file_extensions(&self) -> &'static [&'static str] {
            &["invalid"]
        }

        fn lower(
            &self,
            source: &SourceFile,
            _context: &mut LowerCtx<'_>,
        ) -> Result<polygl_hir::Module, Diagnostics> {
            let mut diagnostics = Diagnostics::new();
            diagnostics.push(Diagnostic::new(
                Severity::Error,
                "E0200",
                "missing the required rewrite",
                source.span(0, source.len()).unwrap(),
            ));
            Err(diagnostics)
        }

        fn capabilities(&self) -> &'static [FeatureTag] {
            &[]
        }
    }

    static INVALID_DIAGNOSTIC: InvalidDiagnosticAdapter = InvalidDiagnosticAdapter;

    #[test]
    fn compiles_in_memory_without_cli_or_filesystem_state() {
        let registry = AdapterRegistry::from_adapters([&EMPTY as &dyn LanguageAdapter]).unwrap();
        let compiler = Compiler::new(registry);
        let source = SourceFile::new(SourceId::new(7), "main.empty", "ignored");

        let output = compiler
            .compile(&source, "empty", CompileOptions::check())
            .unwrap();

        assert!(output.javascript.javascript.contains("__polyglRuntimeAbi"));
        assert!(output.javascript.source_map.is_none());
        assert!(output.shaders.shaders.is_empty());
        assert!(output.assets.is_empty());
        assert!(output.warnings.is_empty());
    }

    #[test]
    fn reports_unknown_languages_and_invalid_option_combinations() {
        let compiler = Compiler::new(AdapterRegistry::new());
        let source = SourceFile::new(SourceId::new(0), "main.unknown", "");
        assert!(matches!(
            compiler.analyze(&source, "unknown"),
            Err(CompileError::UnsupportedLanguage(language)) if language == "unknown"
        ));

        let registry = AdapterRegistry::from_adapters([&EMPTY as &dyn LanguageAdapter]).unwrap();
        let compiler = Compiler::new(registry);
        let options = CompileOptions {
            sources_content: true,
            ..CompileOptions::check()
        };
        let error = compiler.compile(&source, "empty", options).unwrap_err();
        let diagnostics = error.diagnostics().unwrap();
        let diagnostic = diagnostics.iter().next().unwrap();
        assert_eq!(diagnostic.code, polygl_span::DiagnosticCode::E0001);
        assert!(error.render(&source).contains("E0001"));
    }

    #[test]
    fn rejects_invalid_diagnostics_from_an_external_adapter() {
        let registry =
            AdapterRegistry::from_adapters([&INVALID_DIAGNOSTIC as &dyn LanguageAdapter]).unwrap();
        let compiler = Compiler::new(registry);
        let source = SourceFile::new(SourceId::new(0), "main.invalid", "dynamic");

        let error = compiler.analyze(&source, "invalid").unwrap_err();
        assert!(matches!(error, CompileError::InvalidDiagnostics { .. }));
        assert!(
            error
                .to_string()
                .contains("requires at least one suggested fix")
        );
    }

    #[test]
    fn standard_compiler_owns_the_canonical_language_registry() {
        let compiler = Compiler::standard();
        assert_eq!(
            compiler
                .adapters()
                .iter()
                .map(LanguageAdapter::id)
                .collect::<Vec<_>>(),
            ["ruby", "php", "perl"]
        );
        assert_eq!(
            compiler.adapters().for_extension(".php").unwrap().id(),
            "php"
        );
    }
}
