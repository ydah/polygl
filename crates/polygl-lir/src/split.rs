use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use polygl_builtins::{BuiltinTable, Domain as BuiltinDomain};
use polygl_span::{Diagnostic, DiagnosticCode, Diagnostics, Label, Severity, Suggestion};
use polygl_types::Type;

use crate::{
    Block, CallTarget, Constant, Domain, EntryKind, EntryPoint, Expr, ExprKind, Function, Literal,
    Module, Place, PlaceKind, StatementKind,
};
use crate::{
    dependency::{block_dependencies, expression_dependencies},
    graph::DependencyGraph,
    symbol::{SymbolKind, SymbolTable},
};

#[derive(Clone, Debug, PartialEq)]
pub struct SplitProgram {
    pub host: Module,
    pub gpu: Module,
    pub assets: Vec<AssetReference>,
    pub warnings: Diagnostics,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetReference {
    pub path: String,
    pub span: polygl_span::Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Dependency<'module> {
    Function(&'module str),
    Constant(&'module str),
}

/// Separates a resolved LIR module into Host and GPU programs and validates the
/// GLSL ES 3.00 subset at that boundary.
pub fn split(module: &Module) -> Result<SplitProgram, Diagnostics> {
    let mut validator = Validator::new(module);
    validator.validate_declaration_uniqueness();
    if validator.diagnostics.has_errors() {
        return Err(validator.diagnostics);
    }
    validator.validate_shader_pairs();
    validator.resolve_host_reachability();
    validator.validate_material_references();
    validator.validate_asset_references();
    validator.validate_host_graph();
    validator.validate_gpu_graph();
    if validator.diagnostics.has_errors() {
        return Err(validator.diagnostics);
    }

    let host = filtered_host_module(module, &validator.host_functions, &validator.host_constants);
    let gpu = filtered_gpu_module(
        module,
        &validator.gpu_functions,
        &validator.gpu_constants,
        &validator.gpu_structs,
    );
    Ok(SplitProgram {
        host,
        gpu,
        assets: validator.assets,
        warnings: validator.diagnostics,
    })
}

fn filtered_host_module(
    module: &Module,
    functions: &HashSet<&str>,
    constants: &HashSet<&str>,
) -> Module {
    Module {
        functions: module
            .functions
            .iter()
            .filter(|function| functions.contains(function.name.as_str()))
            .cloned()
            .collect(),
        structs: module.structs.clone(),
        constants: module
            .constants
            .iter()
            .filter(|constant| constants.contains(constant.name.as_str()))
            .cloned()
            .collect(),
        entries: module
            .entries
            .iter()
            .filter(|entry| entry.domain == Domain::Host)
            .cloned()
            .collect(),
        span: module.span,
    }
}

fn filtered_gpu_module(
    module: &Module,
    functions: &HashSet<&str>,
    constants: &HashSet<&str>,
    structs: &HashSet<&str>,
) -> Module {
    Module {
        functions: module
            .functions
            .iter()
            .filter(|function| functions.contains(function.name.as_str()))
            .cloned()
            .collect(),
        structs: module
            .structs
            .iter()
            .filter(|definition| structs.contains(definition.name.as_str()))
            .cloned()
            .collect(),
        constants: module
            .constants
            .iter()
            .filter(|constant| constants.contains(constant.name.as_str()))
            .cloned()
            .collect(),
        entries: module
            .entries
            .iter()
            .filter(|entry| entry.domain == Domain::Gpu)
            .cloned()
            .collect(),
        span: module.span,
    }
}

struct Validator<'module> {
    module: &'module Module,
    functions: HashMap<&'module str, &'module Function>,
    constants: HashMap<&'module str, &'module Constant>,
    diagnostics: Diagnostics,
    gpu_functions: HashSet<&'module str>,
    gpu_constants: HashSet<&'module str>,
    gpu_structs: HashSet<&'module str>,
    host_functions: HashSet<&'module str>,
    host_constants: HashSet<&'module str>,
    assets: Vec<AssetReference>,
    validating_structs: HashSet<&'module str>,
    validated_structs: HashSet<&'module str>,
    current_path: Vec<String>,
}

impl<'module> Validator<'module> {
    fn new(module: &'module Module) -> Self {
        Self {
            module,
            functions: module
                .functions
                .iter()
                .map(|function| (function.name.as_str(), function))
                .collect(),
            constants: module
                .constants
                .iter()
                .map(|constant| (constant.name.as_str(), constant))
                .collect(),
            diagnostics: Diagnostics::new(),
            gpu_functions: HashSet::new(),
            gpu_constants: HashSet::new(),
            gpu_structs: HashSet::new(),
            host_functions: HashSet::new(),
            host_constants: HashSet::new(),
            assets: Vec::new(),
            validating_structs: HashSet::new(),
            validated_structs: HashSet::new(),
            current_path: Vec::new(),
        }
    }

    fn validate_declaration_uniqueness(&mut self) {
        let mut functions = HashSet::new();
        for function in &self.module.functions {
            if !functions.insert(function.name.as_str()) {
                self.invalid_lir(
                    format!("LIR declares function `{}` more than once", function.name),
                    function.span,
                );
            }
        }
        let mut constants = HashSet::new();
        for constant in &self.module.constants {
            if !constants.insert(constant.name.as_str()) {
                self.invalid_lir(
                    format!("LIR declares constant `{}` more than once", constant.name),
                    constant.span,
                );
            }
        }
    }

    fn validate_shader_pairs(&mut self) {
        let mut pairs: BTreeMap<&str, (Option<&EntryPoint>, Option<&EntryPoint>)> = BTreeMap::new();
        for entry in &self.module.entries {
            let slot = match &entry.kind {
                EntryKind::Vertex(name) => Some(&mut pairs.entry(name).or_default().0),
                EntryKind::Fragment(name) => Some(&mut pairs.entry(name).or_default().1),
                EntryKind::Setup | EntryKind::Frame | EntryKind::OnEvent => None,
            };
            let Some(slot) = slot else { continue };
            if let Some(previous) = slot {
                self.diagnostics.push(
                    Diagnostic::new(
                        Severity::Error,
                        "E0405",
                        "shader stage is declared more than once",
                        entry.span,
                    )
                    .with_label(Label::new(previous.span, "first declaration is here"))
                    .with_suggestion(Suggestion::rewrite(
                        entry.span,
                        "keep one vertex and one fragment entry for each shader name",
                    )),
                );
            } else {
                *slot = Some(entry);
            }
        }
        for (name, (vertex, fragment)) in pairs {
            let (Some(vertex), Some(fragment)) = (vertex, fragment) else {
                let present = vertex.or(fragment).expect("shader pair contains one stage");
                let missing = if vertex.is_none() {
                    format!("vertex_{name}")
                } else {
                    format!("fragment_{name}")
                };
                self.error(
                    "E0405",
                    format!("shader pair `{name}` is missing `{missing}`"),
                    present.span,
                    "define both stages with the same case-sensitive suffix",
                );
                continue;
            };
            self.validate_pair(name, vertex, fragment);
        }
    }

    fn validate_pair(&mut self, name: &str, vertex: &EntryPoint, fragment: &EntryPoint) {
        const ATTRIBUTES: [(&str, Type); 4] = [
            ("position", Type::Vector(3)),
            ("normal", Type::Vector(3)),
            ("uv", Type::Vector(2)),
            ("color", Type::Vector(4)),
        ];
        let mut used_attributes = HashSet::new();
        for parameter in &vertex.params {
            if !used_attributes.insert(parameter.name.as_str()) {
                self.error(
                    "E0405",
                    format!(
                        "vertex attribute `{}` is requested more than once",
                        parameter.name
                    ),
                    parameter.span,
                    "keep one parameter for each standard vertex attribute",
                );
                continue;
            }
            match ATTRIBUTES
                .iter()
                .find(|(attribute, _)| *attribute == parameter.name)
            {
                Some((_, expected)) if *expected == parameter.ty => {}
                Some((_, expected)) => self.diagnostics.push(
                    Diagnostic::new(
                        Severity::Error,
                        "E0405",
                        format!(
                            "vertex attribute `{}` must have type `{expected}`, found `{}`",
                            parameter.name, parameter.ty
                        ),
                        parameter.span,
                    )
                    .with_suggestion(Suggestion::rewrite(
                        parameter.span,
                        format!("annotate `{}` as `{expected}`", parameter.name),
                    )),
                ),
                None => self.error(
                    "E0405",
                    format!("`{}` is not a standard vertex attribute", parameter.name),
                    parameter.span,
                    "use position, normal, uv, or color",
                ),
            }
        }

        match &vertex.result {
            Type::Vector(4) => {
                if !fragment.params.is_empty() {
                    self.error(
                        "E0405",
                        format!(
                            "zero-varying shader pair `{name}` requires a parameterless fragment entry"
                        ),
                        fragment.span,
                        "remove the fragment parameter or return a varying struct from the vertex entry",
                    );
                }
            }
            Type::Struct(struct_name) => {
                let definition = self
                    .module
                    .structs
                    .iter()
                    .find(|definition| definition.name == struct_name.as_str());
                match definition {
                    Some(definition)
                        if definition.fields.first().is_some_and(|field| {
                            field.name == "clip_pos" && field.ty == Type::Vector(4)
                        }) =>
                    {
                        for field in definition.fields.iter().skip(1) {
                            if !valid_varying_type(&field.ty) {
                                self.error(
                                    "E0405",
                                    format!(
                                        "varying field `{}.{}` has unsupported type `{}`",
                                        definition.name, field.name, field.ty
                                    ),
                                    field.span,
                                    "use int, float, a vector, or a matrix as a varying field",
                                );
                            }
                        }
                    }
                    Some(definition) => self.error(
                        "E0405",
                        format!(
                            "vertex varying struct `{}` must start with `clip_pos: vec4`",
                            definition.name
                        ),
                        definition.span,
                        "make clip_pos the first field and give it type vec4",
                    ),
                    None => self.error(
                        "E0405",
                        format!("vertex result references unknown struct `{struct_name}`"),
                        vertex.span,
                        "return a declared varying struct",
                    ),
                }
                if fragment.params.len() != 1 || fragment.params[0].ty != vertex.result {
                    self.error(
                        "E0405",
                        format!(
                            "fragment entry for `{name}` must take the vertex varying struct `{}`",
                            vertex.result
                        ),
                        fragment.span,
                        "use exactly one fragment parameter with the vertex result type",
                    );
                }
            }
            result => self.error(
                "E0405",
                format!("vertex entry for `{name}` cannot return `{result}`"),
                vertex.span,
                "return vec4 or a varying struct beginning with clip_pos: vec4",
            ),
        }

        if fragment.result != Type::Vector(4) {
            self.error(
                "E0405",
                format!(
                    "fragment entry for `{name}` must return `vec4`, found `{}`",
                    fragment.result
                ),
                fragment.span,
                "return a vec4 color",
            );
        }
    }

    fn resolve_host_reachability(&mut self) {
        let mut pending = self
            .module
            .entries
            .iter()
            .filter(|entry| entry.domain == Domain::Host)
            .flat_map(|entry| self.block_dependencies(&entry.body))
            .collect::<Vec<_>>();

        while let Some(dependency) = pending.pop() {
            let newly_reachable = match dependency {
                Dependency::Function(name) => {
                    self.functions
                        .get(name)
                        .is_some_and(|function| function.domain != Domain::Gpu)
                        && self.host_functions.insert(name)
                }
                Dependency::Constant(name) => {
                    self.constants
                        .get(name)
                        .is_some_and(|constant| constant.domain != Domain::Gpu)
                        && self.host_constants.insert(name)
                }
            };
            if newly_reachable {
                pending.extend(self.dependencies(dependency));
            }
        }
    }

    fn block_dependencies(&self, block: &Block) -> Vec<Dependency<'module>> {
        let dependencies = block_dependencies(block);
        self.named_dependencies(dependencies.functions, dependencies.constants)
    }

    fn validate_material_references(&mut self) {
        let material_operation = BuiltinTable::find("material_shader")
            .expect("material_shader is a canonical builtin")
            .runtime_op;
        let mut pairs: HashMap<&str, (bool, bool)> = HashMap::new();
        for entry in &self.module.entries {
            match &entry.kind {
                EntryKind::Vertex(name) => pairs.entry(name).or_default().0 = true,
                EntryKind::Fragment(name) => pairs.entry(name).or_default().1 = true,
                EntryKind::Setup | EntryKind::Frame | EntryKind::OnEvent => {}
            }
        }
        let mut available = pairs
            .into_iter()
            .filter_map(|(name, stages)| (stages == (true, true)).then_some(name))
            .collect::<Vec<_>>();
        available.sort_unstable();

        let mut references = Vec::new();
        for entry in self
            .module
            .entries
            .iter()
            .filter(|entry| entry.domain == Domain::Host)
        {
            collect_string_runtime_references_block(
                &entry.body,
                material_operation,
                &mut references,
            );
        }
        for function in self
            .module
            .functions
            .iter()
            .filter(|function| self.host_functions.contains(function.name.as_str()))
        {
            collect_string_runtime_references_block(
                &function.body,
                material_operation,
                &mut references,
            );
        }
        for constant in self
            .module
            .constants
            .iter()
            .filter(|constant| self.host_constants.contains(constant.name.as_str()))
        {
            collect_string_runtime_references_expr(
                &constant.value,
                material_operation,
                &mut references,
            );
        }

        for (name, span) in references {
            let Some(name) = name else {
                self.error(
                    "E0405",
                    "material_shader requires a string literal shader name",
                    span,
                    "pass a literal such as material_shader(\"main\")",
                );
                continue;
            };
            if !available.contains(&name.as_str()) {
                let suggestion = if available.is_empty() {
                    "define matching vertex_<name> and fragment_<name> entries".to_owned()
                } else {
                    format!(
                        "use one of the declared shader pairs: {}",
                        available.join(", ")
                    )
                };
                self.error(
                    "E0405",
                    format!("material_shader references unknown shader pair `{name}`"),
                    span,
                    suggestion,
                );
            }
        }
    }

    fn validate_asset_references(&mut self) {
        let texture_operation = BuiltinTable::find("texture_load")
            .expect("texture_load is a canonical builtin")
            .runtime_op;
        let mut references = Vec::new();
        for entry in self
            .module
            .entries
            .iter()
            .filter(|entry| entry.domain == Domain::Host)
        {
            collect_string_runtime_references_block(
                &entry.body,
                texture_operation,
                &mut references,
            );
        }
        for function in self
            .module
            .functions
            .iter()
            .filter(|function| self.host_functions.contains(function.name.as_str()))
        {
            collect_string_runtime_references_block(
                &function.body,
                texture_operation,
                &mut references,
            );
        }
        for constant in self
            .module
            .constants
            .iter()
            .filter(|constant| self.host_constants.contains(constant.name.as_str()))
        {
            collect_string_runtime_references_expr(
                &constant.value,
                texture_operation,
                &mut references,
            );
        }

        let mut seen = HashSet::new();
        for (path, span) in references {
            let Some(path) = path else {
                self.error(
                    "E0501",
                    "texture_load requires a string literal asset path",
                    span,
                    "pass a relative literal such as texture_load(\"assets/brick.png\")",
                );
                continue;
            };
            if let Some(reason) = invalid_asset_path_reason(&path) {
                self.error(
                    "E0501",
                    format!("texture asset path `{path}` is not portable: {reason}"),
                    span,
                    "use a relative slash-separated path without . or .. components",
                );
                continue;
            }
            if seen.insert(path.clone()) {
                self.assets.push(AssetReference { path, span });
            }
        }
    }

    fn validate_host_graph(&mut self) {
        let paths = self.shortest_dependency_paths(Domain::Host);
        for entry in self
            .module
            .entries
            .iter()
            .filter(|entry| entry.domain == Domain::Host)
        {
            self.current_path = vec![entry_path_name(entry)];
            self.inspect_host_block(&entry.body);
        }
        let functions = self
            .module
            .functions
            .iter()
            .filter(|function| self.host_functions.contains(function.name.as_str()))
            .map(|function| (function.name.as_str(), function.body.clone()))
            .collect::<Vec<_>>();
        for (name, body) in &functions {
            self.current_path = paths
                .get(&Dependency::Function(name))
                .cloned()
                .unwrap_or_else(|| vec![(*name).to_owned()]);
            self.inspect_host_block(body);
        }
        let constants = self
            .module
            .constants
            .iter()
            .filter(|constant| self.host_constants.contains(constant.name.as_str()))
            .map(|constant| (constant.name.as_str(), constant.value.clone()))
            .collect::<Vec<_>>();
        for (name, value) in &constants {
            self.current_path = paths
                .get(&Dependency::Constant(name))
                .cloned()
                .unwrap_or_else(|| vec![(*name).to_owned()]);
            self.inspect_host_expr(value);
        }
    }

    fn inspect_host_block(&mut self, block: &Block) {
        for statement in &block.statements {
            match &statement.kind {
                StatementKind::Let { init, .. } | StatementKind::Expr(init) => {
                    self.inspect_host_expr(init);
                }
                StatementKind::Assign { target, value } => {
                    self.inspect_host_place(target);
                    self.inspect_host_expr(value);
                }
                StatementKind::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    self.inspect_host_expr(condition);
                    self.inspect_host_block(then_block);
                    if let Some(else_block) = else_block {
                        self.inspect_host_block(else_block);
                    }
                }
                StatementKind::While { condition, body } => {
                    self.inspect_host_expr(condition);
                    self.inspect_host_block(body);
                }
                StatementKind::For { range, body, .. } => {
                    self.inspect_host_expr(&range.start);
                    self.inspect_host_expr(&range.end);
                    self.inspect_host_block(body);
                }
                StatementKind::Return(value) => {
                    if let Some(value) = value {
                        self.inspect_host_expr(value);
                    }
                }
                StatementKind::Break | StatementKind::Continue => {}
            }
        }
    }

    fn inspect_host_place(&mut self, place: &Place) {
        match &place.kind {
            PlaceKind::Variable(_) => {}
            PlaceKind::Index { base, index } => {
                self.inspect_host_expr(base);
                self.inspect_host_expr(index);
            }
            PlaceKind::Field { base, .. } => self.inspect_host_expr(base),
        }
    }

    fn inspect_host_expr(&mut self, expression: &Expr) {
        match &expression.kind {
            ExprKind::Literal(_) | ExprKind::Variable(_) => {}
            ExprKind::Uniform(name) => self.error_with_dependency_path(
                "E0404",
                format!("GPU uniform `{name}` is used by Host code"),
                expression.span,
                "keep shader uniforms inside shader entries",
                format!("uniform {name}"),
            ),
            ExprKind::Constant(name) => {
                if self
                    .constants
                    .get(name.as_str())
                    .is_some_and(|constant| constant.domain == Domain::Gpu)
                {
                    self.error_with_dependency_path(
                        "E0404",
                        format!("GPU-only constant `{name}` is used by Host code"),
                        expression.span,
                        "keep GPU-only values inside shader entries",
                        name,
                    );
                }
            }
            ExprKind::Binary { left, right, .. }
            | ExprKind::Index {
                base: left,
                index: right,
            } => {
                self.inspect_host_expr(left);
                self.inspect_host_expr(right);
            }
            ExprKind::Unary { operand, .. }
            | ExprKind::Field { base: operand, .. }
            | ExprKind::ArrayLength(operand)
            | ExprKind::IsNil(operand)
            | ExprKind::IsFalsy(operand) => self.inspect_host_expr(operand),
            ExprKind::Call { target, args } => {
                match target {
                    CallTarget::Function(name) => {
                        if self
                            .functions
                            .get(name.as_str())
                            .is_some_and(|function| function.domain == Domain::Gpu)
                        {
                            self.error_with_dependency_path(
                                "E0404",
                                format!("GPU-only function `{name}` is called by Host code"),
                                expression.span,
                                "call GPU-only helpers from shader entries",
                                name,
                            );
                        }
                    }
                    CallTarget::Runtime(operation) => {
                        let Some(builtin) = BuiltinTable::all()
                            .iter()
                            .find(|builtin| builtin.runtime_op == *operation)
                        else {
                            self.invalid_lir(
                                format!(
                                    "LIR references unregistered runtime operation `{}`",
                                    operation.as_str()
                                ),
                                expression.span,
                            );
                            for argument in args {
                                self.inspect_host_expr(argument);
                            }
                            return;
                        };
                        if builtin.domain == BuiltinDomain::Gpu {
                            self.error_with_dependency_path(
                                "E0404",
                                format!(
                                    "GPU-only builtin `{}` is called by Host code",
                                    builtin.name
                                ),
                                expression.span,
                                "call this builtin only from shader code",
                                builtin.name,
                            );
                        }
                    }
                }
                for argument in args {
                    self.inspect_host_expr(argument);
                }
            }
            ExprKind::Array(items) | ExprKind::Vector { args: items, .. } => {
                for item in items {
                    self.inspect_host_expr(item);
                }
            }
            ExprKind::Map(entries) => {
                for entry in entries {
                    self.inspect_host_expr(&entry.key);
                    self.inspect_host_expr(&entry.value);
                }
            }
            ExprKind::Struct { fields, .. } => {
                for field in fields {
                    self.inspect_host_expr(&field.value);
                }
            }
        }
    }

    fn validate_gpu_graph(&mut self) {
        let paths = self.shortest_dependency_paths(Domain::Gpu);
        for entry in self
            .module
            .entries
            .iter()
            .filter(|entry| entry.domain == Domain::Gpu)
        {
            self.current_path = vec![entry_path_name(entry)];
            self.validate_type(&entry.result, entry.span);
            for parameter in &entry.params {
                self.validate_type(&parameter.ty, parameter.span);
            }
            self.inspect_block(&entry.body);
        }

        let mut checked_functions = HashSet::new();
        let mut checked_constants = HashSet::new();
        loop {
            if let Some(name) = self
                .module
                .functions
                .iter()
                .map(|function| function.name.as_str())
                .find(|name| self.gpu_functions.contains(name) && !checked_functions.contains(name))
            {
                checked_functions.insert(name);
                let Some(function) = self.functions.get(name).copied() else {
                    continue;
                };
                self.current_path = paths
                    .get(&Dependency::Function(name))
                    .cloned()
                    .unwrap_or_else(|| vec![name.to_owned()]);
                self.validate_type(&function.result, function.span);
                for parameter in &function.params {
                    self.validate_type(&parameter.ty, parameter.span);
                }
                if function.domain == Domain::Shared && self.host_functions.contains(name) {
                    self.diagnostics.push(
                        Diagnostic::new(
                            Severity::Warning,
                            "W0401",
                            format!(
                                "shared function `{}` uses Host f64 and GPU f32 float semantics",
                                function.name
                            ),
                            function.span,
                        )
                        .with_note(
                            "keep precision-sensitive calculations in target-specific functions",
                        ),
                    );
                }
                self.inspect_block(&function.body);
                continue;
            }
            if let Some(name) = self
                .module
                .constants
                .iter()
                .map(|constant| constant.name.as_str())
                .find(|name| self.gpu_constants.contains(name) && !checked_constants.contains(name))
            {
                checked_constants.insert(name);
                let Some(constant) = self.constants.get(name).copied() else {
                    continue;
                };
                self.current_path = paths
                    .get(&Dependency::Constant(name))
                    .cloned()
                    .unwrap_or_else(|| vec![name.to_owned()]);
                self.validate_type(&constant.ty, constant.span);
                self.inspect_expr(&constant.value);
                continue;
            }
            break;
        }
        self.validate_dependency_cycles(&checked_functions, &checked_constants);
    }

    fn validate_dependency_cycles(
        &mut self,
        reachable_functions: &HashSet<&'module str>,
        reachable_constants: &HashSet<&'module str>,
    ) {
        let symbols = SymbolTable::new(self.module);
        let dependencies = self
            .module
            .functions
            .iter()
            .map(|function| Dependency::Function(function.name.as_str()))
            .chain(
                self.module
                    .constants
                    .iter()
                    .map(|constant| Dependency::Constant(constant.name.as_str())),
            )
            .collect::<Vec<_>>();
        let mut graph = DependencyGraph::new(symbols.len());
        for dependency in &dependencies {
            let symbol = dependency_symbol(&symbols, *dependency);
            graph.set_dependencies(
                symbol,
                self.dependencies(*dependency)
                    .into_iter()
                    .map(|dependency| dependency_symbol(&symbols, dependency)),
            );
        }

        for component in graph.strongly_connected_components() {
            let recursive = component.len() > 1
                || graph
                    .dependencies(component[0])
                    .binary_search(&component[0])
                    .is_ok();
            if !recursive {
                continue;
            }
            let cycle = component
                .iter()
                .map(|symbol| dependencies[symbol.index()])
                .filter(|dependency| match dependency {
                    Dependency::Function(name) => reachable_functions.contains(name),
                    Dependency::Constant(name) => reachable_constants.contains(name),
                })
                .collect::<Vec<_>>();
            let Some(first) = cycle.first().copied() else {
                continue;
            };
            let names = component
                .iter()
                .map(|symbol| dependency_name(dependencies[symbol.index()]))
                .collect::<Vec<_>>();
            let (kind, name, span) = self.dependency_description(first);
            self.diagnostics.push(
                Diagnostic::new(
                    Severity::Error,
                    "E0401",
                    format!("GPU dependency cycle reaches {kind} `{name}`"),
                    span,
                )
                .with_note(format!("dependency cycle members: {}", names.join(", ")))
                .with_suggestion(Suggestion::rewrite(
                    span,
                    "replace the function/constant cycle with an iterative or acyclic expression",
                )),
            );
        }
    }

    fn shortest_dependency_paths(
        &self,
        domain: Domain,
    ) -> HashMap<Dependency<'module>, Vec<String>> {
        let mut roots = self
            .module
            .entries
            .iter()
            .filter(|entry| entry.domain == domain)
            .map(|entry| (entry_path_name(entry), self.block_dependencies(&entry.body)))
            .collect::<Vec<_>>();
        roots.sort_by(|left, right| left.0.cmp(&right.0));

        let mut paths = HashMap::new();
        let mut pending = VecDeque::new();
        for (root, mut dependencies) in roots {
            dependencies.sort_unstable();
            dependencies.dedup();
            for dependency in dependencies {
                let candidate = vec![root.clone(), dependency_name(dependency)];
                if prefer_path(paths.get(&dependency), &candidate) {
                    paths.insert(dependency, candidate);
                    pending.push_back(dependency);
                }
            }
        }

        while let Some(dependency) = pending.pop_front() {
            let path = paths[&dependency].clone();
            let mut next = self.dependencies(dependency);
            next.sort_unstable();
            next.dedup();
            for dependency in next {
                let mut candidate = path.clone();
                candidate.push(dependency_name(dependency));
                if prefer_path(paths.get(&dependency), &candidate) {
                    paths.insert(dependency, candidate);
                    pending.push_back(dependency);
                }
            }
        }
        paths
    }

    fn dependency_description(
        &self,
        dependency: Dependency<'module>,
    ) -> (&'static str, &'module str, polygl_span::Span) {
        match dependency {
            Dependency::Function(name) => {
                let function = self.functions[name];
                ("function", name, function.span)
            }
            Dependency::Constant(name) => {
                let constant = self.constants[name];
                ("constant", name, constant.span)
            }
        }
    }

    fn dependencies(&self, dependency: Dependency<'module>) -> Vec<Dependency<'module>> {
        let (function_names, constant_names) = match dependency {
            Dependency::Function(name) => {
                let Some(function) = self.functions.get(name) else {
                    return Vec::new();
                };
                let dependencies = block_dependencies(&function.body);
                (dependencies.functions, dependencies.constants)
            }
            Dependency::Constant(name) => {
                let Some(constant) = self.constants.get(name) else {
                    return Vec::new();
                };
                let dependencies = expression_dependencies(&constant.value);
                (dependencies.functions, dependencies.constants)
            }
        };

        self.named_dependencies(function_names, constant_names)
    }

    fn named_dependencies(
        &self,
        function_names: Vec<String>,
        constant_names: Vec<String>,
    ) -> Vec<Dependency<'module>> {
        function_names
            .into_iter()
            .filter_map(|name| {
                self.functions
                    .get_key_value(name.as_str())
                    .map(|(name, _)| Dependency::Function(name))
            })
            .chain(constant_names.into_iter().filter_map(|name| {
                self.constants
                    .get_key_value(name.as_str())
                    .map(|(name, _)| Dependency::Constant(name))
            }))
            .collect()
    }

    fn inspect_block(&mut self, block: &Block) {
        for statement in &block.statements {
            match &statement.kind {
                StatementKind::Let { ty, init, .. } => {
                    self.validate_type(ty, statement.span);
                    self.inspect_expr(init);
                }
                StatementKind::Assign { target, value } => {
                    self.inspect_place(target);
                    self.inspect_expr(value);
                }
                StatementKind::Expr(expression) => self.inspect_expr(expression),
                StatementKind::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    self.inspect_expr(condition);
                    self.inspect_block(then_block);
                    if let Some(else_block) = else_block {
                        self.inspect_block(else_block);
                    }
                }
                StatementKind::While { condition, body } => {
                    self.inspect_expr(condition);
                    self.inspect_block(body);
                }
                StatementKind::For { range, body, .. } => {
                    self.inspect_expr(&range.start);
                    self.inspect_expr(&range.end);
                    if constant_trip_count(range).is_some_and(|count| count > 1024) {
                        self.diagnostics.push(
                            Diagnostic::new(
                                Severity::Warning,
                                "W0402",
                                "GPU loop has more than 1024 compiler-visible iterations",
                                range.span,
                            )
                            .with_note(
                                "dynamic loops are legal in GLSL ES 3.00 but long loops may stall a frame",
                            ),
                        );
                    }
                    self.inspect_block(body);
                }
                StatementKind::Return(value) => {
                    if let Some(value) = value {
                        self.inspect_expr(value);
                    }
                }
                StatementKind::Break | StatementKind::Continue => {}
            }
        }
    }

    fn inspect_place(&mut self, place: &Place) {
        match &place.kind {
            PlaceKind::Variable(_) => {}
            PlaceKind::Index { base, index } => {
                self.inspect_expr(base);
                self.inspect_expr(index);
            }
            PlaceKind::Field { base, .. } => self.inspect_expr(base),
        }
    }

    fn inspect_expr(&mut self, expression: &Expr) {
        self.validate_type(&expression.ty, expression.span);
        match &expression.kind {
            ExprKind::Literal(Literal::Str(_)) => self.error(
                "E0402",
                "strings are unavailable in GPU code",
                expression.span,
                "replace the string with a numeric or boolean shader value",
            ),
            ExprKind::Literal(_) | ExprKind::Variable(_) | ExprKind::Uniform(_) => {}
            ExprKind::Constant(name) => {
                let Some(constant) = self.constants.get(name.as_str()).copied() else {
                    return;
                };
                if constant.domain == Domain::Host {
                    self.error_with_dependency_path(
                        "E0404",
                        format!("Host-only constant `{name}` is used by GPU code"),
                        expression.span,
                        "move the value into a GPU-compatible constant or uniform",
                        name,
                    );
                } else {
                    self.gpu_constants.insert(constant.name.as_str());
                }
            }
            ExprKind::Binary { op, left, right } => {
                if matches!(
                    op,
                    crate::BinaryOp::IntegerDivide
                        | crate::BinaryOp::FloorRemainder
                        | crate::BinaryOp::TruncatingRemainder
                ) && expression.ty == Type::Int
                    && !self.is_provably_nonzero_integer(right)
                {
                    self.error(
                        "E0406",
                        "GPU integer divisor is not provably nonzero",
                        right.span,
                        "use a compiler-visible nonzero integer constant or move the checked operation to Host code",
                    );
                }
                self.inspect_expr(left);
                self.inspect_expr(right);
            }
            ExprKind::Index {
                base: left,
                index: right,
            } => {
                self.inspect_expr(left);
                self.inspect_expr(right);
            }
            ExprKind::Unary { operand, .. }
            | ExprKind::Field { base: operand, .. }
            | ExprKind::IsNil(operand)
            | ExprKind::IsFalsy(operand) => self.inspect_expr(operand),
            ExprKind::Call { target, args } => {
                match target {
                    CallTarget::Function(name) => {
                        let Some(function) = self.functions.get(name.as_str()).copied() else {
                            return;
                        };
                        if function.domain == Domain::Host {
                            self.error_with_dependency_path(
                                "E0404",
                                format!("Host-only function `{name}` is called by GPU code"),
                                expression.span,
                                "call a GPU-compatible helper",
                                name,
                            );
                        } else {
                            self.gpu_functions.insert(function.name.as_str());
                        }
                    }
                    CallTarget::Runtime(operation) => {
                        let Some(builtin) = BuiltinTable::all()
                            .iter()
                            .find(|builtin| builtin.runtime_op == *operation)
                        else {
                            self.invalid_lir(
                                format!(
                                    "LIR references unregistered runtime operation `{}`",
                                    operation.as_str()
                                ),
                                expression.span,
                            );
                            for argument in args {
                                self.inspect_expr(argument);
                            }
                            return;
                        };
                        if builtin.domain == BuiltinDomain::Host {
                            self.error_with_dependency_path(
                                "E0404",
                                format!(
                                    "Host-only builtin `{}` is called by GPU code",
                                    builtin.name
                                ),
                                expression.span,
                                "use a builtin whose domain is GPU or Host/GPU",
                                builtin.name,
                            );
                        }
                    }
                }
                for argument in args {
                    self.inspect_expr(argument);
                }
            }
            ExprKind::Array(items) => {
                self.error(
                    "E0403",
                    "dynamic arrays are unavailable in GPU code",
                    expression.span,
                    "use vectors, matrices, or scalar locals",
                );
                for item in items {
                    self.inspect_expr(item);
                }
            }
            ExprKind::ArrayLength(value) => {
                self.error(
                    "E0403",
                    "dynamic array length is unavailable in GPU code",
                    expression.span,
                    "keep array iteration in Host code",
                );
                self.inspect_expr(value);
            }
            ExprKind::Map(entries) => {
                self.error(
                    "E0403",
                    "maps are unavailable in GPU code",
                    expression.span,
                    "use a fixed struct with GPU-compatible fields",
                );
                for entry in entries {
                    self.inspect_expr(&entry.key);
                    self.inspect_expr(&entry.value);
                }
            }
            ExprKind::Struct { fields, .. } => {
                for field in fields {
                    self.inspect_expr(&field.value);
                }
            }
            ExprKind::Vector { args, .. } => {
                for argument in args {
                    self.inspect_expr(argument);
                }
            }
        }
    }

    fn validate_type(&mut self, ty: &Type, span: polygl_span::Span) {
        match ty {
            Type::Array(_) | Type::Map(_) => self.error(
                "E0403",
                format!("`{ty}` has dynamic storage and cannot be represented in GLSL ES 3.00"),
                span,
                "use a fixed vector, matrix, or struct",
            ),
            Type::Str | Type::Option(_) => self.error(
                "E0402",
                format!("`{ty}` cannot be represented in GPU code"),
                span,
                "use int, float, bool, vectors, matrices, or a GPU-compatible struct",
            ),
            Type::Opaque(polygl_hir::OpaqueType::Texture) => {}
            Type::Opaque(_) => self.error(
                "E0402",
                format!("opaque Host handle `{ty}` cannot be represented in GPU code"),
                span,
                "pass only Texture handles to shaders",
            ),
            Type::Struct(name) => {
                if let Some(definition) = self
                    .module
                    .structs
                    .iter()
                    .find(|definition| definition.name == name.as_str())
                {
                    let name = definition.name.as_str();
                    self.gpu_structs.insert(name);
                    if self.validated_structs.contains(name) {
                        return;
                    }
                    if !self.validating_structs.insert(name) {
                        self.error(
                            "E0402",
                            format!("recursive struct `{name}` has no finite GPU representation"),
                            definition.span,
                            "remove the direct or indirect struct cycle",
                        );
                        return;
                    }
                    let fields = definition.fields.clone();
                    for field in fields {
                        self.validate_type(&field.ty, field.span);
                    }
                    self.validating_structs.remove(name);
                    self.validated_structs.insert(name);
                }
            }
            Type::Unit
            | Type::Int
            | Type::Float
            | Type::Bool
            | Type::Vector(_)
            | Type::Matrix(_) => {}
        }
    }

    fn is_provably_nonzero_integer(&self, expression: &Expr) -> bool {
        self.constant_integer_value(expression, &mut HashSet::new())
            .is_some_and(|value| value != 0)
    }

    fn constant_integer_value(
        &self,
        expression: &Expr,
        visiting: &mut HashSet<&'module str>,
    ) -> Option<i32> {
        match &expression.kind {
            ExprKind::Literal(Literal::Int(value)) => Some(*value),
            ExprKind::Unary {
                op: crate::UnaryOp::Negate,
                operand,
            } => self
                .constant_integer_value(operand, visiting)
                .map(i32::wrapping_neg),
            ExprKind::Constant(name) => {
                let (name, constant) = self.constants.get_key_value(name.as_str())?;
                if constant.ty != Type::Int || !visiting.insert(name) {
                    return None;
                }
                let value = self.constant_integer_value(&constant.value, visiting);
                visiting.remove(name);
                value
            }
            _ => None,
        }
    }

    fn error(
        &mut self,
        code: impl Into<DiagnosticCode>,
        message: impl Into<String>,
        span: polygl_span::Span,
        suggestion: impl Into<String>,
    ) {
        let code = code.into();
        self.diagnostics.push(
            Diagnostic::new(Severity::Error, code, message, span)
                .with_suggestion(Suggestion::rewrite(span, suggestion)),
        );
    }

    fn error_with_dependency_path(
        &mut self,
        code: impl Into<DiagnosticCode>,
        message: impl Into<String>,
        span: polygl_span::Span,
        suggestion: impl Into<String>,
        target: impl AsRef<str>,
    ) {
        let code = code.into();
        let mut path = self.current_path.clone();
        path.push(target.as_ref().to_owned());
        self.diagnostics.push(
            Diagnostic::new(Severity::Error, code, message, span)
                .with_note(format!("dependency path: {}", path.join(" → ")))
                .with_suggestion(Suggestion::rewrite(span, suggestion)),
        );
    }

    fn invalid_lir(&mut self, message: impl Into<String>, span: polygl_span::Span) {
        self.diagnostics.push(
            Diagnostic::new(Severity::Error, "E0001", message, span)
                .with_note("rejecting malformed LIR at the Host/GPU split boundary"),
        );
    }
}

fn entry_path_name(entry: &EntryPoint) -> String {
    match &entry.kind {
        EntryKind::Setup => "setup".to_owned(),
        EntryKind::Frame => "frame".to_owned(),
        EntryKind::OnEvent => "on_event".to_owned(),
        EntryKind::Vertex(name) => format!("vertex_{name}"),
        EntryKind::Fragment(name) => format!("fragment_{name}"),
    }
}

fn dependency_name(dependency: Dependency<'_>) -> String {
    match dependency {
        Dependency::Function(name) | Dependency::Constant(name) => name.to_owned(),
    }
}

fn dependency_symbol(symbols: &SymbolTable<'_>, dependency: Dependency<'_>) -> crate::SymbolId {
    match dependency {
        Dependency::Function(name) => symbols.require(name, SymbolKind::Function),
        Dependency::Constant(name) => symbols.require(name, SymbolKind::Constant),
    }
}

fn prefer_path(existing: Option<&Vec<String>>, candidate: &[String]) -> bool {
    existing.is_none_or(|existing| {
        candidate.len() < existing.len()
            || (candidate.len() == existing.len() && candidate < existing.as_slice())
    })
}

fn constant_trip_count(range: &crate::Range) -> Option<u64> {
    let ExprKind::Literal(Literal::Int(start)) = range.start.kind else {
        return None;
    };
    let ExprKind::Literal(Literal::Int(end)) = range.end.kind else {
        return None;
    };
    if end < start {
        return Some(0);
    }
    let distance = i64::from(end) - i64::from(start);
    Some(u64::try_from(distance).ok()? + u64::from(range.inclusive))
}

const fn valid_varying_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Int | Type::Float | Type::Vector(_) | Type::Matrix(_)
    )
}

fn collect_string_runtime_references_block(
    block: &Block,
    operation: polygl_builtins::RuntimeOp,
    references: &mut Vec<(Option<String>, polygl_span::Span)>,
) {
    for statement in &block.statements {
        match &statement.kind {
            StatementKind::Let { init, .. } | StatementKind::Expr(init) => {
                collect_string_runtime_references_expr(init, operation, references);
            }
            StatementKind::Assign { target, value } => {
                collect_string_runtime_references_place(target, operation, references);
                collect_string_runtime_references_expr(value, operation, references);
            }
            StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                collect_string_runtime_references_expr(condition, operation, references);
                collect_string_runtime_references_block(then_block, operation, references);
                if let Some(else_block) = else_block {
                    collect_string_runtime_references_block(else_block, operation, references);
                }
            }
            StatementKind::While { condition, body } => {
                collect_string_runtime_references_expr(condition, operation, references);
                collect_string_runtime_references_block(body, operation, references);
            }
            StatementKind::For { range, body, .. } => {
                collect_string_runtime_references_expr(&range.start, operation, references);
                collect_string_runtime_references_expr(&range.end, operation, references);
                collect_string_runtime_references_block(body, operation, references);
            }
            StatementKind::Return(value) => {
                if let Some(value) = value {
                    collect_string_runtime_references_expr(value, operation, references);
                }
            }
            StatementKind::Break | StatementKind::Continue => {}
        }
    }
}

fn collect_string_runtime_references_place(
    place: &Place,
    operation: polygl_builtins::RuntimeOp,
    references: &mut Vec<(Option<String>, polygl_span::Span)>,
) {
    match &place.kind {
        PlaceKind::Variable(_) => {}
        PlaceKind::Index { base, index } => {
            collect_string_runtime_references_expr(base, operation, references);
            collect_string_runtime_references_expr(index, operation, references);
        }
        PlaceKind::Field { base, .. } => {
            collect_string_runtime_references_expr(base, operation, references);
        }
    }
}

fn collect_string_runtime_references_expr(
    expression: &Expr,
    operation: polygl_builtins::RuntimeOp,
    references: &mut Vec<(Option<String>, polygl_span::Span)>,
) {
    match &expression.kind {
        ExprKind::Call { target, args } => {
            if matches!(target, CallTarget::Runtime(candidate) if *candidate == operation) {
                references.push((
                    args.first().and_then(|argument| {
                        if let ExprKind::Literal(Literal::Str(name)) = &argument.kind {
                            Some(name.clone())
                        } else {
                            None
                        }
                    }),
                    args.first()
                        .map_or(expression.span, |argument| argument.span),
                ));
            }
            for argument in args {
                collect_string_runtime_references_expr(argument, operation, references);
            }
        }
        ExprKind::Binary { left, right, .. }
        | ExprKind::Index {
            base: left,
            index: right,
        } => {
            collect_string_runtime_references_expr(left, operation, references);
            collect_string_runtime_references_expr(right, operation, references);
        }
        ExprKind::Unary { operand, .. }
        | ExprKind::Field { base: operand, .. }
        | ExprKind::ArrayLength(operand)
        | ExprKind::IsNil(operand)
        | ExprKind::IsFalsy(operand) => {
            collect_string_runtime_references_expr(operand, operation, references);
        }
        ExprKind::Array(items) | ExprKind::Vector { args: items, .. } => {
            for item in items {
                collect_string_runtime_references_expr(item, operation, references);
            }
        }
        ExprKind::Map(entries) => {
            for entry in entries {
                collect_string_runtime_references_expr(&entry.key, operation, references);
                collect_string_runtime_references_expr(&entry.value, operation, references);
            }
        }
        ExprKind::Struct { fields, .. } => {
            for field in fields {
                collect_string_runtime_references_expr(&field.value, operation, references);
            }
        }
        ExprKind::Literal(_)
        | ExprKind::Variable(_)
        | ExprKind::Constant(_)
        | ExprKind::Uniform(_) => {}
    }
}

fn invalid_asset_path_reason(path: &str) -> Option<&'static str> {
    const GENERATED_ARTIFACTS: [&str; 6] = [
        "app.js",
        "app.js.map",
        "index.html",
        "polygl-manifest.json",
        "runtime.js",
        "shaders.js",
    ];
    if path.is_empty() {
        return Some("the path is empty");
    }
    if path.starts_with('/') {
        return Some("absolute paths are not allowed");
    }
    if path.contains('\\') {
        return Some("backslashes are not portable path separators");
    }
    if path.contains(':') {
        return Some("URL schemes and drive prefixes are not allowed");
    }
    if path.contains(['#', '?', '%']) {
        return Some("URL fragment, query, and percent-escape delimiters are not allowed");
    }
    if path
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Some("empty, . and .. path components are not allowed");
    }
    if GENERATED_ARTIFACTS.contains(&path) {
        return Some("the path would overwrite a generated build artifact");
    }
    None
}

#[cfg(test)]
mod tests {
    use polygl_builtins::BuiltinTable;
    use polygl_hir::OpaqueType;
    use polygl_span::{SourceFile, SourceId, Span};

    use super::*;
    use crate::{Field, Parameter, Statement};

    fn span() -> Span {
        SourceFile::new(SourceId::new(1), "shader.rb", "x")
            .span(0, 1)
            .unwrap()
    }

    fn expression(kind: ExprKind, ty: Type) -> Expr {
        Expr::new(kind, ty, span())
    }

    fn vector4() -> Expr {
        expression(
            ExprKind::Vector {
                size: 4,
                args: [0.0, 0.0, 0.0, 1.0]
                    .into_iter()
                    .map(|value| expression(ExprKind::Literal(Literal::Float(value)), Type::Float))
                    .collect(),
            },
            Type::Vector(4),
        )
    }

    fn return_vector4() -> Statement {
        Statement::new(StatementKind::Return(Some(vector4())), span())
    }

    fn shader_entry(kind: EntryKind, body: Vec<Statement>) -> EntryPoint {
        EntryPoint {
            kind,
            params: Vec::new(),
            result: Type::Vector(4),
            body: Block {
                statements: body,
                span: span(),
            },
            domain: Domain::Gpu,
            span: span(),
        }
    }

    fn module(entries: Vec<EntryPoint>) -> Module {
        Module {
            functions: Vec::new(),
            structs: Vec::new(),
            constants: Vec::new(),
            entries,
            span: span(),
        }
    }

    fn valid_pair(vertex_body: Vec<Statement>) -> Module {
        module(vec![
            shader_entry(EntryKind::Vertex("main".to_owned()), vertex_body),
            shader_entry(
                EntryKind::Fragment("main".to_owned()),
                vec![return_vector4()],
            ),
        ])
    }

    fn codes(diagnostics: &Diagnostics) -> HashSet<&str> {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect()
    }

    #[test]
    fn splits_a_valid_zero_varying_shader_pair() {
        let split = split(&valid_pair(vec![return_vector4()])).expect("valid shader pair");
        assert!(split.host.entries.is_empty());
        assert_eq!(split.gpu.entries.len(), 2);
        assert!(split.warnings.is_empty());
    }

    #[test]
    fn rejects_malformed_public_lir_instead_of_panicking() {
        let unknown_runtime = expression(
            ExprKind::Call {
                target: CallTarget::Runtime(polygl_builtins::RuntimeOp::new("not_registered")),
                args: Vec::new(),
            },
            Type::Unit,
        );
        let unknown = valid_pair(vec![
            Statement::new(StatementKind::Expr(unknown_runtime), span()),
            return_vector4(),
        ]);
        let diagnostics = split(&unknown).expect_err("unknown runtime operations must be rejected");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "E0001" && diagnostic.message.contains("not_registered")
        }));

        let duplicate = Function {
            name: "duplicate".to_owned(),
            params: Vec::new(),
            result: Type::Unit,
            body: Block {
                statements: Vec::new(),
                span: span(),
            },
            domain: Domain::Host,
            span: span(),
        };
        let mut duplicate_declarations = valid_pair(vec![return_vector4()]);
        duplicate_declarations.functions = vec![duplicate.clone(), duplicate];
        let diagnostics = split(&duplicate_declarations)
            .expect_err("duplicate declarations must be rejected before graph construction");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "E0001" && diagnostic.message.contains("more than once")
        }));
    }

    #[test]
    fn validates_varying_struct_abi() {
        let mut module = module(vec![
            EntryPoint {
                kind: EntryKind::Vertex("mesh".to_owned()),
                params: vec![Parameter {
                    name: "position".to_owned(),
                    ty: Type::Vector(3),
                    span: span(),
                }],
                result: Type::Struct(polygl_hir::Symbol::new("Varyings")),
                body: Block {
                    statements: Vec::new(),
                    span: span(),
                },
                domain: Domain::Gpu,
                span: span(),
            },
            EntryPoint {
                kind: EntryKind::Fragment("mesh".to_owned()),
                params: vec![Parameter {
                    name: "varyings".to_owned(),
                    ty: Type::Struct(polygl_hir::Symbol::new("Varyings")),
                    span: span(),
                }],
                result: Type::Vector(4),
                body: Block {
                    statements: vec![return_vector4()],
                    span: span(),
                },
                domain: Domain::Gpu,
                span: span(),
            },
        ]);
        module.structs.push(crate::StructDef {
            name: "Varyings".to_owned(),
            fields: vec![
                Field {
                    name: "clip_pos".to_owned(),
                    ty: Type::Vector(4),
                    span: span(),
                },
                Field {
                    name: "uv".to_owned(),
                    ty: Type::Vector(2),
                    span: span(),
                },
                Field {
                    name: "index".to_owned(),
                    ty: Type::Int,
                    span: span(),
                },
            ],
            span: span(),
        });
        split(&module).expect("fixed varying ABI should validate");

        module.structs[0].fields.push(Field {
            name: "texture".to_owned(),
            ty: Type::Opaque(OpaqueType::Texture),
            span: span(),
        });
        let diagnostics = split(&module).expect_err("textures cannot cross as varyings");
        assert!(codes(&diagnostics).contains("E0405"));
    }

    #[test]
    fn reports_gpu_subset_and_pair_diagnostics() {
        let random = BuiltinTable::find("random").expect("registered random builtin");
        let invalid_body = vec![
            Statement::new(
                StatementKind::Expr(expression(
                    ExprKind::Literal(Literal::Str("gpu".to_owned())),
                    Type::Str,
                )),
                span(),
            ),
            Statement::new(
                StatementKind::Expr(expression(
                    ExprKind::Array(vec![expression(
                        ExprKind::Literal(Literal::Int(1)),
                        Type::Int,
                    )]),
                    Type::Array(Box::new(Type::Int)),
                )),
                span(),
            ),
            Statement::new(
                StatementKind::Expr(expression(
                    ExprKind::Call {
                        target: CallTarget::Runtime(random.runtime_op),
                        args: vec![
                            expression(ExprKind::Literal(Literal::Float(0.0)), Type::Float),
                            expression(ExprKind::Literal(Literal::Float(1.0)), Type::Float),
                        ],
                    },
                    Type::Float,
                )),
                span(),
            ),
            return_vector4(),
        ];
        let diagnostics = split(&module(vec![shader_entry(
            EntryKind::Vertex("broken".to_owned()),
            invalid_body,
        )]))
        .expect_err("invalid GPU subset and incomplete pair must fail");
        let codes = codes(&diagnostics);
        assert!(codes.contains("E0402"));
        assert!(codes.contains("E0403"));
        assert!(codes.contains("E0404"));
        assert!(codes.contains("E0405"));
    }

    #[test]
    fn resolves_material_shader_literal_names_at_split_time() {
        let material = BuiltinTable::find("material_shader").unwrap();
        let setup_with = |argument: Expr| EntryPoint {
            kind: EntryKind::Setup,
            params: Vec::new(),
            result: Type::Unit,
            body: Block {
                statements: vec![Statement::new(
                    StatementKind::Expr(expression(
                        ExprKind::Call {
                            target: CallTarget::Runtime(material.runtime_op),
                            args: vec![argument],
                        },
                        Type::Opaque(OpaqueType::Material),
                    )),
                    span(),
                )],
                span: span(),
            },
            domain: Domain::Host,
            span: span(),
        };

        let mut valid = valid_pair(vec![return_vector4()]);
        valid.entries.push(setup_with(expression(
            ExprKind::Literal(Literal::Str("main".to_owned())),
            Type::Str,
        )));
        split(&valid).expect("a literal declared shader pair resolves");

        let mut missing = valid_pair(vec![return_vector4()]);
        missing.entries.push(setup_with(expression(
            ExprKind::Literal(Literal::Str("missing".to_owned())),
            Type::Str,
        )));
        let diagnostics = split(&missing).expect_err("missing shader names must fail");
        assert!(codes(&diagnostics).contains("E0405"));

        let mut dynamic = valid_pair(vec![return_vector4()]);
        dynamic.entries.push(setup_with(expression(
            ExprKind::Variable("name".to_owned()),
            Type::Str,
        )));
        let diagnostics = split(&dynamic).expect_err("dynamic shader names must fail");
        assert!(codes(&diagnostics).contains("E0405"));
    }

    #[test]
    fn collects_only_portable_literal_texture_assets() {
        let texture = BuiltinTable::find("texture_load").unwrap();
        let setup_with = |arguments: Vec<Expr>| EntryPoint {
            kind: EntryKind::Setup,
            params: Vec::new(),
            result: Type::Unit,
            body: Block {
                statements: arguments
                    .into_iter()
                    .map(|argument| {
                        Statement::new(
                            StatementKind::Expr(expression(
                                ExprKind::Call {
                                    target: CallTarget::Runtime(texture.runtime_op),
                                    args: vec![argument],
                                },
                                Type::Opaque(OpaqueType::Texture),
                            )),
                            span(),
                        )
                    })
                    .collect(),
                span: span(),
            },
            domain: Domain::Host,
            span: span(),
        };
        let string =
            |value: &str| expression(ExprKind::Literal(Literal::Str(value.to_owned())), Type::Str);

        let mut valid = valid_pair(vec![return_vector4()]);
        valid.entries.push(setup_with(vec![
            string("assets/brick.png"),
            string("assets/brick.png"),
            string("terrain/height map.png"),
            string("textures/日本語 café.png"),
        ]));
        let program = split(&valid).expect("portable literal assets should be collected");
        assert_eq!(
            program
                .assets
                .iter()
                .map(|asset| asset.path.as_str())
                .collect::<Vec<_>>(),
            [
                "assets/brick.png",
                "terrain/height map.png",
                "textures/日本語 café.png"
            ]
        );

        let mut dynamic = valid_pair(vec![return_vector4()]);
        dynamic.entries.push(setup_with(vec![expression(
            ExprKind::Variable("path".to_owned()),
            Type::Str,
        )]));
        let diagnostics = split(&dynamic).expect_err("dynamic assets cannot be packaged");
        assert!(codes(&diagnostics).contains("E0501"));

        for unsafe_path in [
            "",
            "/absolute.png",
            "../outside.png",
            "assets\\windows.png",
            "https://example.test/a.png",
            "assets/query?.png",
            "assets/fragment#.png",
            "assets/percent%20.png",
            "polygl-manifest.json",
            "runtime.js",
        ] {
            let mut invalid = valid_pair(vec![return_vector4()]);
            invalid.entries.push(setup_with(vec![string(unsafe_path)]));
            let diagnostics = split(&invalid).expect_err("unsafe asset paths must be rejected");
            assert!(codes(&diagnostics).contains("E0501"), "{unsafe_path}");
        }

        let asset_function = |name: &str, argument: Expr| Function {
            name: name.to_owned(),
            params: Vec::new(),
            result: Type::Unit,
            body: Block {
                statements: setup_with(vec![argument]).body.statements,
                span: span(),
            },
            domain: Domain::Host,
            span: span(),
        };
        let mut unreachable = valid_pair(vec![return_vector4()]);
        unreachable.functions.push(asset_function(
            "unused_asset",
            expression(ExprKind::Variable("dynamic_path".to_owned()), Type::Str),
        ));
        let program = split(&unreachable).expect("unreachable assets must not affect packaging");
        assert!(program.assets.is_empty());
        assert!(program.host.functions.is_empty());

        let mut reachable = valid_pair(vec![return_vector4()]);
        reachable
            .functions
            .push(asset_function("load_asset", string("reachable.png")));
        reachable.entries.push(EntryPoint {
            kind: EntryKind::Setup,
            params: Vec::new(),
            result: Type::Unit,
            body: Block {
                statements: vec![Statement::new(
                    StatementKind::Expr(expression(
                        ExprKind::Call {
                            target: CallTarget::Function("load_asset".to_owned()),
                            args: Vec::new(),
                        },
                        Type::Unit,
                    )),
                    span(),
                )],
                span: span(),
            },
            domain: Domain::Host,
            span: span(),
        });
        let program = split(&reachable).expect("reachable helper assets must be packaged");
        assert_eq!(
            program
                .assets
                .iter()
                .map(|asset| asset.path.as_str())
                .collect::<Vec<_>>(),
            ["reachable.png"]
        );
        assert_eq!(program.host.functions[0].name, "load_asset");
    }

    #[test]
    fn rejects_recursive_gpu_functions() {
        let function_call = expression(
            ExprKind::Call {
                target: CallTarget::Function("cycle".to_owned()),
                args: Vec::new(),
            },
            Type::Unit,
        );
        let mut module = valid_pair(vec![
            Statement::new(StatementKind::Expr(function_call.clone()), span()),
            return_vector4(),
        ]);
        module.functions.push(Function {
            name: "cycle".to_owned(),
            params: Vec::new(),
            result: Type::Unit,
            body: Block {
                statements: vec![
                    Statement::new(StatementKind::Expr(function_call), span()),
                    Statement::new(StatementKind::Return(None), span()),
                ],
                span: span(),
            },
            domain: Domain::Gpu,
            span: span(),
        });
        let diagnostics = split(&module).expect_err("GPU recursion must fail");
        assert!(codes(&diagnostics).contains("E0401"));
    }

    #[test]
    fn reports_the_shortest_dependency_path_for_domain_violations() {
        let call = |name: &str| {
            Statement::new(
                StatementKind::Expr(expression(
                    ExprKind::Call {
                        target: CallTarget::Function(name.to_owned()),
                        args: Vec::new(),
                    },
                    Type::Unit,
                )),
                span(),
            )
        };
        let function = |name: &str, domain: Domain, statements: Vec<Statement>| Function {
            name: name.to_owned(),
            params: Vec::new(),
            result: Type::Unit,
            body: Block {
                statements,
                span: span(),
            },
            domain,
            span: span(),
        };

        let mut module = valid_pair(vec![call("helper_a"), call("helper_b"), return_vector4()]);
        module.functions = vec![
            function("helper_a", Domain::Gpu, vec![call("helper_b")]),
            function("helper_b", Domain::Gpu, vec![call("host_io")]),
            function("host_io", Domain::Host, Vec::new()),
        ];

        let diagnostics = split(&module).expect_err("GPU-to-Host call must fail");
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.message.contains("host_io"))
            .unwrap();
        assert_eq!(
            diagnostic.notes,
            ["dependency path: vertex_main → helper_b → host_io"]
        );
    }

    #[test]
    fn orders_shader_pair_diagnostics_by_stable_shader_name() {
        let module = module(vec![
            shader_entry(EntryKind::Vertex("zeta".to_owned()), vec![return_vector4()]),
            shader_entry(
                EntryKind::Vertex("alpha".to_owned()),
                vec![return_vector4()],
            ),
        ]);

        let diagnostics = split(&module).expect_err("both shader pairs are incomplete");
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>(),
            [
                "shader pair `alpha` is missing `fragment_alpha`",
                "shader pair `zeta` is missing `fragment_zeta`",
            ]
        );
    }

    #[test]
    fn rejects_cyclic_gpu_constants() {
        let mut module = valid_pair(vec![
            Statement::new(
                StatementKind::Expr(expression(ExprKind::Constant("A".to_owned()), Type::Int)),
                span(),
            ),
            return_vector4(),
        ]);
        module.constants = vec![
            Constant {
                name: "A".to_owned(),
                ty: Type::Int,
                value: expression(ExprKind::Constant("B".to_owned()), Type::Int),
                domain: Domain::Gpu,
                span: span(),
            },
            Constant {
                name: "B".to_owned(),
                ty: Type::Int,
                value: expression(ExprKind::Constant("A".to_owned()), Type::Int),
                domain: Domain::Gpu,
                span: span(),
            },
        ];

        let diagnostics = split(&module).expect_err("GPU constant cycles must fail");
        assert!(codes(&diagnostics).contains("E0401"));

        let mut mixed = valid_pair(vec![
            Statement::new(
                StatementKind::Expr(expression(ExprKind::Constant("A".to_owned()), Type::Int)),
                span(),
            ),
            return_vector4(),
        ]);
        mixed.constants.push(Constant {
            name: "A".to_owned(),
            ty: Type::Int,
            value: expression(
                ExprKind::Call {
                    target: CallTarget::Function("read_a".to_owned()),
                    args: Vec::new(),
                },
                Type::Int,
            ),
            domain: Domain::Gpu,
            span: span(),
        });
        mixed.functions.push(Function {
            name: "read_a".to_owned(),
            params: Vec::new(),
            result: Type::Int,
            body: Block {
                statements: vec![Statement::new(
                    StatementKind::Return(Some(expression(
                        ExprKind::Constant("A".to_owned()),
                        Type::Int,
                    ))),
                    span(),
                )],
                span: span(),
            },
            domain: Domain::Gpu,
            span: span(),
        });

        let diagnostics =
            split(&mixed).expect_err("GPU function/constant dependency cycles must fail");
        assert!(codes(&diagnostics).contains("E0401"));
    }

    #[test]
    fn reports_shared_precision_and_long_loop_warnings() {
        let helper_call = expression(
            ExprKind::Call {
                target: CallTarget::Function("shared_helper".to_owned()),
                args: Vec::new(),
            },
            Type::Unit,
        );
        let range = crate::Range {
            start: expression(ExprKind::Literal(Literal::Int(0)), Type::Int),
            end: expression(ExprKind::Literal(Literal::Int(2048)), Type::Int),
            inclusive: false,
            span: span(),
        };
        let mut module = valid_pair(vec![
            Statement::new(StatementKind::Expr(helper_call.clone()), span()),
            Statement::new(
                StatementKind::For {
                    variable: "i".to_owned(),
                    range,
                    body: Block {
                        statements: Vec::new(),
                        span: span(),
                    },
                },
                span(),
            ),
            return_vector4(),
        ]);
        module.functions.push(Function {
            name: "shared_helper".to_owned(),
            params: Vec::new(),
            result: Type::Unit,
            body: Block {
                statements: vec![Statement::new(StatementKind::Return(None), span())],
                span: span(),
            },
            domain: Domain::Shared,
            span: span(),
        });
        module.functions.push(Function {
            name: "unused_host_shape".to_owned(),
            params: vec![Parameter {
                name: "label".to_owned(),
                ty: Type::Str,
                span: span(),
            }],
            result: Type::Unit,
            body: Block {
                statements: vec![Statement::new(StatementKind::Return(None), span())],
                span: span(),
            },
            domain: Domain::Shared,
            span: span(),
        });
        module.entries.push(EntryPoint {
            kind: EntryKind::Setup,
            params: Vec::new(),
            result: Type::Unit,
            body: Block {
                statements: vec![Statement::new(StatementKind::Expr(helper_call), span())],
                span: span(),
            },
            domain: Domain::Host,
            span: span(),
        });
        let split = split(&module).expect("warnings do not block splitting");
        let warning_codes = codes(&split.warnings);
        assert!(warning_codes.contains("W0401"));
        assert!(warning_codes.contains("W0402"));
        assert_eq!(split.host.functions.len(), 1);
        assert_eq!(split.gpu.functions.len(), 1);
        assert_eq!(split.gpu.functions[0].name, "shared_helper");
    }

    #[test]
    fn rejects_non_texture_opaque_gpu_values() {
        let mut module = valid_pair(vec![return_vector4()]);
        module.entries[0].params.push(Parameter {
            name: "position".to_owned(),
            ty: Type::Opaque(OpaqueType::Node),
            span: span(),
        });
        let diagnostics = split(&module).expect_err("Host handles cannot enter GPU code");
        assert!(codes(&diagnostics).contains("E0402"));
        assert!(codes(&diagnostics).contains("E0405"));
    }

    #[test]
    fn rejects_gpu_integer_divisors_that_can_reach_zero() {
        for divisor in [
            expression(ExprKind::Literal(Literal::Int(0)), Type::Int),
            expression(ExprKind::Variable("divisor".to_owned()), Type::Int),
        ] {
            let divide = expression(
                ExprKind::Binary {
                    op: crate::BinaryOp::IntegerDivide,
                    left: Box::new(expression(ExprKind::Literal(Literal::Int(1)), Type::Int)),
                    right: Box::new(divisor),
                },
                Type::Int,
            );
            let diagnostics = split(&valid_pair(vec![
                Statement::new(StatementKind::Expr(divide), span()),
                return_vector4(),
            ]))
            .expect_err("GPU integer division needs a statically nonzero divisor");
            assert!(codes(&diagnostics).contains("E0406"));
        }

        let safe_divide = expression(
            ExprKind::Binary {
                op: crate::BinaryOp::IntegerDivide,
                left: Box::new(expression(
                    ExprKind::Literal(Literal::Int(i32::MIN)),
                    Type::Int,
                )),
                right: Box::new(expression(ExprKind::Literal(Literal::Int(-1)), Type::Int)),
            },
            Type::Int,
        );
        split(&valid_pair(vec![
            Statement::new(StatementKind::Expr(safe_divide), span()),
            return_vector4(),
        ]))
        .expect("nonzero literal divisors satisfy the GPU arithmetic precondition");

        let mut constant_divisor = valid_pair(vec![
            Statement::new(
                StatementKind::Expr(expression(
                    ExprKind::Binary {
                        op: crate::BinaryOp::FloorRemainder,
                        left: Box::new(expression(ExprKind::Literal(Literal::Int(7)), Type::Int)),
                        right: Box::new(expression(
                            ExprKind::Constant("NONZERO".to_owned()),
                            Type::Int,
                        )),
                    },
                    Type::Int,
                )),
                span(),
            ),
            return_vector4(),
        ]);
        constant_divisor.constants.push(Constant {
            name: "NONZERO".to_owned(),
            ty: Type::Int,
            value: expression(ExprKind::Literal(Literal::Int(3)), Type::Int),
            domain: Domain::Gpu,
            span: span(),
        });
        split(&constant_divisor).expect("nonzero integer constants are propagated");
    }

    #[test]
    fn rejects_duplicate_stages_attributes_and_recursive_structs() {
        let duplicate_vertex =
            shader_entry(EntryKind::Vertex("main".to_owned()), vec![return_vector4()]);
        let mut duplicate_module = valid_pair(vec![return_vector4()]);
        duplicate_module.entries.push(duplicate_vertex);
        duplicate_module.entries[0].params = vec![
            Parameter {
                name: "uv".to_owned(),
                ty: Type::Vector(2),
                span: span(),
            },
            Parameter {
                name: "uv".to_owned(),
                ty: Type::Vector(2),
                span: span(),
            },
        ];
        let diagnostics = split(&duplicate_module).expect_err("duplicate ABI items must fail");
        assert!(codes(&diagnostics).contains("E0405"));

        let mut recursive = module(vec![
            EntryPoint {
                kind: EntryKind::Vertex("recursive".to_owned()),
                params: Vec::new(),
                result: Type::Struct(polygl_hir::Symbol::new("Recursive")),
                body: Block {
                    statements: Vec::new(),
                    span: span(),
                },
                domain: Domain::Gpu,
                span: span(),
            },
            EntryPoint {
                kind: EntryKind::Fragment("recursive".to_owned()),
                params: vec![Parameter {
                    name: "varyings".to_owned(),
                    ty: Type::Struct(polygl_hir::Symbol::new("Recursive")),
                    span: span(),
                }],
                result: Type::Vector(4),
                body: Block {
                    statements: vec![return_vector4()],
                    span: span(),
                },
                domain: Domain::Gpu,
                span: span(),
            },
        ]);
        recursive.structs.push(crate::StructDef {
            name: "Recursive".to_owned(),
            fields: vec![
                Field {
                    name: "clip_pos".to_owned(),
                    ty: Type::Vector(4),
                    span: span(),
                },
                Field {
                    name: "next".to_owned(),
                    ty: Type::Struct(polygl_hir::Symbol::new("Recursive")),
                    span: span(),
                },
            ],
            span: span(),
        });
        let diagnostics = split(&recursive).expect_err("recursive GPU structs must fail");
        assert!(codes(&diagnostics).contains("E0402"));
    }
}
