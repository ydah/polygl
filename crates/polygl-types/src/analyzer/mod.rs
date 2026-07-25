mod annotate;
mod diagnostic;
mod expr;
mod stmt;

use std::collections::{HashMap, HashSet};

use polygl_builtins::BuiltinTable;
use polygl_hir::{
    Block, EntryPoint, EntryPointKind, Function, Item, Module, Param, StmtKind, StructDef, Symbol,
    TypeExpr, TypeKind,
};
use polygl_span::{Diagnostics, Span};

use crate::solver::{InferType, Solver};
use crate::{Type, TypedModule};

const DEFAULT_INSTANCE_LIMIT: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnalyzeOptions {
    pub instance_limit: usize,
}

impl Default for AnalyzeOptions {
    fn default() -> Self {
        Self {
            instance_limit: DEFAULT_INSTANCE_LIMIT,
        }
    }
}

pub fn analyze(module: &Module) -> Result<TypedModule, Diagnostics> {
    analyze_with_options(module, AnalyzeOptions::default())
}

pub fn analyze_with_options(
    module: &Module,
    options: AnalyzeOptions,
) -> Result<TypedModule, Diagnostics> {
    Analyzer::new(module, options).analyze(module)
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct InstanceKey {
    function: String,
    arguments: Vec<Type>,
}

#[derive(Clone, Debug)]
pub(super) struct InstanceInfo {
    name: String,
    result: Type,
}

#[derive(Clone)]
struct PendingInstance {
    source_name: Symbol,
    _provisional_function: Function,
    provisional_parameters: Vec<InferType>,
    provisional_body_result: InferType,
    provisional_result: InferType,
    call_span: Span,
}

struct DeferredAdd {
    left: InferType,
    right: InferType,
    result: InferType,
    span: Span,
}

#[derive(Clone, Default)]
pub(super) struct Environment {
    scopes: Vec<HashMap<String, Binding>>,
}

#[derive(Clone)]
struct Binding {
    ty: InferType,
    mutable: bool,
}

impl Environment {
    fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop(&mut self) {
        self.scopes.pop();
    }

    fn insert(&mut self, name: &Symbol, ty: InferType) {
        self.insert_binding(name, ty, true);
    }

    fn insert_constant(&mut self, name: &Symbol, ty: InferType) {
        self.insert_binding(name, ty, false);
    }

    fn insert_binding(&mut self, name: &Symbol, ty: InferType, mutable: bool) {
        self.scopes
            .last_mut()
            .expect("type environments always have a scope")
            .insert(name.as_str().to_owned(), Binding { ty, mutable });
    }

    fn get(&self, name: &Symbol) -> Option<InferType> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name.as_str()).map(|binding| binding.ty.clone()))
    }

    fn is_mutable(&self, name: &Symbol) -> Option<bool> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name.as_str()).map(|binding| binding.mutable))
    }
}

#[derive(Clone, Default)]
pub(super) struct BodyContext {
    environment: Environment,
    returns: Vec<InferType>,
    loop_depth: usize,
}

pub(super) struct Analyzer {
    solver: Solver,
    diagnostics: Diagnostics,
    templates: HashMap<String, Function>,
    structs: HashMap<String, StructDef>,
    constant_annotations: HashMap<String, Type>,
    constant_types: HashMap<String, InferType>,
    instances: HashMap<InstanceKey, InstanceInfo>,
    pending_instances: HashMap<usize, PendingInstance>,
    instance_counts: HashMap<String, usize>,
    instance_returns: HashMap<String, Type>,
    active: HashSet<String>,
    generated: Vec<Function>,
    deferred_adds: Vec<DeferredAdd>,
    annotated_bindings: HashSet<usize>,
    binding_types: HashMap<usize, InferType>,
    expression_types: HashMap<usize, InferType>,
    record_instances: bool,
    options: AnalyzeOptions,
}

impl Analyzer {
    fn new(module: &Module, options: AnalyzeOptions) -> Self {
        let templates = module
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Function(function) => {
                    Some((function.name.as_str().to_owned(), function.clone()))
                }
                _ => None,
            })
            .collect();
        let structs = module
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Struct(definition) => {
                    Some((definition.name.as_str().to_owned(), definition.clone()))
                }
                _ => None,
            })
            .collect();
        let mut solver = Solver::default();
        let constant_annotations = module
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Const(constant) => constant.ty.as_ref().map(|annotation| {
                    (
                        constant.name.as_str().to_owned(),
                        Type::from_expr(annotation),
                    )
                }),
                _ => None,
            })
            .collect::<HashMap<_, _>>();
        let constant_types = module
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Const(constant) => Some((
                    constant.name.as_str().to_owned(),
                    constant
                        .ty
                        .as_ref()
                        .map(Self::annotated_type)
                        .unwrap_or_else(|| solver.fresh()),
                )),
                _ => None,
            })
            .collect();
        Self {
            solver,
            diagnostics: Diagnostics::new(),
            templates,
            structs,
            constant_annotations,
            constant_types,
            instances: HashMap::new(),
            pending_instances: HashMap::new(),
            instance_counts: HashMap::new(),
            instance_returns: HashMap::new(),
            active: HashSet::new(),
            generated: Vec::new(),
            deferred_adds: Vec::new(),
            annotated_bindings: HashSet::new(),
            binding_types: HashMap::new(),
            expression_types: HashMap::new(),
            record_instances: true,
            options,
        }
    }

    fn analyze(mut self, module: &Module) -> Result<TypedModule, Diagnostics> {
        if self.options.instance_limit == 0 {
            self.configuration_error(module.span);
            return Err(self.diagnostics);
        }
        self.validate_declarations(module);
        self.validate_annotations(module);

        let mut constants = Vec::new();
        let mut retained = Vec::new();
        let mut entries = Vec::new();
        for item in &module.items {
            match item {
                Item::Function(_) => {}
                Item::Entry(entry) => entries.push(entry.clone()),
                Item::Const(constant) => constants.push(constant.clone()),
                Item::Struct(definition) => {
                    self.validate_struct(definition);
                    retained.push(Item::Struct(definition.clone()));
                }
            }
        }
        for constant in &mut constants {
            self.infer_constant(constant);
        }
        let entry_contexts = entries
            .iter_mut()
            .map(|entry| self.infer_entry(entry))
            .collect::<Vec<_>>();
        self.stabilize_pending_constraints();
        if self.diagnostics.has_errors() {
            return Err(self.diagnostics);
        }
        for constant in &mut constants {
            self.annotate_constant(constant);
        }
        retained.extend(constants.into_iter().map(Item::Const));
        for (entry, context) in entries.iter_mut().zip(&entry_contexts) {
            self.annotate_entry(entry, context);
        }
        let entries = entries.into_iter().map(Item::Entry).collect::<Vec<_>>();
        if self.diagnostics.has_errors() {
            return Err(self.diagnostics);
        }

        let mut items = self
            .generated
            .into_iter()
            .map(Item::Function)
            .collect::<Vec<_>>();
        items.extend(retained);
        items.extend(entries);
        let hir = Module {
            items,
            span: module.span,
        };
        Ok(TypedModule::new(
            hir,
            self.instance_counts,
            self.instance_returns,
        ))
    }

    fn infer_entry(&mut self, entry: &mut EntryPoint) -> BodyContext {
        let parameter_types = self.entry_parameter_types(entry);
        if parameter_types.len() != entry.params.len() {
            return BodyContext::default();
        }
        let mut context = self.body_context();
        for (parameter, ty) in entry.params.iter_mut().zip(parameter_types) {
            parameter.ty = Some(ty.to_expr(parameter.span));
            let binding = self.solver.fresh();
            if let Err(error) = self
                .solver
                .assign(binding.clone(), InferType::from_type(&ty))
            {
                self.solve_error(error, parameter.span, "E0303");
            }
            self.solver.mark_fixed(&binding);
            context.environment.insert(&parameter.name, binding);
        }
        self.infer_block(&mut entry.body, &mut context, false);
        if matches!(
            entry.kind,
            EntryPointKind::Vertex(_) | EntryPointKind::Fragment(_)
        ) {
            let expected = context.returns.first().cloned().unwrap_or(InferType::Unit);
            for returned in context.returns.iter().skip(1) {
                if let Err(error) = self.solver.equal(expected.clone(), returned.clone()) {
                    self.solve_error(error, entry.span, "E0303");
                }
            }
        } else {
            for returned in &context.returns {
                if let Err(error) = self.solver.equal(InferType::Unit, returned.clone()) {
                    self.solve_error(error, entry.span, "E0303");
                }
            }
        }
        context
    }

    fn annotate_entry(&mut self, entry: &mut EntryPoint, context: &BodyContext) {
        self.annotate_block(&mut entry.body, context, false);
        self.annotate_block(&mut entry.body, context, false);
        let returns = final_return_types(&entry.body);
        if matches!(
            entry.kind,
            EntryPointKind::Vertex(_) | EntryPointKind::Fragment(_)
        ) {
            if let Some(result) = self.final_return_type(&entry.body, entry.span) {
                entry.return_type = Some(result.to_expr(entry.span));
            }
        } else {
            for returned in returns {
                if returned != Type::Unit {
                    self.solve_error(
                        crate::solver::SolveError::Mismatch {
                            expected: InferType::Unit,
                            actual: InferType::from_type(&returned),
                        },
                        entry.span,
                        "E0303",
                    );
                }
            }
            entry.return_type = None;
        }
    }

    fn infer_constant(&mut self, constant: &mut polygl_hir::ConstDef) {
        let mut context = self.body_context();
        let inferred = self.infer_expr(&mut constant.value, &mut context);
        self.reject_unit_value(&inferred, constant.value.span);
        let expected = self
            .constant_types
            .get(constant.name.as_str())
            .cloned()
            .expect("constant types are predeclared");
        if let Err(error) = self.solver.assign(expected, inferred) {
            self.solve_error(error, constant.span, "E0303");
        }
    }

    fn annotate_constant(&mut self, constant: &mut polygl_hir::ConstDef) {
        let binding = self
            .constant_types
            .get(constant.name.as_str())
            .cloned()
            .expect("constant types are predeclared");
        let context = self.body_context();
        let value_type = self.annotate_expr(&mut constant.value, &context.environment);
        self.validate_final_value(&value_type, constant.value.span);
        let refreshed =
            if let Some(annotation) = self.constant_annotations.get(constant.name.as_str()) {
                self.solver.assign(
                    InferType::from_type(annotation),
                    InferType::from_type(&value_type),
                )
            } else {
                self.solver
                    .join(binding.clone(), InferType::from_type(&value_type))
            };
        if let Err(error) = refreshed {
            self.solve_error(error, constant.span, "E0303");
        }
        if let Some(resolved) =
            self.resolve_expression_type(&binding, constant.span, Some(constant.name.as_str()))
        {
            constant.ty = Some(resolved.to_expr(constant.span));
            if !self.diagnostics.has_errors() {
                let context = self.body_context();
                self.annotate_expr(&mut constant.value, &context.environment);
            }
        }
    }

    fn body_context(&self) -> BodyContext {
        let mut context = BodyContext::default();
        context.environment.push();
        for (name, ty) in &self.constant_types {
            context
                .environment
                .insert_constant(&Symbol::new(name.clone()), ty.clone());
        }
        context
    }

    pub(super) fn defer_add(
        &mut self,
        left: InferType,
        right: InferType,
        result: InferType,
        span: Span,
    ) {
        self.deferred_adds.push(DeferredAdd {
            left,
            right,
            result,
            span,
        });
    }

    fn solve_deferred_adds(&mut self) {
        let mut remaining = std::mem::take(&mut self.deferred_adds);
        for _ in 0..=remaining.len() {
            if remaining.is_empty() {
                break;
            }
            let previous_len = remaining.len();
            let mut unresolved = Vec::new();
            for constraint in remaining {
                let left = self.solver.resolve(&constraint.left);
                let right = self.solver.resolve(&constraint.right);
                let result = self.solver.resolve(&constraint.result);
                let expected = if left == InferType::Str
                    || right == InferType::Str
                    || result == InferType::Str
                {
                    Some(InferType::Str)
                } else if left == InferType::Float
                    || right == InferType::Float
                    || result == InferType::Float
                {
                    Some(InferType::Float)
                } else if left == InferType::Int
                    || right == InferType::Int
                    || result == InferType::Int
                {
                    Some(InferType::Int)
                } else {
                    None
                };
                if let Some(expected) = expected {
                    for actual in [
                        constraint.left.clone(),
                        constraint.right.clone(),
                        constraint.result.clone(),
                    ] {
                        if let Err(error) = self.solver.assign(expected.clone(), actual) {
                            self.solve_error(error, constraint.span, "E0303");
                        }
                    }
                } else {
                    unresolved.push(constraint);
                }
            }
            if unresolved.len() == previous_len {
                remaining = unresolved;
                break;
            }
            remaining = unresolved;
        }
        self.deferred_adds = remaining;
    }

    fn unify_pending_results(&mut self) {
        let mut groups = HashMap::<InstanceKey, Vec<(InferType, Span)>>::new();
        for pending in self.pending_instances.values() {
            let arguments = pending
                .provisional_parameters
                .iter()
                .map(|parameter| self.solver.resolve_expression(parameter))
                .collect::<Result<Vec<_>, _>>();
            if let Ok(arguments) = arguments {
                groups
                    .entry(InstanceKey {
                        function: pending.source_name.as_str().to_owned(),
                        arguments,
                    })
                    .or_default()
                    .push((pending.provisional_result.clone(), pending.call_span));
            }
        }
        for calls in groups.into_values() {
            let mut calls = calls.into_iter();
            let Some((mut shared, first_span)) = calls.next() else {
                continue;
            };
            let mut members = vec![(shared.clone(), first_span)];
            for (result, span) in calls {
                members.push((result.clone(), span));
                shared = match self.solver.join(shared, result) {
                    Ok(shared) => shared,
                    Err(error) => {
                        self.solve_error(error, span, "E0303");
                        InferType::Error
                    }
                };
            }
            for (result, span) in members {
                if let Err(error) = self.solver.assign(shared.clone(), result) {
                    self.solve_error(error, span, "E0303");
                }
            }
        }
    }

    fn propagate_pending_result_constraints(&mut self) {
        let pending = self
            .pending_instances
            .values()
            .map(|pending| {
                (
                    pending.provisional_result.clone(),
                    pending.provisional_body_result.clone(),
                    pending.call_span,
                )
            })
            .collect::<Vec<_>>();
        for (result, body_result, span) in pending {
            if let Ok(expected) = self.solver.resolve_expression(&result)
                && let Err(error) = self
                    .solver
                    .assign(InferType::from_type(&expected), body_result)
            {
                self.solve_error(error, span, "E0303");
            }
        }
    }

    fn stabilize_pending_constraints(&mut self) {
        for _ in 0..=self.pending_instances.len() {
            self.propagate_pending_result_constraints();
            self.solve_deferred_adds();
            self.unify_pending_results();
        }
    }

    fn validate_struct(&mut self, definition: &StructDef) {
        for field in &definition.fields {
            if let Some(annotation) = &field.ty {
                if !Type::from_expr(annotation).is_value_type() {
                    self.unit_value_error(field.span);
                }
            } else {
                self.unresolved_error(
                    &InferType::Var(u32::MAX),
                    field.span,
                    Some(field.name.as_str()),
                );
            }
        }
        for method in &definition.methods {
            self.diagnostics.push(
                polygl_span::Diagnostic::new(
                    polygl_span::Severity::Error,
                    "E0312",
                    "struct methods require M3 type lowering",
                    method.span,
                )
                .with_suggestion(polygl_span::Suggestion::rewrite(
                    method.span,
                    "move the method to a typed free function until M3",
                )),
            );
        }
    }

    fn validate_annotations(&mut self, module: &Module) {
        for item in &module.items {
            match item {
                Item::Function(function) => {
                    for parameter in &function.params {
                        if let Some(annotation) = &parameter.ty {
                            self.validate_type_annotation(annotation, false);
                        }
                    }
                    if let Some(annotation) = &function.return_type {
                        self.validate_type_annotation(annotation, true);
                    }
                    self.validate_block_annotations(&function.body);
                }
                Item::Struct(definition) => {
                    for field in &definition.fields {
                        if let Some(annotation) = &field.ty {
                            self.validate_type_annotation(annotation, false);
                        }
                    }
                    for method in &definition.methods {
                        for parameter in &method.params {
                            if let Some(annotation) = &parameter.ty {
                                self.validate_type_annotation(annotation, false);
                            }
                        }
                        if let Some(annotation) = &method.return_type {
                            self.validate_type_annotation(annotation, true);
                        }
                        self.validate_block_annotations(&method.body);
                    }
                }
                Item::Const(constant) => {
                    if let Some(annotation) = &constant.ty {
                        self.validate_type_annotation(annotation, false);
                    }
                }
                Item::Entry(entry) => {
                    for parameter in &entry.params {
                        if let Some(annotation) = &parameter.ty {
                            self.validate_type_annotation(annotation, false);
                        }
                    }
                    if let Some(annotation) = &entry.return_type {
                        self.validate_type_annotation(annotation, true);
                    }
                    self.validate_block_annotations(&entry.body);
                }
            }
        }
    }

    fn validate_declarations(&mut self, module: &Module) {
        let mut functions = HashSet::new();
        let mut structs = HashSet::new();
        let mut constants = HashSet::new();
        let mut entries = HashSet::new();
        for item in &module.items {
            match item {
                Item::Function(function) => {
                    if !functions.insert(function.name.as_str()) {
                        self.duplicate_declaration_error(
                            "function",
                            function.name.as_str(),
                            function.span,
                        );
                    }
                    self.validate_parameter_names(&function.params);
                }
                Item::Struct(definition) => {
                    if BuiltinTable::find_struct(definition.name.as_str()).is_some() {
                        self.reserved_type_error(definition.name.as_str(), definition.span);
                    }
                    if !structs.insert(definition.name.as_str()) {
                        self.duplicate_declaration_error(
                            "struct",
                            definition.name.as_str(),
                            definition.span,
                        );
                    }
                    let mut fields = HashSet::new();
                    for field in &definition.fields {
                        if !fields.insert(field.name.as_str()) {
                            self.duplicate_declaration_error(
                                "struct field",
                                field.name.as_str(),
                                field.span,
                            );
                        }
                    }
                    let mut methods = HashSet::new();
                    for method in &definition.methods {
                        if !methods.insert(method.name.as_str()) {
                            self.duplicate_declaration_error(
                                "method",
                                method.name.as_str(),
                                method.span,
                            );
                        }
                        self.validate_parameter_names(&method.params);
                    }
                }
                Item::Const(constant) => {
                    if !constants.insert(constant.name.as_str()) {
                        self.duplicate_declaration_error(
                            "constant",
                            constant.name.as_str(),
                            constant.span,
                        );
                    }
                }
                Item::Entry(entry) => {
                    let name = entry.kind.canonical_name();
                    if !entries.insert(name.clone()) {
                        self.duplicate_declaration_error("entry point", &name, entry.span);
                    }
                    self.validate_parameter_names(&entry.params);
                }
            }
        }
    }

    fn validate_parameter_names(&mut self, parameters: &[Param]) {
        let mut names = HashSet::new();
        for parameter in parameters {
            if !names.insert(parameter.name.as_str()) {
                self.duplicate_declaration_error(
                    "parameter",
                    parameter.name.as_str(),
                    parameter.span,
                );
            }
        }
    }

    fn validate_block_annotations(&mut self, block: &Block) {
        for statement in &block.statements {
            match &statement.kind {
                StmtKind::Let { ty, .. } => {
                    if let Some(annotation) = ty {
                        self.validate_type_annotation(annotation, false);
                    }
                }
                StmtKind::If {
                    then_block,
                    else_block,
                    ..
                } => {
                    self.validate_block_annotations(then_block);
                    if let Some(else_block) = else_block {
                        self.validate_block_annotations(else_block);
                    }
                }
                StmtKind::While { body, .. } | StmtKind::For { body, .. } => {
                    self.validate_block_annotations(body);
                }
                StmtKind::Assign { .. }
                | StmtKind::Expr(_)
                | StmtKind::Return(_)
                | StmtKind::Break
                | StmtKind::Continue => {}
            }
        }
    }

    fn validate_type_annotation(&mut self, annotation: &TypeExpr, allow_unit: bool) {
        match &annotation.kind {
            TypeKind::Array(element) | TypeKind::Map(element) | TypeKind::Option(element) => {
                self.validate_type_annotation(element, false);
            }
            TypeKind::Unit if !allow_unit => self.unit_value_error(annotation.span),
            TypeKind::Struct(name)
                if !self.structs.contains_key(name.as_str())
                    && BuiltinTable::find_struct(name.as_str()).is_none() =>
            {
                self.unknown_type_error(name.as_str(), annotation.span);
            }
            TypeKind::Vector(size) if !(2..=4).contains(size) => {
                self.invalid_dimension_error("vector", *size, annotation.span);
            }
            TypeKind::Matrix(size) if !(2..=4).contains(size) => {
                self.invalid_dimension_error("matrix", *size, annotation.span);
            }
            TypeKind::Unit
            | TypeKind::Int
            | TypeKind::Float
            | TypeKind::Bool
            | TypeKind::Str
            | TypeKind::Struct(_)
            | TypeKind::Vector(_)
            | TypeKind::Matrix(_)
            | TypeKind::Opaque(_) => {}
        }
    }

    fn entry_parameter_types(&mut self, entry: &EntryPoint) -> Vec<Type> {
        let expected = match &entry.kind {
            EntryPointKind::Setup => Vec::new(),
            EntryPointKind::Frame => vec![Type::Float],
            EntryPointKind::OnEvent => vec![Type::Struct(Symbol::new("Event"))],
            EntryPointKind::Vertex(_) | EntryPointKind::Fragment(_) => {
                return self.annotated_parameter_types(&entry.params);
            }
        };
        if expected.len() != entry.params.len() {
            self.arity_error(
                &entry.kind.canonical_name(),
                expected.len(),
                entry.params.len(),
                entry.span,
            );
            Vec::new()
        } else {
            for (parameter, expected) in entry.params.iter().zip(&expected) {
                if let Some(annotation) = &parameter.ty {
                    let annotated = Type::from_expr(annotation);
                    if annotated != *expected {
                        self.solve_error(
                            crate::solver::SolveError::Mismatch {
                                expected: InferType::from_type(expected),
                                actual: InferType::from_type(&annotated),
                            },
                            parameter.span,
                            "E0303",
                        );
                    }
                }
            }
            expected
        }
    }

    fn annotated_parameter_types(&mut self, params: &[Param]) -> Vec<Type> {
        params
            .iter()
            .filter_map(|parameter| {
                parameter.ty.as_ref().map(Type::from_expr).or_else(|| {
                    self.unresolved_error(
                        &InferType::Var(u32::MAX),
                        parameter.span,
                        Some(parameter.name.as_str()),
                    );
                    None
                })
            })
            .collect()
    }

    pub(super) fn infer_user_call(
        &mut self,
        source_name: &Symbol,
        argument_types: &[InferType],
        call_span: Span,
        expression_key: usize,
    ) -> InferType {
        let Some(mut function) = self.templates.get(source_name.as_str()).cloned() else {
            self.unknown_function_error(source_name.as_str(), call_span);
            return InferType::Error;
        };
        if function.params.len() != argument_types.len() {
            self.arity_error(
                source_name.as_str(),
                function.params.len(),
                argument_types.len(),
                call_span,
            );
            return InferType::Error;
        }
        if self.active.contains(source_name.as_str()) {
            self.recursive_error(source_name.as_str(), call_span);
            return InferType::Error;
        }
        let arguments = function
            .params
            .iter()
            .zip(argument_types)
            .map(|(parameter, supplied)| {
                let Some(annotation) = &parameter.ty else {
                    return supplied.clone();
                };
                let expected = Self::annotated_type(annotation);
                match self.solver.assign(expected, supplied.clone()) {
                    Ok(normalized) => normalized,
                    Err(error) => {
                        self.solve_error(error, call_span, "E0303");
                        InferType::Error
                    }
                }
            })
            .collect::<Vec<_>>();

        let record_instance = self.record_instances;
        self.record_instances = false;
        self.active.insert(source_name.as_str().to_owned());
        let mut context = self.body_context();
        let mut parameter_bindings = Vec::new();
        for (parameter, argument) in function.params.iter().zip(&arguments) {
            let binding = self.solver.fresh();
            if let Err(error) = self.solver.assign(binding.clone(), argument.clone()) {
                self.solve_error(error, parameter.span, "E0303");
            }
            if parameter.ty.is_some() {
                self.solver.mark_fixed(&binding);
            }
            context.environment.insert(&parameter.name, binding.clone());
            parameter_bindings.push(binding);
        }
        self.infer_block(&mut function.body, &mut context, false);
        self.active.remove(source_name.as_str());
        self.record_instances = record_instance;
        let mut result = self.infer_return_type(&context, &function.body, function.span);
        if let Some(annotation) = &function.return_type {
            let expected = Self::annotated_type(annotation);
            result = match self.solver.assign(expected, result) {
                Ok(normalized) => normalized,
                Err(error) => {
                    self.solve_error(error, function.span, "E0303");
                    InferType::Error
                }
            };
        }
        let call_result = self.solver.fresh();
        if function.return_type.is_some()
            && let Err(error) = self.solver.join(call_result.clone(), result.clone())
        {
            self.solve_error(error, function.span, "E0303");
        }

        if record_instance {
            self.pending_instances.insert(
                expression_key,
                PendingInstance {
                    source_name: source_name.clone(),
                    _provisional_function: function,
                    provisional_parameters: parameter_bindings,
                    provisional_body_result: result,
                    provisional_result: call_result.clone(),
                    call_span,
                },
            );
        }
        call_result
    }

    pub(super) fn finish_instance(
        &mut self,
        expression_key: usize,
        supplied_arguments: &[Type],
    ) -> Option<InstanceInfo> {
        let pending = self
            .pending_instances
            .remove(&expression_key)
            .expect("every inferred user call has a pending instance");
        let template = self
            .templates
            .get(pending.source_name.as_str())
            .cloned()
            .expect("pending calls always have a source template");
        let supplied_arguments = template
            .params
            .iter()
            .zip(supplied_arguments)
            .map(|(parameter, supplied)| {
                let Some(annotation) = &parameter.ty else {
                    return Some(supplied.clone());
                };
                let expected = Self::annotated_type(annotation);
                let actual = InferType::from_type(supplied);
                match self.solver.assign(expected, actual) {
                    Ok(normalized) => {
                        self.resolve_expression_type(&normalized, pending.call_span, None)
                    }
                    Err(error) => {
                        self.solve_error(error, pending.call_span, "E0303");
                        None
                    }
                }
            })
            .collect::<Option<Vec<_>>>()?;
        let mut function = template;
        let mut context = self.body_context();
        let mut parameter_bindings = Vec::new();
        for (parameter, argument) in function.params.iter().zip(&supplied_arguments) {
            let binding = self.solver.fresh();
            if let Err(error) = self
                .solver
                .assign(binding.clone(), InferType::from_type(argument))
            {
                self.solve_error(error, parameter.span, "E0303");
            }
            if parameter.ty.is_some() {
                self.solver.mark_fixed(&binding);
            }
            context.environment.insert(&parameter.name, binding.clone());
            parameter_bindings.push(binding);
        }
        self.active.insert(pending.source_name.as_str().to_owned());
        self.infer_block(&mut function.body, &mut context, false);
        self.active.remove(pending.source_name.as_str());
        let source_return_type = function.return_type.clone();
        let inferred_result = self.infer_return_type(&context, &function.body, function.span);
        if let Some(annotation) = &source_return_type {
            let expected = Self::annotated_type(annotation);
            if let Err(error) = self.solver.assign(expected, inferred_result.clone()) {
                self.solve_error(error, function.span, "E0303");
            }
        }
        if let Ok(expected) = self
            .solver
            .resolve_expression(&pending.provisional_result)
            .map(|ty| InferType::from_type(&ty))
            && let Err(error) = self.solver.assign(expected, inferred_result)
        {
            self.solve_error(error, pending.call_span, "E0303");
        }
        self.stabilize_pending_constraints();

        let arguments = function
            .params
            .iter()
            .zip(&parameter_bindings)
            .map(|(parameter, binding)| self.resolve_expression_type(binding, parameter.span, None))
            .collect::<Option<Vec<_>>>()?;
        let key = InstanceKey {
            function: pending.source_name.as_str().to_owned(),
            arguments,
        };
        if let Some(instance) = self.instances.get(&key) {
            let instance = instance.clone();
            if let Ok(expected) = self.solver.resolve_expression(&pending.provisional_result)
                && let Err(error) = self.solver.assign(
                    InferType::from_type(&expected),
                    InferType::from_type(&instance.result),
                )
            {
                self.solve_error(error, pending.call_span, "E0303");
            }
            return Some(instance);
        }
        let count = self
            .instance_counts
            .get(pending.source_name.as_str())
            .copied()
            .unwrap_or(0);
        if count >= self.options.instance_limit {
            self.instance_limit_error(
                pending.source_name.as_str(),
                self.options.instance_limit,
                pending.call_span,
            );
            return None;
        }

        let instance_name = instance_name(pending.source_name.as_str(), &key.arguments);
        function.name = Symbol::new(instance_name.clone());
        for (parameter, ty) in function.params.iter_mut().zip(&key.arguments) {
            parameter.ty = Some(ty.to_expr(parameter.span));
        }
        self.annotate_block(&mut function.body, &context, false);
        self.annotate_block(&mut function.body, &context, false);
        let structural_result = self.final_return_type(&function.body, function.span)?;
        let result = if let Some(annotation) = &source_return_type {
            let expected = Self::annotated_type(annotation);
            let actual = InferType::from_type(&structural_result);
            match self.solver.assign(expected, actual) {
                Ok(normalized) => self.resolve_expression_type(&normalized, function.span, None)?,
                Err(error) => {
                    self.solve_error(error, function.span, "E0303");
                    return None;
                }
            }
        } else {
            structural_result
        };
        function.return_type = Some(result.to_expr(function.span));
        let info = InstanceInfo {
            name: instance_name.clone(),
            result: result.clone(),
        };
        self.instances.insert(key, info.clone());
        *self
            .instance_counts
            .entry(pending.source_name.as_str().to_owned())
            .or_default() += 1;
        self.instance_returns.insert(instance_name, result);
        self.generated.push(function);
        Some(info)
    }

    fn infer_return_type(
        &mut self,
        context: &BodyContext,
        body: &polygl_hir::Block,
        span: Span,
    ) -> InferType {
        let mut returns = context.returns.clone();
        if !block_always_returns(body) {
            returns.push(InferType::Unit);
        }
        let mut returned = returns.into_iter();
        let Some(mut result) = returned.next() else {
            return InferType::Unit;
        };
        for returned in returned {
            result = match self.solver.join(result, returned.clone()) {
                Ok(result) => result,
                Err(error) => {
                    self.solve_error(error, span, "E0303");
                    return InferType::Error;
                }
            };
        }
        result
    }

    fn final_return_type(&mut self, body: &polygl_hir::Block, span: Span) -> Option<Type> {
        let mut returns = final_return_types(body);
        if !block_always_returns(body) {
            returns.push(Type::Unit);
        }
        let mut returned = returns.into_iter();
        let Some(mut result) = returned.next() else {
            return Some(Type::Unit);
        };
        for returned in returned {
            result = match join_final_types(result, returned) {
                Ok(result) => result,
                Err((expected, actual)) => {
                    self.solve_error(
                        crate::solver::SolveError::Mismatch {
                            expected: InferType::from_type(&expected),
                            actual: InferType::from_type(&actual),
                        },
                        span,
                        "E0303",
                    );
                    return None;
                }
            };
        }
        Some(result)
    }

    pub(super) fn resolve_expression_type(
        &mut self,
        ty: &InferType,
        span: Span,
        name: Option<&str>,
    ) -> Option<Type> {
        match self.solver.resolve_expression(ty) {
            Ok(ty) => Some(ty),
            Err(error) => {
                self.solve_or_unresolved(error, span, name);
                None
            }
        }
    }

    pub(super) fn annotated_type(expression: &TypeExpr) -> InferType {
        InferType::from_type(&Type::from_expr(expression))
    }
}

fn block_always_returns(block: &polygl_hir::Block) -> bool {
    block
        .statements
        .iter()
        .any(|statement| match &statement.kind {
            polygl_hir::StmtKind::Return(_) => true,
            polygl_hir::StmtKind::If {
                then_block,
                else_block: Some(else_block),
                ..
            } => block_always_returns(then_block) && block_always_returns(else_block),
            polygl_hir::StmtKind::Let { .. }
            | polygl_hir::StmtKind::Assign { .. }
            | polygl_hir::StmtKind::Expr(_)
            | polygl_hir::StmtKind::If {
                else_block: None, ..
            }
            | polygl_hir::StmtKind::While { .. }
            | polygl_hir::StmtKind::For { .. }
            | polygl_hir::StmtKind::Break
            | polygl_hir::StmtKind::Continue => false,
        })
}

fn final_return_types(block: &polygl_hir::Block) -> Vec<Type> {
    let mut returns = Vec::new();
    for statement in &block.statements {
        match &statement.kind {
            polygl_hir::StmtKind::Return(value) => {
                returns.push(value.as_ref().map_or(Type::Unit, |value| {
                    value.ty.as_ref().map(Type::from_expr).unwrap_or(Type::Unit)
                }));
            }
            polygl_hir::StmtKind::If {
                then_block,
                else_block,
                ..
            } => {
                returns.extend(final_return_types(then_block));
                if let Some(else_block) = else_block {
                    returns.extend(final_return_types(else_block));
                }
            }
            polygl_hir::StmtKind::While { body, .. } | polygl_hir::StmtKind::For { body, .. } => {
                returns.extend(final_return_types(body));
            }
            polygl_hir::StmtKind::Let { .. }
            | polygl_hir::StmtKind::Assign { .. }
            | polygl_hir::StmtKind::Expr(_)
            | polygl_hir::StmtKind::Break
            | polygl_hir::StmtKind::Continue => {}
        }
    }
    returns
}

fn join_final_types(left: Type, right: Type) -> Result<Type, (Type, Type)> {
    match (left, right) {
        (Type::Int, Type::Float) | (Type::Float, Type::Int) => Ok(Type::Float),
        (left, right) if left == right => Ok(left),
        (Type::Option(inner), value) | (value, Type::Option(inner)) if *inner == value => {
            Ok(Type::Option(inner))
        }
        (expected, actual) => Err((expected, actual)),
    }
}

fn instance_name(source_name: &str, arguments: &[Type]) -> String {
    let suffix = match arguments {
        [] => "unit".to_owned(),
        [argument] => argument.mangle(),
        arguments => {
            let mut suffix = arguments.len().to_string();
            for argument in arguments {
                let mangled = argument.mangle();
                suffix.push('_');
                suffix.push_str(&mangled.len().to_string());
                suffix.push('_');
                suffix.push_str(&mangled);
            }
            suffix
        }
    };
    format!("__pgl_{}_{source_name}__{suffix}", source_name.len())
}
