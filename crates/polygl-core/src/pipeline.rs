use std::error::Error;
use std::fmt;

use crate::{
    AdapterRegistry, BuiltinTable, CompileBudget, CompileStage, CompileStatistics,
    DomainResolvedLir, LoweredHir, PassTrace, TypedHir, ValidatedSplitProgram,
};
use crate::{pass::PassManager, stage::StageValidator};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompileOptions {
    pub mode: BuildMode,
    pub source_map: SourceMapMode,
    pub sources_content: bool,
    pub budget: CompileBudget,
}

impl CompileOptions {
    #[must_use]
    pub const fn check() -> Self {
        Self {
            mode: BuildMode::Debug,
            source_map: SourceMapMode::None,
            sources_content: false,
            budget: CompileBudget::standard(),
        }
    }
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            mode: BuildMode::Debug,
            source_map: SourceMapMode::External,
            sources_content: false,
            budget: CompileBudget::standard(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FrontendOutput {
    pub typed: TypedHir,
    pub trace: Vec<PassTrace>,
}

#[derive(Clone, Debug)]
pub struct CompileOutput {
    pub typed: TypedHir,
    pub javascript: Artifacts,
    pub shaders: GlslArtifacts,
    pub assets: Vec<AssetReference>,
    pub warnings: Diagnostics,
    pub trace: Vec<PassTrace>,
    pub statistics: CompileStatistics,
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
        self.analyze_with_budget(source, language_id, CompileBudget::standard())
    }

    pub fn analyze_with_budget(
        &self,
        source: &SourceFile,
        language_id: &str,
        budget: CompileBudget,
    ) -> Result<FrontendOutput, CompileError> {
        let adapter = self
            .adapters
            .by_id(language_id)
            .ok_or_else(|| CompileError::UnsupportedLanguage(language_id.to_owned()))?;
        self.analyze_with_adapter_and_budget(source, adapter, budget)
    }

    pub fn analyze_with_adapter(
        &self,
        source: &SourceFile,
        adapter: &dyn LanguageAdapter,
    ) -> Result<FrontendOutput, CompileError> {
        self.analyze_with_adapter_and_budget(source, adapter, CompileBudget::standard())
    }

    pub fn analyze_with_adapter_and_budget(
        &self,
        source: &SourceFile,
        adapter: &dyn LanguageAdapter,
        budget: CompileBudget,
    ) -> Result<FrontendOutput, CompileError> {
        let validator = StageValidator::new(budget);
        validate_budget_configuration(source, &validator)?;
        validate_source_budget(source, &validator)?;
        let mut passes = PassManager::default();
        let typed = self.analyze_managed(source, adapter, budget, &validator, &mut passes)?;
        Ok(FrontendOutput {
            typed,
            trace: passes.finish(),
        })
    }

    fn analyze_managed(
        &self,
        source: &SourceFile,
        adapter: &dyn LanguageAdapter,
        budget: CompileBudget,
        validator: &StageValidator,
        passes: &mut PassManager,
    ) -> Result<TypedHir, CompileError> {
        let mut context = LowerCtx::new(&BuiltinTable);
        let hir = passes.run(
            "adapter.lower",
            CompileStage::Source,
            CompileStage::LoweredHir,
            true,
            || {
                adapter
                    .lower(source, &mut context)
                    .map(LoweredHir::new)
                    .map_err(|diagnostics| {
                        validated_diagnostics("adapter", source, diagnostics, budget)
                    })
            },
        )?;
        validator
            .validate_lowered(&hir)
            .map_err(|violation| budget_error(source, violation))?;

        let typed = passes.run(
            "types.analyze",
            CompileStage::LoweredHir,
            CompileStage::TypedHir,
            true,
            || {
                polygl_types::analyze(hir.module())
                    .map(TypedHir::new)
                    .map_err(|diagnostics| {
                        validated_diagnostics("type analyzer", source, diagnostics, budget)
                    })
            },
        )?;
        validator
            .validate_typed(&typed)
            .map_err(|violation| budget_error(source, violation))?;
        Ok(typed)
    }

    pub fn compile(
        &self,
        source: &SourceFile,
        language_id: &str,
        options: CompileOptions,
    ) -> Result<CompileOutput, CompileError> {
        validate_options(source, options)?;
        let adapter = self
            .adapters
            .by_id(language_id)
            .ok_or_else(|| CompileError::UnsupportedLanguage(language_id.to_owned()))?;
        let validator = StageValidator::new(options.budget);
        validate_budget_configuration(source, &validator)?;
        validate_source_budget(source, &validator)?;
        let mut passes = PassManager::default();
        let typed =
            self.analyze_managed(source, adapter, options.budget, &validator, &mut passes)?;
        self.compile_typed(source, typed, options, &validator, passes)
    }

    fn compile_typed(
        &self,
        source: &SourceFile,
        typed: TypedHir,
        options: CompileOptions,
        validator: &StageValidator,
        mut passes: PassManager,
    ) -> Result<CompileOutput, CompileError> {
        let lir = passes.run(
            "lir.lower",
            CompileStage::TypedHir,
            CompileStage::DomainResolvedLir,
            true,
            || Ok::<_, CompileError>(DomainResolvedLir::new(polygl_lir::lower(typed.module()))),
        )?;
        validator
            .validate_lir(&lir)
            .map_err(|violation| budget_error(source, violation))?;

        let split = passes.run(
            "lir.split",
            CompileStage::DomainResolvedLir,
            CompileStage::SplitProgram,
            true,
            || {
                polygl_lir::split(lir.module())
                    .map(ValidatedSplitProgram::new)
                    .map_err(|diagnostics| {
                        validated_split_diagnostics(
                            "LIR split",
                            source,
                            diagnostics,
                            options.budget,
                        )
                    })
            },
        )?;
        validator
            .validate_split(&split)
            .map_err(|violation| budget_error(source, violation))?;
        split
            .program()
            .warnings
            .validate()
            .map_err(|reason| CompileError::InvalidDiagnostics {
                producer: "LIR split",
                reason,
            })?;
        validate_diagnostic_budget(source, &split.program().warnings, options.budget)?;
        let javascript = passes.run(
            "backend.javascript",
            CompileStage::SplitProgram,
            CompileStage::JavaScript,
            true,
            || {
                JavaScriptBackend::new(options.mode)
                    .with_source_map_mode(options.source_map)
                    .with_sources_content(options.sources_content)
                    .generate(&split.program().host, std::slice::from_ref(source))
                    .map_err(CompileError::JavaScript)
            },
        )?;
        let shaders = passes.run(
            "backend.glsl",
            CompileStage::SplitProgram,
            CompileStage::Glsl,
            true,
            || {
                GlslBackend::new()
                    .generate(&split.program().gpu)
                    .map_err(CompileError::Glsl)
            },
        )?;
        let statistics = CompileStatistics::from_split(typed.metrics(), split.program());
        let split = split.into_program();
        Ok(CompileOutput {
            typed,
            javascript,
            shaders,
            assets: split.assets,
            warnings: split.warnings,
            trace: passes.finish(),
            statistics,
        })
    }
}

fn validate_budget_configuration(
    source: &SourceFile,
    validator: &StageValidator,
) -> Result<(), CompileError> {
    validator
        .validate_configuration()
        .map_err(|violation| budget_error(source, violation))
}

fn validate_source_budget(
    source: &SourceFile,
    validator: &StageValidator,
) -> Result<(), CompileError> {
    validator
        .validate_source(source.len())
        .map_err(|violation| budget_error(source, violation))
}

fn validate_diagnostic_budget(
    source: &SourceFile,
    diagnostics: &Diagnostics,
    budget: CompileBudget,
) -> Result<(), CompileError> {
    let actual = diagnostics.iter().len();
    if actual > budget.max_diagnostics {
        Err(budget_error(
            source,
            crate::stage::BudgetViolation {
                resource: "diagnostic count",
                limit: budget.max_diagnostics,
                actual,
            },
        ))
    } else {
        Ok(())
    }
}

fn budget_error(source: &SourceFile, violation: crate::stage::BudgetViolation) -> CompileError {
    let span = source
        .span(0, 0)
        .expect("the start of every source is a valid empty span");
    let mut diagnostics = Diagnostics::new();
    diagnostics.push(
        Diagnostic::new(
            Severity::Error,
            DiagnosticCode::E0001,
            format!(
                "compiler budget for {} was exceeded (limit {}, actual {})",
                violation.resource, violation.limit, violation.actual
            ),
            span,
        )
        .with_note("reduce the input size or raise the corresponding CompileBudget limit"),
    );
    CompileError::Configuration(diagnostics)
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

fn validated_diagnostics(
    producer: &'static str,
    source: &SourceFile,
    diagnostics: Diagnostics,
    budget: CompileBudget,
) -> CompileError {
    match diagnostics.validate() {
        Ok(()) => match validate_diagnostic_budget(source, &diagnostics, budget) {
            Ok(()) => CompileError::Frontend(diagnostics),
            Err(error) => error,
        },
        Err(reason) => CompileError::InvalidDiagnostics { producer, reason },
    }
}

fn validated_split_diagnostics(
    producer: &'static str,
    source: &SourceFile,
    diagnostics: Diagnostics,
    budget: CompileBudget,
) -> CompileError {
    match diagnostics.validate() {
        Ok(()) => match validate_diagnostic_budget(source, &diagnostics, budget) {
            Ok(()) => CompileError::Split(diagnostics),
            Err(error) => error,
        },
        Err(reason) => CompileError::InvalidDiagnostics { producer, reason },
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use polygl_adapter_api::{FeatureTag, LanguageAdapter, LowerCtx};
    use polygl_hir::{HirBuilder, Item, UnOp};
    use polygl_span::{Diagnostic, Diagnostics, Severity, SourceFile, SourceId};

    use super::{CompileError, CompileOptions, Compiler};
    use crate::{AdapterRegistry, CompileBudget, CompileStage};

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

    static COUNTING_LOWER_CALLS: AtomicUsize = AtomicUsize::new(0);

    struct CountingAdapter;

    impl LanguageAdapter for CountingAdapter {
        fn id(&self) -> &'static str {
            "counting"
        }

        fn file_extensions(&self) -> &'static [&'static str] {
            &["counting"]
        }

        fn lower(
            &self,
            source: &SourceFile,
            _context: &mut LowerCtx<'_>,
        ) -> Result<polygl_hir::Module, Diagnostics> {
            COUNTING_LOWER_CALLS.fetch_add(1, Ordering::Relaxed);
            let span = source.span(0, source.len()).unwrap();
            Ok(HirBuilder::new(span).module(Vec::new()))
        }

        fn capabilities(&self) -> &'static [FeatureTag] {
            &[FeatureTag::Core]
        }
    }

    static COUNTING: CountingAdapter = CountingAdapter;

    struct DeepAdapter;

    impl LanguageAdapter for DeepAdapter {
        fn id(&self) -> &'static str {
            "deep"
        }

        fn file_extensions(&self) -> &'static [&'static str] {
            &["deep"]
        }

        fn lower(
            &self,
            source: &SourceFile,
            _context: &mut LowerCtx<'_>,
        ) -> Result<polygl_hir::Module, Diagnostics> {
            let span = source.span(0, source.len()).unwrap();
            let builder = HirBuilder::new(span);
            let mut value = builder.int(1);
            for _ in 0..32 {
                value = builder.unary(UnOp::Neg, value);
            }
            Ok(builder.module(vec![Item::Const(polygl_hir::ConstDef {
                name: "DEEP".into(),
                ty: None,
                value,
                span,
            })]))
        }

        fn capabilities(&self) -> &'static [FeatureTag] {
            &[FeatureTag::Core]
        }
    }

    static DEEP: DeepAdapter = DeepAdapter;

    struct NoisyAdapter;

    impl LanguageAdapter for NoisyAdapter {
        fn id(&self) -> &'static str {
            "noisy"
        }

        fn file_extensions(&self) -> &'static [&'static str] {
            &["noisy"]
        }

        fn lower(
            &self,
            source: &SourceFile,
            _context: &mut LowerCtx<'_>,
        ) -> Result<polygl_hir::Module, Diagnostics> {
            let mut diagnostics = Diagnostics::new();
            for index in 0..3 {
                diagnostics.push(Diagnostic::new(
                    Severity::Warning,
                    "W0401",
                    format!("failure {index}"),
                    source.span(0, 0).unwrap(),
                ));
            }
            Err(diagnostics)
        }

        fn capabilities(&self) -> &'static [FeatureTag] {
            &[FeatureTag::Core]
        }
    }

    static NOISY: NoisyAdapter = NoisyAdapter;

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
        assert_eq!(
            output
                .trace
                .iter()
                .map(|pass| (pass.name, pass.input, pass.output))
                .collect::<Vec<_>>(),
            [
                (
                    "adapter.lower",
                    CompileStage::Source,
                    CompileStage::LoweredHir
                ),
                (
                    "types.analyze",
                    CompileStage::LoweredHir,
                    CompileStage::TypedHir
                ),
                (
                    "lir.lower",
                    CompileStage::TypedHir,
                    CompileStage::DomainResolvedLir
                ),
                (
                    "lir.split",
                    CompileStage::DomainResolvedLir,
                    CompileStage::SplitProgram
                ),
                (
                    "backend.javascript",
                    CompileStage::SplitProgram,
                    CompileStage::JavaScript
                ),
                (
                    "backend.glsl",
                    CompileStage::SplitProgram,
                    CompileStage::Glsl
                ),
            ]
        );
        assert_eq!(output.statistics.hir.item_count, 0);
        assert_eq!(output.statistics.host_functions, 0);
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
    fn rejects_source_before_calling_an_adapter_when_source_budget_is_exceeded() {
        COUNTING_LOWER_CALLS.store(0, Ordering::Relaxed);
        let registry = AdapterRegistry::from_adapters([&COUNTING as &dyn LanguageAdapter]).unwrap();
        let compiler = Compiler::new(registry);
        let source = SourceFile::new(SourceId::new(0), "main.deep", "too large");
        let budget = CompileBudget {
            max_source_bytes: 2,
            ..CompileBudget::standard()
        };

        let error = compiler
            .analyze_with_budget(&source, "counting", budget)
            .unwrap_err();
        assert!(matches!(error, CompileError::Configuration(_)));
        assert_eq!(COUNTING_LOWER_CALLS.load(Ordering::Relaxed), 0);
        assert!(error.render(&source).contains("source bytes"));
    }

    #[test]
    fn rejects_deep_lowered_hir_before_recursive_type_analysis() {
        let registry = AdapterRegistry::from_adapters([&DEEP as &dyn LanguageAdapter]).unwrap();
        let compiler = Compiler::new(registry);
        let source = SourceFile::new(SourceId::new(0), "main.deep", "x");
        let budget = CompileBudget {
            max_syntax_depth: 8,
            ..CompileBudget::standard()
        };

        let error = compiler
            .analyze_with_budget(&source, "deep", budget)
            .unwrap_err();
        assert!(matches!(error, CompileError::Configuration(_)));
        assert!(error.render(&source).contains("syntax depth"));
    }

    #[test]
    fn rejects_diagnostic_floods_at_the_trust_boundary() {
        let registry = AdapterRegistry::from_adapters([&NOISY as &dyn LanguageAdapter]).unwrap();
        let compiler = Compiler::new(registry);
        let source = SourceFile::new(SourceId::new(0), "main.noisy", "x");
        let budget = CompileBudget {
            max_diagnostics: 2,
            ..CompileBudget::standard()
        };

        let error = compiler
            .analyze_with_budget(&source, "noisy", budget)
            .unwrap_err();
        assert!(matches!(error, CompileError::Configuration(_)));
        assert!(error.render(&source).contains("diagnostic count"));
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
