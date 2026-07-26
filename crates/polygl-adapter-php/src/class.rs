use std::collections::HashSet;

use mago_span::HasSpan;
use mago_syntax::cst::{
    Access, AssignmentOperator, Class, ClassLikeMember, ClassLikeMemberSelector, Expression,
    Method, MethodBody, Statement, Variable,
};
use polygl_hir::{
    Block, DomainHint, Expr, ExprKind, FieldDef, FieldInit, Function, Item, Param, Stmt, StmtKind,
    StructDef, Symbol, TypeExpr, TypeKind,
};

use crate::lowerer::Lowerer;

impl Lowerer<'_, '_, '_> {
    pub(crate) fn register_class_shape(&mut self, class: &Class<'_>) {
        let class_name = self.name(class.name.value);
        self.class_names.insert(class_name.clone());
        for member in class.members.iter() {
            let ClassLikeMember::Method(method) = member else {
                continue;
            };
            let method_name = self.name(method.name.value);
            if method_name == "__construct" {
                let MethodBody::Concrete(body) = &method.body else {
                    continue;
                };
                for statement in body.statements.iter() {
                    if let Some((field, _)) = constructor_field_assignment(statement) {
                        self.field_names.insert(self.name(field.value));
                    }
                }
            } else {
                self.class_methods
                    .entry(class_name.clone())
                    .or_default()
                    .insert(method_name);
            }
        }
    }

    pub(crate) fn lower_class(&mut self, class: &Class<'_>) -> Option<Vec<Item>> {
        if !class.attribute_lists.is_empty()
            || !class.modifiers.is_empty()
            || class.extends.is_some()
            || class.implements.is_some()
        {
            self.unsupported_with_code(
                class.span(),
                "E0203",
                "class attributes, modifiers, inheritance, and interfaces are outside Common Core",
                "use an unmodified top-level class and compose behavior through fields",
            );
            return None;
        }

        let class_name = self.name(class.name.value);
        let mut constructor = None;
        let mut methods = Vec::new();
        for member in class.members.iter() {
            let ClassLikeMember::Method(method) = member else {
                self.unsupported_with_code(
                    member.span(),
                    "E0203",
                    "class bodies may contain only a constructor and instance methods",
                    "establish fields in `__construct` and move constants into top-level functions",
                );
                continue;
            };
            let method_name = self.name(method.name.value);
            if method_name == "__construct" {
                if constructor.is_some() {
                    self.unsupported_with_code(
                        method.span(),
                        "E0203",
                        "a Common Core class may have only one constructor",
                        "merge constructor logic into one `__construct` method",
                    );
                } else {
                    constructor = Some(method);
                }
            } else if let Some(method) = self.lower_instance_method(&class_name, method) {
                methods.push(method);
            }
        }

        let (fields, constructor) =
            self.lower_constructor(&class_name, constructor, class.span())?;
        Some(vec![
            Item::Struct(StructDef {
                name: Symbol::new(class_name),
                fields,
                methods,
                span: self.span(class.span()),
            }),
            Item::Function(constructor),
        ])
    }

    fn lower_constructor(
        &mut self,
        class_name: &str,
        constructor: Option<&Method<'_>>,
        class_span: mago_span::Span,
    ) -> Option<(Vec<FieldDef>, Function)> {
        let span = self.span(class_span);
        let Some(constructor) = constructor else {
            return Some((
                Vec::new(),
                Function {
                    name: Symbol::new(constructor_name(class_name)),
                    params: Vec::new(),
                    return_type: Some(TypeExpr::new(
                        TypeKind::Struct(Symbol::new(class_name)),
                        span,
                    )),
                    body: Block {
                        statements: vec![struct_return(class_name, Vec::new(), span)],
                        span,
                    },
                    span,
                    domain: DomainHint::Auto,
                },
            ));
        };
        if !valid_method_shape(constructor) {
            self.unsupported_with_code(
                constructor.span(),
                "E0203",
                "constructor modifiers, attributes, references, and abstract bodies are outside Common Core",
                "use an unmodified concrete `function __construct(...)` method",
            );
            return None;
        }
        if constructor.return_type_hint.is_some() {
            self.unsupported_with_code(
                constructor.span(),
                "E0203",
                "constructors cannot declare a Common Core return type",
                "remove the constructor return type",
            );
            return None;
        }
        let params = self.lower_parameters(
            &constructor.parameter_list,
            constructor.span().start_offset() as usize,
        )?;
        self.declared = params
            .iter()
            .map(|parameter| parameter.name.as_str().to_owned())
            .collect();
        let MethodBody::Concrete(body) = &constructor.body else {
            unreachable!("method shape was checked");
        };
        let mut fields = Vec::new();
        let mut values = Vec::new();
        let mut initialized = HashSet::new();
        for statement in body.statements.iter() {
            let Some((field, value)) = constructor_field_assignment(statement) else {
                self.unsupported_with_code(
                    statement.span(),
                    "E0203",
                    "constructors may only establish instance fields",
                    "assign each field directly with `$this->field = $value` and move other work into a method",
                );
                continue;
            };
            let field_name = self.name(field.value);
            if !initialized.insert(field_name.clone()) {
                self.unsupported_with_code(
                    statement.span(),
                    "E0203",
                    "constructors must establish each field exactly once",
                    "combine repeated writes into one `$this->field = $value` assignment",
                );
                continue;
            }
            let annotation = self.annotation_for(&field_name, statement.span());
            let value = self.lower_expression_with_expected(value, annotation.as_ref())?;
            let ty = annotation.or_else(|| field_type_from_parameter(&value, &params));
            fields.push(FieldDef {
                name: Symbol::new(field_name.clone()),
                ty,
                span: self.span(field.span()),
            });
            values.push(FieldInit {
                name: Symbol::new(field_name),
                value,
                span: self.span(statement.span()),
            });
        }
        self.current_class = None;
        self.declared.clear();
        let body_span = self.span(body.span());
        Some((
            fields,
            Function {
                name: Symbol::new(constructor_name(class_name)),
                params,
                return_type: Some(TypeExpr::new(
                    TypeKind::Struct(Symbol::new(class_name)),
                    body_span,
                )),
                body: Block {
                    statements: vec![struct_return(class_name, values, body_span)],
                    span: body_span,
                },
                span: self.span(constructor.span()),
                domain: DomainHint::Auto,
            },
        ))
    }

    fn lower_instance_method(&mut self, class_name: &str, method: &Method<'_>) -> Option<Function> {
        if !valid_method_shape(method) {
            self.unsupported_with_code(
                method.span(),
                "E0203",
                "method modifiers, attributes, references, and abstract bodies are outside Common Core",
                "use an unmodified concrete instance method or a top-level function",
            );
            return None;
        }
        let span = self.span(method.span());
        let mut params = self.lower_parameters(
            &method.parameter_list,
            method.span().start_offset() as usize,
        )?;
        params.insert(
            0,
            Param {
                name: Symbol::new("self"),
                ty: Some(TypeExpr::new(
                    TypeKind::Struct(Symbol::new(class_name)),
                    span,
                )),
                span,
            },
        );
        let return_type = self.lower_return_hint(method.return_type_hint.as_ref())?;
        self.declared = params
            .iter()
            .map(|parameter| parameter.name.as_str().to_owned())
            .collect();
        self.current_class = Some(class_name.to_owned());
        let MethodBody::Concrete(body) = &method.body else {
            unreachable!("method shape was checked");
        };
        let body = self.lower_block(body);
        self.current_class = None;
        self.declared.clear();
        Some(Function {
            name: Symbol::new(self.name(method.name.value)),
            params,
            return_type,
            body,
            span,
            domain: DomainHint::Auto,
        })
    }
}

pub(crate) fn constructor_name(class_name: &str) -> String {
    format!("{class_name}::new")
}

fn valid_method_shape(method: &Method<'_>) -> bool {
    method.attribute_lists.is_empty()
        && method.modifiers.is_empty()
        && method.ampersand.is_none()
        && matches!(method.body, MethodBody::Concrete(_))
}

fn constructor_field_assignment<'arena>(
    statement: &'arena Statement<'arena>,
) -> Option<(
    &'arena mago_syntax::cst::LocalIdentifier<'arena>,
    &'arena Expression<'arena>,
)> {
    let Statement::Expression(statement) = statement else {
        return None;
    };
    let Expression::Assignment(assignment) = statement.expression else {
        return None;
    };
    if !matches!(assignment.operator, AssignmentOperator::Assign(_)) {
        return None;
    }
    let Expression::Access(Access::Property(access)) = assignment.lhs else {
        return None;
    };
    let Expression::Variable(Variable::Direct(receiver)) = access.object else {
        return None;
    };
    if receiver.name != b"$this" {
        return None;
    }
    let ClassLikeMemberSelector::Identifier(field) = &access.property else {
        return None;
    };
    Some((field, assignment.rhs))
}

fn field_type_from_parameter(value: &Expr, params: &[Param]) -> Option<TypeExpr> {
    let ExprKind::Var(name) = &value.kind else {
        return None;
    };
    params
        .iter()
        .find(|parameter| parameter.name == *name)
        .and_then(|parameter| parameter.ty.clone())
}

fn struct_return(class_name: &str, fields: Vec<FieldInit>, span: polygl_span::Span) -> Stmt {
    Stmt::new(
        StmtKind::Return(Some(Expr::new(
            ExprKind::Struct {
                name: Symbol::new(class_name),
                fields,
            },
            span,
        ))),
        span,
    )
}
