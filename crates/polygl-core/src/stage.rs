use std::collections::BTreeSet;

use polygl_hir::{ModuleMetrics, module_metrics};
use polygl_lir::SplitProgram;
use polygl_types::TypedModule;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompileBudget {
    pub max_source_bytes: usize,
    pub max_syntax_nodes: usize,
    pub max_syntax_depth: usize,
    pub max_items: usize,
    pub max_functions: usize,
    pub max_shaders: usize,
    pub max_diagnostics: usize,
}

impl Default for CompileBudget {
    fn default() -> Self {
        Self::standard()
    }
}

impl CompileBudget {
    #[must_use]
    pub const fn standard() -> Self {
        Self {
            max_source_bytes: 1024 * 1024,
            max_syntax_nodes: 200_000,
            max_syntax_depth: 512,
            max_items: 4_096,
            max_functions: 2_048,
            max_shaders: 128,
            max_diagnostics: 100,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LoweredHir {
    module: polygl_hir::Module,
    metrics: ModuleMetrics,
}

impl LoweredHir {
    pub(crate) fn new(module: polygl_hir::Module) -> Self {
        let metrics = module_metrics(&module);
        Self { module, metrics }
    }

    #[must_use]
    pub const fn module(&self) -> &polygl_hir::Module {
        &self.module
    }

    #[must_use]
    pub const fn metrics(&self) -> ModuleMetrics {
        self.metrics
    }
}

#[derive(Clone, Debug)]
pub struct TypedHir {
    module: TypedModule,
    metrics: ModuleMetrics,
}

impl TypedHir {
    pub(crate) fn new(module: TypedModule) -> Self {
        let metrics = module_metrics(module.as_hir());
        Self { module, metrics }
    }

    #[must_use]
    pub const fn module(&self) -> &TypedModule {
        &self.module
    }

    #[must_use]
    pub fn into_module(self) -> TypedModule {
        self.module
    }

    #[must_use]
    pub const fn metrics(&self) -> ModuleMetrics {
        self.metrics
    }
}

#[derive(Clone, Debug)]
pub struct DomainResolvedLir {
    module: polygl_lir::Module,
}

impl DomainResolvedLir {
    pub(crate) const fn new(module: polygl_lir::Module) -> Self {
        Self { module }
    }

    #[must_use]
    pub const fn module(&self) -> &polygl_lir::Module {
        &self.module
    }
}

#[derive(Clone, Debug)]
pub struct ValidatedSplitProgram {
    program: SplitProgram,
}

impl ValidatedSplitProgram {
    pub(crate) const fn new(program: SplitProgram) -> Self {
        Self { program }
    }

    #[must_use]
    pub const fn program(&self) -> &SplitProgram {
        &self.program
    }

    pub(crate) fn into_program(self) -> SplitProgram {
        self.program
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BudgetViolation {
    pub resource: &'static str,
    pub limit: usize,
    pub actual: usize,
}

pub(crate) struct StageValidator {
    budget: CompileBudget,
}

impl StageValidator {
    pub(crate) const fn new(budget: CompileBudget) -> Self {
        Self { budget }
    }

    pub(crate) fn validate_configuration(&self) -> Result<(), BudgetViolation> {
        self.check("diagnostic count", self.budget.max_diagnostics, 1)
    }

    pub(crate) fn validate_source(&self, source_bytes: usize) -> Result<(), BudgetViolation> {
        self.check("source bytes", self.budget.max_source_bytes, source_bytes)
    }

    pub(crate) fn validate_lowered(&self, hir: &LoweredHir) -> Result<(), BudgetViolation> {
        let metrics = hir.metrics();
        self.check("HIR items", self.budget.max_items, metrics.item_count)?;
        self.check(
            "functions",
            self.budget.max_functions,
            metrics.function_count,
        )?;
        self.check("shaders", self.budget.max_shaders, metrics.shader_count)?;
        self.check(
            "syntax nodes",
            self.budget.max_syntax_nodes,
            metrics.syntax_node_count,
        )?;
        self.check(
            "syntax depth",
            self.budget.max_syntax_depth,
            metrics.max_syntax_depth,
        )
    }

    pub(crate) fn validate_typed(&self, hir: &TypedHir) -> Result<(), BudgetViolation> {
        self.check(
            "typed syntax nodes",
            self.budget.max_syntax_nodes,
            hir.metrics().syntax_node_count,
        )
    }

    pub(crate) fn validate_lir(&self, lir: &DomainResolvedLir) -> Result<(), BudgetViolation> {
        self.check(
            "LIR functions",
            self.budget.max_functions,
            lir.module().functions.len(),
        )
    }

    pub(crate) fn validate_split(
        &self,
        split: &ValidatedSplitProgram,
    ) -> Result<(), BudgetViolation> {
        let shaders = split
            .program()
            .gpu
            .entries
            .iter()
            .filter_map(|entry| match &entry.kind {
                polygl_lir::EntryKind::Vertex(name) | polygl_lir::EntryKind::Fragment(name) => {
                    Some(name.as_str())
                }
                polygl_lir::EntryKind::Setup
                | polygl_lir::EntryKind::Frame
                | polygl_lir::EntryKind::OnEvent => None,
            })
            .collect::<BTreeSet<_>>()
            .len();
        self.check("split shaders", self.budget.max_shaders, shaders)
    }

    const fn check(
        &self,
        resource: &'static str,
        limit: usize,
        actual: usize,
    ) -> Result<(), BudgetViolation> {
        if actual > limit {
            Err(BudgetViolation {
                resource,
                limit,
                actual,
            })
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompileStatistics {
    pub hir: ModuleMetrics,
    pub host_functions: usize,
    pub gpu_functions: usize,
    pub host_constants: usize,
    pub gpu_constants: usize,
    pub shaders: usize,
    pub assets: usize,
}

impl CompileStatistics {
    pub(crate) fn from_split(hir: ModuleMetrics, split: &SplitProgram) -> Self {
        Self {
            hir,
            host_functions: split.host.functions.len(),
            gpu_functions: split.gpu.functions.len(),
            host_constants: split.host.constants.len(),
            gpu_constants: split.gpu.constants.len(),
            shaders: split
                .gpu
                .entries
                .iter()
                .filter(|entry| matches!(entry.kind, polygl_lir::EntryKind::Vertex(_)))
                .count(),
            assets: split.assets.len(),
        }
    }
}
