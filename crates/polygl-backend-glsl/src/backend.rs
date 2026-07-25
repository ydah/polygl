use std::collections::{BTreeMap, HashMap, HashSet};

use polygl_lir::{
    Block, CallTarget, EntryKind, EntryPoint, Expr, ExprKind, Module, Place, PlaceKind,
    StatementKind,
};
use polygl_types::Type;

use crate::emitter::{Emitter, attribute_bindings, uses_time};
use crate::{EmitError, GlslArtifacts, ShaderArtifact, ShaderStage, UniformBinding, UniformSource};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlslBackend;

impl GlslBackend {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub fn generate(&self, program: &Module) -> Result<GlslArtifacts, EmitError> {
        let mut pairs: BTreeMap<&str, (Option<&EntryPoint>, Option<&EntryPoint>)> = BTreeMap::new();
        for entry in &program.entries {
            match &entry.kind {
                EntryKind::Vertex(name) => pairs.entry(name).or_default().0 = Some(entry),
                EntryKind::Fragment(name) => pairs.entry(name).or_default().1 = Some(entry),
                EntryKind::Setup | EntryKind::Frame | EntryKind::OnEvent => {}
            }
        }

        let mut shaders = Vec::with_capacity(pairs.len());
        for (name, (vertex, fragment)) in pairs {
            let (Some(vertex), Some(fragment)) = (vertex, fragment) else {
                return Err(EmitError::IncompletePair(name.to_owned()));
            };
            let pair_program = project_pair(program, vertex, fragment);
            let (projected_vertex, projected_fragment) = pair_entries(&pair_program, name)
                .ok_or_else(|| EmitError::IncompletePair(name.to_owned()))?;
            let vertex_source = Emitter::new(
                &pair_program,
                name,
                ShaderStage::Vertex,
                projected_vertex,
                projected_fragment,
            )
            .emit()?;
            let fragment_source = Emitter::new(
                &pair_program,
                name,
                ShaderStage::Fragment,
                projected_fragment,
                projected_vertex,
            )
            .emit()?;
            let mut uniforms = Vec::new();
            if uses_time(&pair_program) {
                uniforms.push(UniformBinding {
                    name: "u_time".to_owned(),
                    glsl_name: "u_time".to_owned(),
                    ty: Type::Float,
                    source: UniformSource::Automatic,
                });
            }
            shaders.push(ShaderArtifact {
                name: name.to_owned(),
                vertex: vertex_source,
                fragment: fragment_source,
                attributes: attribute_bindings(vertex)?,
                uniforms,
                vertex_span: vertex.span,
                fragment_span: fragment.span,
            });
        }
        Ok(GlslArtifacts { shaders })
    }
}

fn pair_entries<'module>(
    module: &'module Module,
    name: &str,
) -> Option<(&'module EntryPoint, &'module EntryPoint)> {
    let vertex = module
        .entries
        .iter()
        .find(|entry| matches!(&entry.kind, EntryKind::Vertex(entry_name) if entry_name == name))?;
    let fragment = module.entries.iter().find(
        |entry| matches!(&entry.kind, EntryKind::Fragment(entry_name) if entry_name == name),
    )?;
    Some((vertex, fragment))
}

fn project_pair(module: &Module, vertex: &EntryPoint, fragment: &EntryPoint) -> Module {
    let functions = module
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect::<HashMap<_, _>>();
    let constants = module
        .constants
        .iter()
        .map(|constant| (constant.name.as_str(), constant))
        .collect::<HashMap<_, _>>();
    let mut reachable_functions = HashSet::new();
    let mut reachable_constants = HashSet::new();
    collect_block_dependencies(
        &vertex.body,
        &mut reachable_functions,
        &mut reachable_constants,
    );
    collect_block_dependencies(
        &fragment.body,
        &mut reachable_functions,
        &mut reachable_constants,
    );

    let mut scanned_functions = HashSet::new();
    let mut scanned_constants = HashSet::new();
    loop {
        let pending_functions = reachable_functions
            .iter()
            .filter(|name| !scanned_functions.contains(*name))
            .cloned()
            .collect::<Vec<_>>();
        let pending_constants = reachable_constants
            .iter()
            .filter(|name| !scanned_constants.contains(*name))
            .cloned()
            .collect::<Vec<_>>();
        if pending_functions.is_empty() && pending_constants.is_empty() {
            break;
        }
        for name in pending_functions {
            scanned_functions.insert(name.clone());
            if let Some(function) = functions.get(name.as_str()) {
                collect_block_dependencies(
                    &function.body,
                    &mut reachable_functions,
                    &mut reachable_constants,
                );
            }
        }
        for name in pending_constants {
            scanned_constants.insert(name.clone());
            if let Some(constant) = constants.get(name.as_str()) {
                collect_expr_dependencies(
                    &constant.value,
                    &mut reachable_functions,
                    &mut reachable_constants,
                );
            }
        }
    }

    Module {
        functions: module
            .functions
            .iter()
            .filter(|function| reachable_functions.contains(&function.name))
            .cloned()
            .collect(),
        structs: module.structs.clone(),
        constants: module
            .constants
            .iter()
            .filter(|constant| reachable_constants.contains(&constant.name))
            .cloned()
            .collect(),
        entries: vec![vertex.clone(), fragment.clone()],
        span: module.span,
    }
}

fn collect_block_dependencies(
    block: &Block,
    functions: &mut HashSet<String>,
    constants: &mut HashSet<String>,
) {
    for statement in &block.statements {
        match &statement.kind {
            StatementKind::Let { init, .. } | StatementKind::Expr(init) => {
                collect_expr_dependencies(init, functions, constants);
            }
            StatementKind::Assign { target, value } => {
                collect_place_dependencies(target, functions, constants);
                collect_expr_dependencies(value, functions, constants);
            }
            StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                collect_expr_dependencies(condition, functions, constants);
                collect_block_dependencies(then_block, functions, constants);
                if let Some(else_block) = else_block {
                    collect_block_dependencies(else_block, functions, constants);
                }
            }
            StatementKind::While { condition, body } => {
                collect_expr_dependencies(condition, functions, constants);
                collect_block_dependencies(body, functions, constants);
            }
            StatementKind::For { range, body, .. } => {
                collect_expr_dependencies(&range.start, functions, constants);
                collect_expr_dependencies(&range.end, functions, constants);
                collect_block_dependencies(body, functions, constants);
            }
            StatementKind::Return(value) => {
                if let Some(value) = value {
                    collect_expr_dependencies(value, functions, constants);
                }
            }
            StatementKind::Break | StatementKind::Continue => {}
        }
    }
}

fn collect_place_dependencies(
    place: &Place,
    functions: &mut HashSet<String>,
    constants: &mut HashSet<String>,
) {
    match &place.kind {
        PlaceKind::Variable(_) => {}
        PlaceKind::Index { base, index } => {
            collect_expr_dependencies(base, functions, constants);
            collect_expr_dependencies(index, functions, constants);
        }
        PlaceKind::Field { base, .. } => {
            collect_expr_dependencies(base, functions, constants);
        }
    }
}

fn collect_expr_dependencies(
    expression: &Expr,
    functions: &mut HashSet<String>,
    constants: &mut HashSet<String>,
) {
    match &expression.kind {
        ExprKind::Constant(name) => {
            constants.insert(name.clone());
        }
        ExprKind::Call { target, args } => {
            if let CallTarget::Function(name) = target {
                functions.insert(name.clone());
            }
            for argument in args {
                collect_expr_dependencies(argument, functions, constants);
            }
        }
        ExprKind::Binary { left, right, .. }
        | ExprKind::Index {
            base: left,
            index: right,
        } => {
            collect_expr_dependencies(left, functions, constants);
            collect_expr_dependencies(right, functions, constants);
        }
        ExprKind::Unary { operand, .. }
        | ExprKind::Field { base: operand, .. }
        | ExprKind::IsNil(operand)
        | ExprKind::IsFalsy(operand) => {
            collect_expr_dependencies(operand, functions, constants);
        }
        ExprKind::Array(items) | ExprKind::Vector { args: items, .. } => {
            for item in items {
                collect_expr_dependencies(item, functions, constants);
            }
        }
        ExprKind::Map(entries) => {
            for entry in entries {
                collect_expr_dependencies(&entry.key, functions, constants);
                collect_expr_dependencies(&entry.value, functions, constants);
            }
        }
        ExprKind::Struct { fields, .. } => {
            for field in fields {
                collect_expr_dependencies(&field.value, functions, constants);
            }
        }
        ExprKind::Literal(_) | ExprKind::Variable(_) => {}
    }
}
