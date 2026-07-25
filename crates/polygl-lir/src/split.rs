use std::collections::{HashMap, HashSet};

use polygl_builtins::{BuiltinTable, Domain as BuiltinDomain};
use polygl_span::{Diagnostic, Diagnostics, Label, Severity, Suggestion};
use polygl_types::Type;

use crate::{
    Block, CallTarget, Constant, Domain, EntryKind, EntryPoint, Expr, ExprKind, Function, Literal,
    Module, Place, PlaceKind, StatementKind,
};

#[derive(Clone, Debug, PartialEq)]
pub struct SplitProgram {
    pub host: Module,
    pub gpu: Module,
    pub warnings: Diagnostics,
}

/// Separates a resolved LIR module into Host and GPU programs and validates the
/// GLSL ES 3.00 subset at that boundary.
pub fn split(module: &Module) -> Result<SplitProgram, Diagnostics> {
    let mut validator = Validator::new(module);
    validator.validate_shader_pairs();
    validator.validate_gpu_graph();
    if validator.diagnostics.has_errors() {
        return Err(validator.diagnostics);
    }

    let host = filtered_host_module(module);
    let gpu = filtered_gpu_module(
        module,
        &validator.gpu_functions,
        &validator.gpu_constants,
        &validator.gpu_structs,
    );
    Ok(SplitProgram {
        host,
        gpu,
        warnings: validator.diagnostics,
    })
}

fn filtered_host_module(module: &Module) -> Module {
    Module {
        functions: module
            .functions
            .iter()
            .filter(|function| function.domain != Domain::Gpu)
            .cloned()
            .collect(),
        structs: module.structs.clone(),
        constants: module
            .constants
            .iter()
            .filter(|constant| constant.domain != Domain::Gpu)
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
    validating_structs: HashSet<&'module str>,
    validated_structs: HashSet<&'module str>,
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
            validating_structs: HashSet::new(),
            validated_structs: HashSet::new(),
        }
    }

    fn validate_shader_pairs(&mut self) {
        let mut pairs: HashMap<&str, (Option<&EntryPoint>, Option<&EntryPoint>)> = HashMap::new();
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
                        }) => {}
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

    fn validate_gpu_graph(&mut self) {
        for entry in self
            .module
            .entries
            .iter()
            .filter(|entry| entry.domain == Domain::Gpu)
        {
            self.validate_type(&entry.result, entry.span);
            for parameter in &entry.params {
                self.validate_type(&parameter.ty, parameter.span);
            }
            self.inspect_block(&entry.body);
        }

        let mut pending_functions = self.gpu_functions.iter().copied().collect::<Vec<_>>();
        let mut pending_constants = self.gpu_constants.iter().copied().collect::<Vec<_>>();
        let mut checked_functions = HashSet::new();
        let mut checked_constants = HashSet::new();
        while !pending_functions.is_empty() || !pending_constants.is_empty() {
            while let Some(name) = pending_functions.pop() {
                if !checked_functions.insert(name) {
                    continue;
                }
                let Some(function) = self.functions.get(name).copied() else {
                    continue;
                };
                self.validate_type(&function.result, function.span);
                for parameter in &function.params {
                    self.validate_type(&parameter.ty, parameter.span);
                }
                if function.domain == Domain::Shared {
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
                pending_functions.extend(
                    self.gpu_functions
                        .iter()
                        .copied()
                        .filter(|dependency| !checked_functions.contains(dependency)),
                );
                pending_constants.extend(
                    self.gpu_constants
                        .iter()
                        .copied()
                        .filter(|dependency| !checked_constants.contains(dependency)),
                );
            }
            while let Some(name) = pending_constants.pop() {
                if !checked_constants.insert(name) {
                    continue;
                }
                let Some(constant) = self.constants.get(name).copied() else {
                    continue;
                };
                self.validate_type(&constant.ty, constant.span);
                self.inspect_expr(&constant.value);
                pending_functions.extend(
                    self.gpu_functions
                        .iter()
                        .copied()
                        .filter(|dependency| !checked_functions.contains(dependency)),
                );
                pending_constants.extend(
                    self.gpu_constants
                        .iter()
                        .copied()
                        .filter(|dependency| !checked_constants.contains(dependency)),
                );
            }
        }
        self.validate_recursion(&checked_functions);
    }

    fn validate_recursion(&mut self, reachable: &HashSet<&'module str>) {
        let mut visiting = HashSet::new();
        let mut visited = HashSet::new();
        for name in reachable {
            self.visit_function(name, reachable, &mut visiting, &mut visited);
        }
    }

    fn visit_function(
        &mut self,
        name: &'module str,
        reachable: &HashSet<&'module str>,
        visiting: &mut HashSet<&'module str>,
        visited: &mut HashSet<&'module str>,
    ) {
        if visited.contains(name) {
            return;
        }
        if !visiting.insert(name) {
            if let Some(function) = self.functions.get(name) {
                self.error(
                    "E0401",
                    format!("GPU function `{name}` is recursive"),
                    function.span,
                    "rewrite direct or indirect recursion as a loop",
                );
            }
            return;
        }
        let dependencies = self
            .functions
            .get(name)
            .map(|function| function_calls(&function.body))
            .unwrap_or_default();
        for dependency in dependencies {
            if reachable.contains(dependency.as_str()) {
                let dependency = self
                    .functions
                    .get_key_value(dependency.as_str())
                    .map(|(key, _)| *key)
                    .expect("reachable dependency is declared");
                self.visit_function(dependency, reachable, visiting, visited);
            }
        }
        visiting.remove(name);
        visited.insert(name);
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
            ExprKind::Literal(_) | ExprKind::Variable(_) => {}
            ExprKind::Constant(name) => {
                let Some(constant) = self.constants.get(name.as_str()).copied() else {
                    return;
                };
                if constant.domain == Domain::Host {
                    self.error(
                        "E0404",
                        format!("Host-only constant `{name}` is used by GPU code"),
                        expression.span,
                        "move the value into a GPU-compatible constant or uniform",
                    );
                } else {
                    self.gpu_constants.insert(constant.name.as_str());
                }
            }
            ExprKind::Binary { left, right, .. }
            | ExprKind::Index {
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
                            self.error(
                                "E0404",
                                format!("Host-only function `{name}` is called by GPU code"),
                                expression.span,
                                "call a GPU-compatible helper",
                            );
                        } else {
                            self.gpu_functions.insert(function.name.as_str());
                        }
                    }
                    CallTarget::Runtime(operation) => {
                        let builtin = BuiltinTable::all()
                            .iter()
                            .find(|builtin| builtin.runtime_op == *operation)
                            .expect("LIR runtime operations come from the builtin registry");
                        if builtin.domain == BuiltinDomain::Host {
                            self.error(
                                "E0404",
                                format!(
                                    "Host-only builtin `{}` is called by GPU code",
                                    builtin.name
                                ),
                                expression.span,
                                "use a builtin whose domain is GPU or Host/GPU",
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

    fn error(
        &mut self,
        code: &str,
        message: impl Into<String>,
        span: polygl_span::Span,
        suggestion: impl Into<String>,
    ) {
        self.diagnostics.push(
            Diagnostic::new(Severity::Error, code, message, span)
                .with_suggestion(Suggestion::rewrite(span, suggestion)),
        );
    }
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

fn function_calls(block: &Block) -> Vec<String> {
    let mut calls = Vec::new();
    collect_block_calls(block, &mut calls);
    calls
}

fn collect_block_calls(block: &Block, calls: &mut Vec<String>) {
    for statement in &block.statements {
        match &statement.kind {
            StatementKind::Let { init, .. } | StatementKind::Expr(init) => {
                collect_expr_calls(init, calls);
            }
            StatementKind::Assign { target, value } => {
                collect_place_calls(target, calls);
                collect_expr_calls(value, calls);
            }
            StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                collect_expr_calls(condition, calls);
                collect_block_calls(then_block, calls);
                if let Some(else_block) = else_block {
                    collect_block_calls(else_block, calls);
                }
            }
            StatementKind::While { condition, body } => {
                collect_expr_calls(condition, calls);
                collect_block_calls(body, calls);
            }
            StatementKind::For { range, body, .. } => {
                collect_expr_calls(&range.start, calls);
                collect_expr_calls(&range.end, calls);
                collect_block_calls(body, calls);
            }
            StatementKind::Return(value) => {
                if let Some(value) = value {
                    collect_expr_calls(value, calls);
                }
            }
            StatementKind::Break | StatementKind::Continue => {}
        }
    }
}

fn collect_place_calls(place: &Place, calls: &mut Vec<String>) {
    match &place.kind {
        PlaceKind::Variable(_) => {}
        PlaceKind::Index { base, index } => {
            collect_expr_calls(base, calls);
            collect_expr_calls(index, calls);
        }
        PlaceKind::Field { base, .. } => collect_expr_calls(base, calls),
    }
}

fn collect_expr_calls(expression: &Expr, calls: &mut Vec<String>) {
    match &expression.kind {
        ExprKind::Call { target, args } => {
            if let CallTarget::Function(name) = target {
                calls.push(name.clone());
            }
            for argument in args {
                collect_expr_calls(argument, calls);
            }
        }
        ExprKind::Binary { left, right, .. }
        | ExprKind::Index {
            base: left,
            index: right,
        } => {
            collect_expr_calls(left, calls);
            collect_expr_calls(right, calls);
        }
        ExprKind::Unary { operand, .. }
        | ExprKind::Field { base: operand, .. }
        | ExprKind::IsNil(operand)
        | ExprKind::IsFalsy(operand) => collect_expr_calls(operand, calls),
        ExprKind::Array(items) | ExprKind::Vector { args: items, .. } => {
            for item in items {
                collect_expr_calls(item, calls);
            }
        }
        ExprKind::Map(entries) => {
            for entry in entries {
                collect_expr_calls(&entry.key, calls);
                collect_expr_calls(&entry.value, calls);
            }
        }
        ExprKind::Struct { fields, .. } => {
            for field in fields {
                collect_expr_calls(&field.value, calls);
            }
        }
        ExprKind::Literal(_) | ExprKind::Variable(_) | ExprKind::Constant(_) => {}
    }
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
            ],
            span: span(),
        });
        split(&module).expect("fixed varying ABI should validate");
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
            Statement::new(StatementKind::Expr(helper_call), span()),
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
        let split = split(&module).expect("warnings do not block splitting");
        let warning_codes = codes(&split.warnings);
        assert!(warning_codes.contains("W0401"));
        assert!(warning_codes.contains("W0402"));
        assert_eq!(split.host.functions.len(), 2);
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
