use polygl_hir::{
    Block, DomainHint, EntryPoint, EntryPointKind, Expr, ExprKind, Function, Item, Literal, Param,
    PlaceKind, Stmt, StmtKind, Symbol,
};
use ruby_prism::DefNode;

use crate::lowerer::Lowerer;

impl Lowerer<'_, '_, '_> {
    pub(crate) fn lower_def(&mut self, definition: &DefNode<'_>) -> Option<Item> {
        if definition.receiver().is_some() {
            self.unsupported(
                &definition.as_node(),
                "singleton methods are outside Common Core",
                "use a regular `def name` declaration",
            );
            return None;
        }

        let name = self.name(definition.name().as_slice());
        let kind = entry_kind(&name);
        let params = self.lower_params(definition)?;
        self.declared = params
            .iter()
            .map(|parameter| parameter.name.as_str().to_owned())
            .collect();
        let span = self.span(definition.location());
        let mut body = self.lower_body(definition.body(), span);
        self.declared.clear();

        match kind {
            Some(kind) => {
                if matches!(
                    kind,
                    EntryPointKind::Vertex(_) | EntryPointKind::Fragment(_)
                ) {
                    ensure_implicit_return(&mut body);
                }
                Some(Item::Entry(EntryPoint {
                    kind,
                    params,
                    return_type: None,
                    body,
                    span,
                }))
            }
            None => {
                ensure_implicit_return(&mut body);
                Some(Item::Function(Function {
                    name: Symbol::new(name),
                    params,
                    return_type: None,
                    body,
                    span,
                    domain: DomainHint::Auto,
                }))
            }
        }
    }

    pub(crate) fn lower_params(&mut self, definition: &DefNode<'_>) -> Option<Vec<Param>> {
        let Some(parameters) = definition.parameters() else {
            return Some(Vec::new());
        };
        if parameters.block().is_some() {
            self.unsupported_with_code(
                &parameters.as_node(),
                "E0202",
                "block parameters would create escaping closure values",
                "replace the block parameter with a plain function and call it directly",
            );
            return None;
        }
        if !parameters.optionals().is_empty()
            || parameters.rest().is_some()
            || !parameters.posts().is_empty()
            || !parameters.keywords().is_empty()
            || parameters.keyword_rest().is_some()
        {
            self.unsupported(
                &parameters.as_node(),
                "only required positional parameters are supported",
                "replace optional, keyword, rest, and block parameters with required positional parameters",
            );
            return None;
        }

        let mut result = Vec::new();
        for node in parameters.requireds().iter() {
            let Some(parameter) = node.as_required_parameter_node() else {
                self.unsupported(
                    &node,
                    "this parameter form is outside Common Core",
                    "use a required positional parameter",
                );
                return None;
            };
            let name = self.name(parameter.name().as_slice());
            let ty = self.parameter_annotation_for(&name, definition.location());
            result.push(Param {
                name: Symbol::new(name),
                ty,
                span: self.span(parameter.location()),
            });
        }
        Some(result)
    }
}

pub(crate) fn ensure_implicit_return(body: &mut Block) {
    if body.statements.is_empty() {
        body.statements.push(unit_return(body.span));
        return;
    }
    let index = body.statements.len() - 1;
    let span = body.statements[index].span;
    let appended = match &mut body.statements[index].kind {
        StmtKind::Expr(expression) => {
            body.statements[index].kind = StmtKind::Return(Some(expression.clone()));
            None
        }
        StmtKind::Let { name, .. } => {
            let value = Expr::new(ExprKind::Var(name.clone()), span);
            Some(StmtKind::Return(Some(value)))
        }
        StmtKind::Assign { target, .. } => {
            let value = match &target.kind {
                PlaceKind::Var(name) => Some(Expr::new(ExprKind::Var(name.clone()), target.span)),
                PlaceKind::Index { .. } | PlaceKind::Field { .. } => None,
            };
            Some(StmtKind::Return(value))
        }
        StmtKind::If {
            then_block,
            else_block,
            ..
        } => {
            ensure_implicit_return(then_block);
            if let Some(else_block) = else_block {
                ensure_implicit_return(else_block);
            } else {
                *else_block = Some(Block {
                    statements: vec![nil_return(span)],
                    span,
                });
            }
            None
        }
        StmtKind::Return(_) => None,
        StmtKind::While { .. } | StmtKind::For { .. } | StmtKind::Break | StmtKind::Continue => {
            Some(unit_return(span).kind)
        }
    };
    if let Some(kind) = appended {
        body.statements.push(Stmt::new(kind, span));
    }
}

fn unit_return(span: polygl_span::Span) -> Stmt {
    Stmt::new(StmtKind::Return(None), span)
}

fn nil_return(span: polygl_span::Span) -> Stmt {
    let value = Expr::new(ExprKind::Literal(Literal::None), span);
    Stmt::new(StmtKind::Return(Some(value)), span)
}

fn entry_kind(name: &str) -> Option<EntryPointKind> {
    match name {
        "setup" => Some(EntryPointKind::Setup),
        "frame" | "draw" => Some(EntryPointKind::Frame),
        "on_event" => Some(EntryPointKind::OnEvent),
        _ => name
            .strip_prefix("vertex_")
            .filter(|name| !name.is_empty())
            .map(|name| EntryPointKind::Vertex(Symbol::new(name)))
            .or_else(|| {
                name.strip_prefix("fragment_")
                    .filter(|name| !name.is_empty())
                    .map(|name| EntryPointKind::Fragment(Symbol::new(name)))
            }),
    }
}
