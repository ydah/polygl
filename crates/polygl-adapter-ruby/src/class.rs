use std::collections::HashSet;

use polygl_adapter_api::constructor_function_name;
use polygl_hir::{
    Block, DomainHint, Expr, ExprKind, FieldDef, FieldInit, Function, Item, Param, Stmt, StmtKind,
    StructDef, Symbol, TypeExpr, TypeKind,
};
use ruby_prism::{ClassNode, DefNode, Node};

use crate::item::ensure_implicit_return;
use crate::lowerer::Lowerer;
use crate::operator::{binary_operator, unary_operator};

impl Lowerer<'_, '_, '_> {
    pub(crate) fn register_class_shape(&mut self, class: &ClassNode<'_>) {
        let name = self.name(class.name().as_slice());
        self.class_names.insert(name.clone());
        for node in class_body_nodes(class) {
            let Some(definition) = node.as_def_node() else {
                continue;
            };
            let method = self.name(definition.name().as_slice());
            if method == "initialize" {
                for statement in definition_body_nodes(&definition) {
                    if let Some(write) = statement.as_instance_variable_write_node() {
                        let field = instance_field_name(self, write.name().as_slice());
                        self.field_names.insert(field);
                    }
                }
            } else if !conflicts_with_direct_lowering(&method) {
                self.class_methods
                    .entry(name.clone())
                    .or_default()
                    .insert(method);
            }
        }
    }

    pub(crate) fn lower_class(&mut self, class: &ClassNode<'_>) -> Option<Vec<Item>> {
        let node = class.as_node();
        if class.superclass().is_some() {
            self.unsupported_with_code(
                &node,
                "E0203",
                "class inheritance is outside Common Core",
                "replace inheritance with a field that composes another class",
            );
            return None;
        }
        if class.constant_path().as_constant_read_node().is_none() {
            self.unsupported_with_code(
                &node,
                "E0203",
                "nested class paths are outside Common Core",
                "define the class once at the top level",
            );
            return None;
        }

        let name = self.name(class.name().as_slice());
        let mut initializer = None;
        let mut methods = Vec::new();
        for member in class_body_nodes(class) {
            let Some(definition) = member.as_def_node() else {
                self.unsupported_with_code(
                    &member,
                    "E0203",
                    "class bodies may contain only a constructor and instance methods",
                    "move constants and executable statements into plain functions",
                );
                continue;
            };
            if definition.receiver().is_some() {
                self.unsupported_with_code(
                    &definition.as_node(),
                    "E0203",
                    "static and singleton methods are outside Common Core",
                    "replace the static method with a top-level function",
                );
                continue;
            }
            let method_name = self.name(definition.name().as_slice());
            if method_name == "initialize" {
                if initializer.is_some() {
                    self.unsupported_with_code(
                        &definition.as_node(),
                        "E0203",
                        "a Common Core class may have only one constructor",
                        "merge constructor logic into one `initialize` method",
                    );
                } else {
                    initializer = Some(definition);
                }
            } else if conflicts_with_direct_lowering(&method_name) {
                self.unsupported_with_code(
                    &definition.as_node(),
                    "E0203",
                    "operator, index, and attribute-writer methods conflict with Common Core syntax",
                    "replace the method with an ordinary named instance method",
                );
            } else if let Some(method) = self.lower_instance_method(&name, &definition) {
                methods.push(method);
            }
        }

        let (fields, constructor) = self.lower_constructor(&name, initializer.as_ref(), class)?;
        Some(vec![
            Item::Struct(StructDef {
                name: Symbol::new(name),
                fields,
                methods,
                span: self.span(class.location()),
            }),
            Item::Function(constructor),
        ])
    }

    fn lower_constructor(
        &mut self,
        class_name: &str,
        initializer: Option<&DefNode<'_>>,
        class: &ClassNode<'_>,
    ) -> Option<(Vec<FieldDef>, Function)> {
        let span = self.span(class.location());
        let Some(initializer) = initializer else {
            return Some((
                Vec::new(),
                Function {
                    name: Symbol::new(constructor_function_name(class_name)),
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

        let params = self.lower_params(initializer)?;
        self.declared = params
            .iter()
            .map(|parameter| parameter.name.as_str().to_owned())
            .collect();
        let mut fields = Vec::new();
        let mut values = Vec::new();
        let mut initialized = HashSet::new();
        for statement in definition_body_nodes(initializer) {
            let Some(write) = statement.as_instance_variable_write_node() else {
                self.unsupported_with_code(
                    &statement,
                    "E0203",
                    "constructors may only establish instance fields",
                    "assign each field directly with `@field = value` and move other work into a method",
                );
                continue;
            };
            let field_name = instance_field_name(self, write.name().as_slice());
            if !initialized.insert(field_name.clone()) {
                self.unsupported_with_code(
                    &statement,
                    "E0203",
                    "constructors must establish each field exactly once",
                    "combine repeated writes into one `@field = value` assignment",
                );
                continue;
            }
            let value = self.lower_expression(&write.value())?;
            let field_span = self.span(write.name_loc());
            let ty = self
                .annotation_for(&field_name, write.location())
                .or_else(|| field_type_from_parameter(&value, &params));
            fields.push(FieldDef {
                name: Symbol::new(field_name.clone()),
                ty,
                span: field_span,
            });
            values.push(FieldInit {
                name: Symbol::new(field_name),
                value,
                span: self.span(write.location()),
            });
        }
        self.declared.clear();
        let body_span = self.span(initializer.location());
        Some((
            fields,
            Function {
                name: Symbol::new(constructor_function_name(class_name)),
                params,
                return_type: Some(TypeExpr::new(
                    TypeKind::Struct(Symbol::new(class_name)),
                    body_span,
                )),
                body: Block {
                    statements: vec![struct_return(class_name, values, body_span)],
                    span: body_span,
                },
                span: body_span,
                domain: DomainHint::Auto,
            },
        ))
    }

    fn lower_instance_method(
        &mut self,
        class_name: &str,
        definition: &DefNode<'_>,
    ) -> Option<Function> {
        let mut params = self.lower_params(definition)?;
        let span = self.span(definition.location());
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
        self.declared = params
            .iter()
            .map(|parameter| parameter.name.as_str().to_owned())
            .collect();
        self.current_class = Some(class_name.to_owned());
        let mut body = self.lower_body(definition.body(), span);
        self.current_class = None;
        self.declared.clear();
        ensure_implicit_return(&mut body);
        Some(Function {
            name: Symbol::new(self.name(definition.name().as_slice())),
            params,
            return_type: None,
            body,
            span,
            domain: DomainHint::Auto,
        })
    }
}

fn class_body_nodes<'pr>(class: &ClassNode<'pr>) -> Vec<Node<'pr>> {
    class.body().map_or_else(Vec::new, |body| {
        body.as_statements_node().map_or_else(
            || vec![body],
            |statements| statements.body().iter().collect(),
        )
    })
}

fn definition_body_nodes<'pr>(definition: &DefNode<'pr>) -> Vec<Node<'pr>> {
    definition.body().map_or_else(Vec::new, |body| {
        body.as_statements_node().map_or_else(
            || vec![body],
            |statements| statements.body().iter().collect(),
        )
    })
}

fn instance_field_name(lowerer: &Lowerer<'_, '_, '_>, name: &[u8]) -> String {
    lowerer
        .name(name)
        .strip_prefix('@')
        .unwrap_or_default()
        .to_owned()
}

fn conflicts_with_direct_lowering(name: &str) -> bool {
    binary_operator(name).is_some()
        || unary_operator(name).is_some()
        || matches!(name, "!" | "[]" | "[]=")
        || name.ends_with('=')
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
