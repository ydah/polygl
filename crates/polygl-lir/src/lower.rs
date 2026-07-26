use std::cell::RefCell;
use std::collections::HashSet;

use polygl_builtins::{BuiltinTable, BuiltinType, DefaultValue};
use polygl_hir as hir;
use polygl_types::{Type, TypedModule};

use crate::domain::resolve_domains;
use crate::optimize::optimize_module;
use crate::{
    BinaryOp, Block, CallTarget, Constant, Domain, EntryKind, EntryPoint, Expr, ExprKind, Field,
    FieldInit, Function, Literal, MapEntry, Module, Parameter, Place, PlaceKind, Range, Statement,
    StatementKind, StructDef, UnaryOp,
};

#[must_use]
pub fn lower(typed: &TypedModule) -> Module {
    let constants = typed
        .as_hir()
        .items
        .iter()
        .filter_map(|item| match item {
            hir::Item::Const(constant) => Some(constant.name.as_str().to_owned()),
            hir::Item::Function(_) | hir::Item::Struct(_) | hir::Item::Entry(_) => None,
        })
        .collect();
    let mut module = Lowerer {
        constants,
        scopes: RefCell::new(Vec::new()),
    }
    .lower_module(typed.as_hir());
    resolve_domains(&mut module);
    optimize_module(&mut module);
    module
}

struct Lowerer {
    constants: HashSet<String>,
    scopes: RefCell<Vec<HashSet<String>>>,
}

impl Lowerer {
    fn lower_module(&self, source: &hir::Module) -> Module {
        let mut functions = Vec::new();
        let mut structs = Vec::new();
        let mut constants = Vec::new();
        let mut entries = Vec::new();
        for item in &source.items {
            match item {
                hir::Item::Function(function) => functions.push(self.lower_function(function)),
                hir::Item::Struct(definition) => structs.push(self.lower_struct(definition)),
                hir::Item::Const(constant) => constants.push(self.lower_constant(constant)),
                hir::Item::Entry(entry) => entries.push(self.lower_entry(entry)),
            }
        }
        Module {
            functions,
            structs,
            constants,
            entries,
            span: source.span,
        }
    }

    fn lower_function(&self, source: &hir::Function) -> Function {
        self.push_scope(
            source
                .params
                .iter()
                .map(|parameter| parameter.name.as_str().to_owned())
                .collect(),
        );
        let function = Function {
            name: source.name.as_str().to_owned(),
            params: source
                .params
                .iter()
                .map(|parameter| self.lower_parameter(parameter))
                .collect(),
            result: lower_type(
                source
                    .return_type
                    .as_ref()
                    .expect("typed functions have result types"),
            ),
            body: self.lower_block(&source.body),
            domain: lower_domain(source.domain),
            span: source.span,
        };
        self.pop_scope();
        function
    }

    fn lower_entry(&self, source: &hir::EntryPoint) -> EntryPoint {
        self.push_scope(
            source
                .params
                .iter()
                .map(|parameter| parameter.name.as_str().to_owned())
                .collect(),
        );
        let entry = EntryPoint {
            kind: lower_entry_kind(&source.kind),
            params: source
                .params
                .iter()
                .map(|parameter| self.lower_parameter(parameter))
                .collect(),
            result: source.return_type.as_ref().map_or(Type::Unit, lower_type),
            body: self.lower_block(&source.body),
            domain: match source.kind.domain() {
                hir::DomainHint::Host => Domain::Host,
                hir::DomainHint::Gpu => Domain::Gpu,
                hir::DomainHint::Auto => Domain::Shared,
            },
            span: source.span,
        };
        self.pop_scope();
        entry
    }

    fn lower_parameter(&self, source: &hir::Param) -> Parameter {
        Parameter {
            name: source.name.as_str().to_owned(),
            ty: lower_type(
                source
                    .ty
                    .as_ref()
                    .expect("typed parameters have concrete types"),
            ),
            span: source.span,
        }
    }

    fn lower_struct(&self, source: &hir::StructDef) -> StructDef {
        debug_assert!(
            source.methods.is_empty(),
            "typed M1 modules reject struct methods"
        );
        StructDef {
            name: source.name.as_str().to_owned(),
            fields: source
                .fields
                .iter()
                .map(|field| Field {
                    name: field.name.as_str().to_owned(),
                    ty: lower_type(
                        field
                            .ty
                            .as_ref()
                            .expect("typed struct fields have concrete types"),
                    ),
                    span: field.span,
                })
                .collect(),
            span: source.span,
        }
    }

    fn lower_constant(&self, source: &hir::ConstDef) -> Constant {
        Constant {
            name: source.name.as_str().to_owned(),
            ty: lower_type(
                source
                    .ty
                    .as_ref()
                    .expect("typed constants have concrete types"),
            ),
            value: self.lower_expr(&source.value),
            domain: Domain::Shared,
            span: source.span,
        }
    }

    fn lower_block(&self, source: &hir::Block) -> Block {
        self.push_scope(HashSet::new());
        let statements = source
            .statements
            .iter()
            .flat_map(|statement| self.lower_statement(statement))
            .collect::<Vec<_>>();
        let block = Block {
            statements,
            span: source.span,
        };
        self.pop_scope();
        block
    }

    fn lower_statement(&self, source: &hir::Stmt) -> Vec<Statement> {
        let span = source.span;
        let declaration = match &source.kind {
            hir::StmtKind::Let { name, .. } => Some(name.as_str().to_owned()),
            _ => None,
        };
        let kind = match &source.kind {
            hir::StmtKind::Let { name, ty, init } => StatementKind::Let {
                name: name.as_str().to_owned(),
                ty: lower_type(ty.as_ref().expect("typed bindings have concrete types")),
                init: self.lower_expr(init),
            },
            hir::StmtKind::Assign { target, value } => StatementKind::Assign {
                target: self.lower_place(target),
                value: self.lower_expr(value),
            },
            hir::StmtKind::Expr(expression) => StatementKind::Expr(self.lower_expr(expression)),
            hir::StmtKind::If {
                condition,
                then_block,
                else_block,
            } => StatementKind::If {
                condition: self.lower_expr(condition),
                then_block: self.lower_block(then_block),
                else_block: else_block
                    .as_ref()
                    .map(|else_block| self.lower_block(else_block)),
            },
            hir::StmtKind::While { condition, body } => StatementKind::While {
                condition: self.lower_expr(condition),
                body: self.lower_block(body),
            },
            hir::StmtKind::For {
                variable,
                range,
                body,
            } => {
                let range = Range {
                    start: self.lower_expr(&range.start),
                    end: self.lower_expr(&range.end),
                    inclusive: range.inclusive,
                    span: range.span,
                };
                self.push_scope(HashSet::from([variable.as_str().to_owned()]));
                let body = self.lower_block(body);
                self.pop_scope();
                StatementKind::For {
                    variable: variable.as_str().to_owned(),
                    range,
                    body,
                }
            }
            hir::StmtKind::Return(Some(value)) if expression_type(value) == Type::Unit => {
                return vec![
                    Statement::new(StatementKind::Expr(self.lower_expr(value)), value.span),
                    Statement::new(StatementKind::Return(None), span),
                ];
            }
            hir::StmtKind::Return(value) => {
                StatementKind::Return(value.as_ref().map(|value| self.lower_expr(value)))
            }
            hir::StmtKind::Break => StatementKind::Break,
            hir::StmtKind::Continue => StatementKind::Continue,
        };
        if let Some(name) = declaration {
            self.declare_local(name);
        }
        vec![Statement::new(kind, span)]
    }

    fn lower_place(&self, source: &hir::Place) -> Place {
        let kind = match &source.kind {
            hir::PlaceKind::Var(name) => PlaceKind::Variable(name.as_str().to_owned()),
            hir::PlaceKind::Index { base, index } => PlaceKind::Index {
                base: self.lower_expr(base),
                index: self.lower_expr(index),
            },
            hir::PlaceKind::Field { base, field } => PlaceKind::Field {
                base: self.lower_expr(base),
                field: field.as_str().to_owned(),
            },
        };
        Place {
            kind,
            span: source.span,
        }
    }

    fn lower_expr(&self, source: &hir::Expr) -> Expr {
        let ty = expression_type(source);
        let kind = match &source.kind {
            hir::ExprKind::Literal(literal) => ExprKind::Literal(lower_literal(literal)),
            hir::ExprKind::Var(name) => {
                let name = name.as_str();
                if self.is_local(name) {
                    ExprKind::Variable(name.to_owned())
                } else if self.constants.contains(name) {
                    ExprKind::Constant(name.to_owned())
                } else {
                    ExprKind::Variable(name.to_owned())
                }
            }
            hir::ExprKind::Binary { op, left, right } => ExprKind::Binary {
                op: lower_binary(*op),
                left: Box::new(self.lower_expr(left)),
                right: Box::new(self.lower_expr(right)),
            },
            hir::ExprKind::Unary { op, operand } => ExprKind::Unary {
                op: lower_unary(*op),
                operand: Box::new(self.lower_expr(operand)),
            },
            hir::ExprKind::Call { callee, args } => {
                let mut args = args
                    .iter()
                    .map(|argument| self.lower_expr(argument))
                    .collect::<Vec<_>>();
                let target = match callee {
                    hir::Callee::User(name) => CallTarget::Function(name.as_str().to_owned()),
                    hir::Callee::Builtin(id) => {
                        let builtin = BuiltinTable::all()
                            .iter()
                            .find(|builtin| builtin.id == *id)
                            .expect("typed HIR contains only registered builtins");
                        for parameter in builtin.signature.params.iter().skip(args.len()) {
                            let default = parameter
                                .default
                                .expect("only optional builtin parameters may be omitted");
                            args.push(default_expression(default, parameter.ty, source.span));
                        }
                        CallTarget::Runtime(builtin.runtime_op)
                    }
                };
                ExprKind::Call { target, args }
            }
            hir::ExprKind::Index { base, index } => ExprKind::Index {
                base: Box::new(self.lower_expr(base)),
                index: Box::new(self.lower_expr(index)),
            },
            hir::ExprKind::Field { base, field } => ExprKind::Field {
                base: Box::new(self.lower_expr(base)),
                field: field.as_str().to_owned(),
            },
            hir::ExprKind::ArrayLength(value) => {
                ExprKind::ArrayLength(Box::new(self.lower_expr(value)))
            }
            hir::ExprKind::Array(items) => {
                ExprKind::Array(items.iter().map(|item| self.lower_expr(item)).collect())
            }
            hir::ExprKind::Map(entries) => ExprKind::Map(
                entries
                    .iter()
                    .map(|entry| MapEntry {
                        key: self.lower_expr(&entry.key),
                        value: self.lower_expr(&entry.value),
                        span: entry.span,
                    })
                    .collect(),
            ),
            hir::ExprKind::Struct { name, fields } => ExprKind::Struct {
                name: name.as_str().to_owned(),
                fields: fields
                    .iter()
                    .map(|field| FieldInit {
                        name: field.name.as_str().to_owned(),
                        value: self.lower_expr(&field.value),
                        span: field.span,
                    })
                    .collect(),
            },
            hir::ExprKind::Vector { size, args } => ExprKind::Vector {
                size: *size,
                args: args
                    .iter()
                    .map(|argument| self.lower_expr(argument))
                    .collect(),
            },
            hir::ExprKind::NilCheck(value) => ExprKind::IsNil(Box::new(self.lower_expr(value))),
            hir::ExprKind::FalsyCheck(value) => ExprKind::IsFalsy(Box::new(self.lower_expr(value))),
        };
        Expr::new(kind, ty, source.span)
    }

    fn push_scope(&self, bindings: HashSet<String>) {
        self.scopes.borrow_mut().push(bindings);
    }

    fn pop_scope(&self) {
        self.scopes
            .borrow_mut()
            .pop()
            .expect("lowering scopes are balanced");
    }

    fn declare_local(&self, name: String) {
        self.scopes
            .borrow_mut()
            .last_mut()
            .expect("bindings are lowered inside a scope")
            .insert(name);
    }

    fn is_local(&self, name: &str) -> bool {
        self.scopes
            .borrow()
            .iter()
            .rev()
            .any(|scope| scope.contains(name))
    }
}

fn expression_type(expression: &hir::Expr) -> Type {
    lower_type(
        expression
            .ty
            .as_ref()
            .expect("typed expressions have concrete types"),
    )
}

fn lower_type(source: &hir::TypeExpr) -> Type {
    match &source.kind {
        hir::TypeKind::Unit => Type::Unit,
        hir::TypeKind::Int => Type::Int,
        hir::TypeKind::Float => Type::Float,
        hir::TypeKind::Bool => Type::Bool,
        hir::TypeKind::Str => Type::Str,
        hir::TypeKind::Array(element) => Type::Array(Box::new(lower_type(element))),
        hir::TypeKind::Map(value) => Type::Map(Box::new(lower_type(value))),
        hir::TypeKind::Option(value) => Type::Option(Box::new(lower_type(value))),
        hir::TypeKind::Struct(name) => Type::Struct(name.clone()),
        hir::TypeKind::Vector(size) => Type::Vector(*size),
        hir::TypeKind::Matrix(size) => Type::Matrix(*size),
        hir::TypeKind::Opaque(kind) => Type::Opaque(*kind),
    }
}

const fn lower_domain(source: hir::DomainHint) -> Domain {
    match source {
        hir::DomainHint::Host => Domain::Host,
        hir::DomainHint::Gpu => Domain::Gpu,
        hir::DomainHint::Auto => Domain::Shared,
    }
}

fn lower_entry_kind(source: &hir::EntryPointKind) -> EntryKind {
    match source {
        hir::EntryPointKind::Setup => EntryKind::Setup,
        hir::EntryPointKind::Frame => EntryKind::Frame,
        hir::EntryPointKind::OnEvent => EntryKind::OnEvent,
        hir::EntryPointKind::Vertex(name) => EntryKind::Vertex(name.as_str().to_owned()),
        hir::EntryPointKind::Fragment(name) => EntryKind::Fragment(name.as_str().to_owned()),
    }
}

fn lower_literal(source: &hir::Literal) -> Literal {
    match source {
        hir::Literal::Int(value) => Literal::Int(*value),
        hir::Literal::Float(value) => Literal::Float(*value),
        hir::Literal::Bool(value) => Literal::Bool(*value),
        hir::Literal::Str(value) => Literal::Str(value.clone()),
        hir::Literal::None => Literal::None,
    }
}

fn lower_binary(source: hir::BinOp) -> BinaryOp {
    match source {
        hir::BinOp::Add => BinaryOp::Add,
        hir::BinOp::Sub => BinaryOp::Subtract,
        hir::BinOp::Mul => BinaryOp::Multiply,
        hir::BinOp::DivInt => BinaryOp::IntegerDivide,
        hir::BinOp::DivFloat => BinaryOp::FloatDivide,
        hir::BinOp::RemFloor => BinaryOp::FloorRemainder,
        hir::BinOp::RemTrunc => BinaryOp::TruncatingRemainder,
        hir::BinOp::Eq => BinaryOp::Equal,
        hir::BinOp::NotEq => BinaryOp::NotEqual,
        hir::BinOp::Less => BinaryOp::Less,
        hir::BinOp::LessEq => BinaryOp::LessEqual,
        hir::BinOp::Greater => BinaryOp::Greater,
        hir::BinOp::GreaterEq => BinaryOp::GreaterEqual,
        hir::BinOp::And => BinaryOp::And,
        hir::BinOp::Or => BinaryOp::Or,
        hir::BinOp::StrConcat => BinaryOp::StringConcat,
    }
}

const fn lower_unary(source: hir::UnOp) -> UnaryOp {
    match source {
        hir::UnOp::Neg => UnaryOp::Negate,
        hir::UnOp::Not => UnaryOp::Not,
    }
}

fn default_expression(default: DefaultValue, ty: BuiltinType, span: polygl_span::Span) -> Expr {
    let (literal, ty) = match (default, ty) {
        (DefaultValue::Int(value), BuiltinType::Int) => (Literal::Int(value), Type::Int),
        (DefaultValue::Float(value), BuiltinType::Float) => (Literal::Float(value), Type::Float),
        (DefaultValue::Bool(value), BuiltinType::Bool) => (Literal::Bool(value), Type::Bool),
        (_, BuiltinType::Opaque(_)) => {
            unreachable!("opaque builtin parameters cannot have literal defaults")
        }
        _ => unreachable!("builtin registry validation guarantees matching defaults"),
    };
    Expr::new(ExprKind::Literal(literal), ty, span)
}
