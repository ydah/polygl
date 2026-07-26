use std::collections::{HashMap, HashSet};

use polygl_adapter_api::{
    LowerCtx, automatic_uniform_type, canonical_entry_kind, constructor_function_name,
    vector_constructor_size,
};
use polygl_adapter_treesitter_util::{
    first_named_field, named_children, named_field_children, node_span, node_text,
};
use polygl_hir::{
    BinOp, Block, Callee, ConstDef, DomainHint, EntryPoint, EntryPointKind, Expr, ExprKind,
    FieldDef, FieldInit, Function, Item, Literal, MapEntry, Module, Param, Place, PlaceKind,
    RangeExpr, Stmt, StmtKind, StructDef, Symbol, TypeExpr, TypeKind, UnOp,
};
use polygl_span::{Diagnostic, Diagnostics, Severity, SourceFile, Span, Suggestion};
use tree_sitter::Node;

use crate::annotation::Annotations;

struct PackageSection<'tree> {
    name: String,
    header: Node<'tree>,
    subroutines: Vec<Node<'tree>>,
}

pub(crate) struct Lowerer<'source, 'context, 'resolver> {
    source: &'source SourceFile,
    context: &'context mut LowerCtx<'resolver>,
    diagnostics: Diagnostics,
    annotations: Annotations,
    declared: HashSet<String>,
    function_names: HashSet<String>,
    constant_names: HashSet<String>,
    class_names: HashSet<String>,
    class_methods: HashMap<String, HashSet<String>>,
    current_class: Option<String>,
    loop_depth: usize,
    shader_annotation_anchor: Option<usize>,
}

impl<'source, 'context, 'resolver> Lowerer<'source, 'context, 'resolver> {
    pub(crate) fn new(
        source: &'source SourceFile,
        context: &'context mut LowerCtx<'resolver>,
        annotations: Annotations,
    ) -> Self {
        Self {
            source,
            context,
            diagnostics: Diagnostics::new(),
            annotations,
            declared: HashSet::new(),
            function_names: HashSet::new(),
            constant_names: HashSet::new(),
            class_names: HashSet::new(),
            class_methods: HashMap::new(),
            current_class: None,
            loop_depth: 0,
            shader_annotation_anchor: None,
        }
    }

    pub(crate) fn lower_program(mut self, root: Node<'_>) -> Result<Module, Diagnostics> {
        let mut current_package = "main".to_owned();
        let mut main_nodes = Vec::new();
        let mut packages: Vec<PackageSection<'_>> = Vec::new();

        for node in named_children(root) {
            match node.kind() {
                "comment" | "pod" => {}
                "package_statement" => {
                    let Some(name_node) = first_named_field(node, "name") else {
                        self.unsupported(
                            node,
                            "package declarations require a static name",
                            "write `package Name;` or `package main;`",
                        );
                        continue;
                    };
                    current_package = node_text(self.source, name_node).trim().to_owned();
                    if current_package != "main" {
                        if packages
                            .iter()
                            .any(|package| package.name == current_package)
                        {
                            self.unsupported_with_code(
                                node,
                                "E0203",
                                "a Common Core package must be declared in one contiguous section",
                                "merge the package methods under one `package Name;` declaration",
                            );
                        } else {
                            packages.push(PackageSection {
                                name: current_package.clone(),
                                header: node,
                                subroutines: Vec::new(),
                            });
                        }
                    }
                }
                "subroutine_declaration_statement" => {
                    if current_package == "main" {
                        main_nodes.push(node);
                    } else if let Some(package) = packages
                        .iter_mut()
                        .find(|package| package.name == current_package)
                    {
                        package.subroutines.push(node);
                    }
                }
                "expression_statement" if current_package == "main" => main_nodes.push(node),
                "use_statement" if allowed_pragma(node_text(self.source, node)) => {}
                "use_version_statement" if current_package == "main" => {}
                _ => self.unsupported(
                    node,
                    "this Perl top-level statement is outside Common Core",
                    "move executable code into `sub setup` and keep one source file",
                ),
            }
        }

        for node in &main_nodes {
            if node.kind() == "subroutine_declaration_statement" {
                if let Some(name) = self.subroutine_name(*node) {
                    self.function_names.insert(name);
                }
            } else if let Some(name) = self.top_level_constant_name(*node) {
                self.constant_names.insert(name);
            }
        }
        for package in &packages {
            self.class_names.insert(package.name.clone());
            for subroutine in &package.subroutines {
                if let Some(name) = self.subroutine_name(*subroutine)
                    && name != "new"
                {
                    self.class_methods
                        .entry(package.name.clone())
                        .or_default()
                        .insert(name);
                }
            }
        }

        let mut items = Vec::new();
        for package in &packages {
            if let Some(class_items) = self.lower_class(package) {
                items.extend(class_items);
            }
        }
        for node in main_nodes {
            match node.kind() {
                "subroutine_declaration_statement" => {
                    if let Some(item) = self.lower_subroutine(node) {
                        items.push(item);
                    }
                }
                "expression_statement" => {
                    if let Some(item) = self.lower_constant(node) {
                        items.push(item);
                    }
                }
                _ => unreachable!("main nodes were filtered"),
            }
        }

        self.annotations.report_unused(&mut self.diagnostics);
        let module = Module {
            items,
            span: node_span(self.source, root),
        };
        if self.diagnostics.has_errors() {
            Err(self.diagnostics)
        } else {
            Ok(module)
        }
    }

    fn lower_subroutine(&mut self, node: Node<'_>) -> Option<Item> {
        self.reject_subroutine_modifiers(node)?;
        let name = self.subroutine_name(node)?;
        let body_node = first_named_field(node, "body")?;
        let (params, body_nodes) = self.subroutine_parts(node, body_node)?;
        self.declared = params
            .iter()
            .map(|parameter| parameter.name.as_str().to_owned())
            .collect();
        let kind = (name == "draw")
            .then_some(EntryPointKind::Frame)
            .or_else(|| canonical_entry_kind(&name));
        let previous_anchor = self.shader_annotation_anchor;
        self.shader_annotation_anchor = kind
            .as_ref()
            .filter(|kind| {
                matches!(
                    kind,
                    EntryPointKind::Vertex(_) | EntryPointKind::Fragment(_)
                )
            })
            .map(|_| node.start_byte());
        let mut body = self.lower_block_nodes(&body_nodes, body_node);
        self.shader_annotation_anchor = previous_anchor;
        self.declared.clear();
        let span = node_span(self.source, node);

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

    fn subroutine_parts<'tree>(
        &mut self,
        subroutine: Node<'tree>,
        body: Node<'tree>,
    ) -> Option<(Vec<Param>, Vec<Node<'tree>>)> {
        let mut statements = named_children(body);
        statements.retain(|node| !matches!(node.kind(), "comment" | "pod"));
        let mut params = Vec::new();
        if let Some(first) = statements.first().copied()
            && let Some(parameter_nodes) = self.parameter_declaration(first)
        {
            for parameter in parameter_nodes {
                if parameter.kind() != "scalar" {
                    self.unsupported(
                        parameter,
                        "Common Core parameters must be scalar Perl variables",
                        "destructure `@_` into scalar variables such as `my ($x, $y) = @_;`",
                    );
                    return None;
                }
                let name = variable_name(self.source, parameter);
                let ty = self
                    .annotations
                    .take(&name, subroutine.start_byte(), self.source);
                params.push(Param {
                    name: Symbol::new(name),
                    ty,
                    span: node_span(self.source, parameter),
                });
            }
            statements.remove(0);
        }
        Some((params, statements))
    }

    fn parameter_declaration<'tree>(&self, statement: Node<'tree>) -> Option<Vec<Node<'tree>>> {
        let expression = expression_statement_value(statement)?;
        if expression.kind() != "assignment_expression" {
            return None;
        }
        let left = first_named_field(expression, "left")?;
        let right = first_named_field(expression, "right")?;
        if left.kind() != "variable_declaration" || node_text(self.source, right).trim() != "@_" {
            return None;
        }
        let variables = named_field_children(left, "variables");
        (!variables.is_empty()).then_some(variables)
    }

    fn lower_constant(&mut self, statement: Node<'_>) -> Option<Item> {
        let expression = expression_statement_value(statement)?;
        let name = self.top_level_constant_name(statement)?;
        let right = first_named_field(expression, "right")?;
        Some(Item::Const(ConstDef {
            name: Symbol::new(name),
            ty: None,
            value: self.lower_expression(right)?,
            span: node_span(self.source, statement),
        }))
    }

    fn top_level_constant_name(&mut self, statement: Node<'_>) -> Option<String> {
        let Some(expression) = expression_statement_value(statement) else {
            self.unsupported(
                statement,
                "top-level Perl expressions are outside Common Core",
                "move this expression into `sub setup`",
            );
            return None;
        };
        if expression.kind() != "assignment_expression"
            || self.operator(expression).as_deref() != Some("=")
        {
            self.unsupported(
                statement,
                "top-level Perl expressions are outside Common Core",
                "declare a constant with `my $NAME = value;` or move the expression into `sub setup`",
            );
            return None;
        }
        let left = first_named_field(expression, "left")?;
        if left.kind() != "variable_declaration" {
            self.unsupported(
                statement,
                "top-level mutable state is outside Common Core",
                "declare an uppercase constant with `my $NAME = value;`",
            );
            return None;
        }
        let variable = first_named_field(left, "variable")?;
        let name = variable_name(self.source, variable);
        if variable.kind() != "scalar" || !is_upper_constant(&name) {
            self.unsupported(
                statement,
                "top-level declarations must be uppercase scalar constants",
                "rename it to an uppercase scalar such as `my $MESH = value;`",
            );
            return None;
        }
        Some(name)
    }

    fn lower_block_nodes(&mut self, nodes: &[Node<'_>], fallback: Node<'_>) -> Block {
        let statements = nodes
            .iter()
            .filter_map(|node| self.lower_statement(*node))
            .flatten()
            .collect();
        Block {
            statements,
            span: node_span(self.source, fallback),
        }
    }

    fn lower_nested_block(&mut self, block: Node<'_>) -> Block {
        let outer = self.declared.clone();
        let nodes = named_children(block);
        let lowered = self.lower_block_nodes(&nodes, block);
        self.declared = outer;
        lowered
    }

    fn lower_statement(&mut self, node: Node<'_>) -> Option<Vec<Stmt>> {
        match node.kind() {
            "comment" | "pod" => Some(Vec::new()),
            "expression_statement" => {
                let expression = expression_statement_value(node)?;
                match expression.kind() {
                    "assignment_expression" => self
                        .lower_assignment(expression)
                        .map(|statement| vec![statement]),
                    "return_expression" => {
                        let value = expression.named_child(0).and_then(|value| {
                            if value.kind() == "list_expression" && value.named_child_count() == 0 {
                                None
                            } else {
                                self.lower_expression(value)
                            }
                        });
                        Some(vec![Stmt::new(
                            StmtKind::Return(value),
                            node_span(self.source, expression),
                        )])
                    }
                    "postinc_expression" | "preinc_expression" => self
                        .lower_increment(expression)
                        .map(|statement| vec![statement]),
                    "loopex_expression" => self.lower_loop_control(expression),
                    _ => self.lower_expression(expression).map(|expression| {
                        let span = expression.span;
                        vec![Stmt::new(StmtKind::Expr(expression), span)]
                    }),
                }
            }
            "conditional_statement" => self
                .lower_conditional(node)
                .map(|statement| vec![statement]),
            "loop_statement" => self.lower_while(node).map(|statement| vec![statement]),
            "for_statement" => self.lower_for(node).map(|statement| vec![statement]),
            "cstyle_for_statement" => {
                self.unsupported_with_code(
                    node,
                    "E0202",
                    "general C-style Perl loops can reevaluate dynamic bounds",
                    "use `for my $i ($start .. $end)` with stable integer bounds",
                );
                None
            }
            _ => {
                self.unsupported(
                    node,
                    "this Perl statement is outside Common Core",
                    "rewrite it using declarations, assignment, if, while, range for, or return",
                );
                None
            }
        }
    }

    fn lower_assignment(&mut self, node: Node<'_>) -> Option<Stmt> {
        if self.operator(node).as_deref() != Some("=") {
            self.unsupported(
                node,
                "compound Perl assignments are outside Common Core",
                "rewrite the update as `$value = $value + amount`",
            );
            return None;
        }
        let left = first_named_field(node, "left")?;
        let right = first_named_field(node, "right")?;
        let span = node_span(self.source, node);
        if left.kind() == "variable_declaration" {
            let variable = first_named_field(left, "variable");
            if variable.is_none() || !named_field_children(left, "variables").is_empty() {
                self.unsupported(
                    left,
                    "destructuring is only supported for a subroutine's leading `@_` parameter declaration",
                    "declare one scalar, array, or hash local at a time",
                );
                return None;
            }
            let variable = variable?;
            let name = variable_name(self.source, variable);
            let init = match variable.kind() {
                "scalar" => self.lower_expression(right)?,
                "array" => self.lower_array_value(right)?,
                "hash" => self.lower_map_value(right)?,
                _ => {
                    self.unsupported(
                        variable,
                        "this Perl variable declaration is outside Common Core",
                        "use a scalar, array, or hash lexical declaration",
                    );
                    return None;
                }
            };
            let ty = self.annotations.take(&name, node.start_byte(), self.source);
            self.declared.insert(name.clone());
            return Some(Stmt::new(
                StmtKind::Let {
                    name: Symbol::new(name),
                    ty,
                    init,
                },
                span,
            ));
        }

        Some(Stmt::new(
            StmtKind::Assign {
                target: self.lower_place(left)?,
                value: self.lower_expression(right)?,
            },
            span,
        ))
    }

    fn lower_increment(&mut self, node: Node<'_>) -> Option<Stmt> {
        let operand = first_named_field(node, "operand").or_else(|| node.named_child(0))?;
        let place = self.lower_place(operand)?;
        let PlaceKind::Var(name) = &place.kind else {
            self.unsupported(
                node,
                "increment is limited to scalar locals",
                "rewrite the indexed or field update as an explicit assignment",
            );
            return None;
        };
        let span = node_span(self.source, node);
        let value = Expr::new(
            ExprKind::Binary {
                op: BinOp::Add,
                left: Box::new(Expr::new(ExprKind::Var(name.clone()), span)),
                right: Box::new(Expr::new(ExprKind::Literal(Literal::Int(1)), span)),
            },
            span,
        );
        Some(Stmt::new(
            StmtKind::Assign {
                target: place,
                value,
            },
            span,
        ))
    }

    fn lower_conditional(&mut self, node: Node<'_>) -> Option<Stmt> {
        let condition = first_named_field(node, "condition")?;
        let then_node = first_named_field(node, "block")?;
        let mut else_block = None;
        let mut cursor = node.walk();
        let alternatives = node
            .children(&mut cursor)
            .filter(|child| matches!(child.kind(), "elsif" | "else"))
            .collect::<Vec<_>>();
        for alternative in alternatives.into_iter().rev() {
            let block_node = first_named_field(alternative, "block").or_else(|| {
                named_children(alternative)
                    .into_iter()
                    .find(|child| child.kind() == "block")
            });
            let Some(block_node) = block_node else {
                continue;
            };
            if alternative.kind() == "else" {
                else_block = Some(self.lower_nested_block(block_node));
            } else {
                let nested_condition = first_named_field(alternative, "condition")?;
                let span = node_span(self.source, alternative);
                let nested = Stmt::new(
                    StmtKind::If {
                        condition: self.lower_expression(nested_condition)?,
                        then_block: self.lower_nested_block(block_node),
                        else_block,
                    },
                    span,
                );
                else_block = Some(Block {
                    statements: vec![nested],
                    span,
                });
            }
        }
        let span = node_span(self.source, node);
        Some(Stmt::new(
            StmtKind::If {
                condition: self.lower_expression(condition)?,
                then_block: self.lower_nested_block(then_node),
                else_block,
            },
            span,
        ))
    }

    fn lower_while(&mut self, node: Node<'_>) -> Option<Stmt> {
        if !node_text(self.source, node)
            .trim_start()
            .starts_with("while")
        {
            self.unsupported_with_code(
                node,
                "E0202",
                "`until` and post-test loops are outside Common Core",
                "rewrite the loop as `while (explicit_boolean_condition) { ... }`",
            );
            return None;
        }
        let condition = first_named_field(node, "condition")?;
        let body_node = first_named_field(node, "block")?;
        self.loop_depth += 1;
        let body = self.lower_nested_block(body_node);
        self.loop_depth -= 1;
        let span = node_span(self.source, node);
        Some(Stmt::new(
            StmtKind::While {
                condition: self.lower_expression(condition)?,
                body,
            },
            span,
        ))
    }

    fn lower_for(&mut self, node: Node<'_>) -> Option<Stmt> {
        let variable = first_named_field(node, "variable")?;
        let list = first_named_field(node, "list")?;
        let body_node = first_named_field(node, "block")?;
        if variable.kind() != "scalar"
            || list.kind() != "binary_expression"
            || self.operator(list).as_deref() != Some("..")
        {
            self.unsupported_with_code(
                node,
                "E0202",
                "Perl loops are limited to one inclusive ascending range",
                "use `for my $i ($start .. $end) { ... }`",
            );
            return None;
        }
        let start = first_named_field(list, "left")?;
        let end = first_named_field(list, "right")?;
        let name = variable_name(self.source, variable);
        let outer = self.declared.clone();
        self.declared.insert(name.clone());
        self.loop_depth += 1;
        let body = self.lower_block_nodes(&named_children(body_node), body_node);
        self.loop_depth -= 1;
        self.declared = outer;
        let span = node_span(self.source, node);
        Some(Stmt::new(
            StmtKind::For {
                variable: Symbol::new(name),
                range: RangeExpr {
                    start: self.lower_expression(start)?,
                    end: self.lower_expression(end)?,
                    inclusive: true,
                    span: node_span(self.source, list),
                },
                body,
            },
            span,
        ))
    }

    fn lower_loop_control(&mut self, node: Node<'_>) -> Option<Vec<Stmt>> {
        if self.loop_depth == 0 {
            self.unsupported_with_code(
                node,
                "E0202",
                "loop control is only valid inside a Common Core loop",
                "remove it or move it into a while or range-for body",
            );
            return None;
        }
        let kind = match node_text(self.source, node).trim().trim_end_matches(';') {
            "last" => StmtKind::Break,
            "next" => StmtKind::Continue,
            _ => {
                self.unsupported_with_code(
                    node,
                    "E0202",
                    "only `last` and `next` are portable loop control",
                    "replace it with `last` or `next`",
                );
                return None;
            }
        };
        Some(vec![Stmt::new(kind, node_span(self.source, node))])
    }

    fn lower_expression(&mut self, node: Node<'_>) -> Option<Expr> {
        let span = node_span(self.source, node);
        let kind = match node.kind() {
            "number" => self.lower_number(node)?,
            "boolean" => {
                ExprKind::Literal(Literal::Bool(node_text(self.source, node).trim() == "true"))
            }
            "undef_expression" => ExprKind::Literal(Literal::None),
            "interpolated_string_literal" | "string_literal" => {
                ExprKind::Literal(Literal::Str(self.lower_string(node)?))
            }
            "scalar" | "array" | "hash" | "container_variable" => {
                let name = variable_name(self.source, node);
                if self.declared.contains(&name) || self.constant_names.contains(&name) {
                    ExprKind::Var(Symbol::new(name))
                } else if let Some(anchor) = self.shader_annotation_anchor {
                    if let Some(declared) = automatic_uniform_type(&name, span)
                        .or_else(|| self.annotations.take(&name, anchor, self.source))
                    {
                        ExprKind::Uniform {
                            name: Symbol::new(name),
                            declared,
                        }
                    } else {
                        self.unsupported(
                            node,
                            "this Perl scalar is not declared in the current Common Core scope",
                            "declare it with `my $name = value;` before use",
                        );
                        return None;
                    }
                } else {
                    self.unsupported(
                        node,
                        "this Perl variable is not declared in the current Common Core scope",
                        "declare it with `my $name = value;` before use",
                    );
                    return None;
                }
            }
            "array_element_expression" | "hash_element_expression" => {
                return self.lower_index_or_field(node);
            }
            "function_call_expression" | "ambiguous_function_call_expression" => {
                return self.lower_call(node);
            }
            "method_call_expression" => return self.lower_method_call(node),
            "binary_expression"
            | "equality_expression"
            | "relational_expression"
            | "lowprec_logical_expression" => return self.lower_binary(node),
            "logical_not_expression" => {
                let operand = first_named_field(node, "operand").or_else(|| node.named_child(0))?;
                ExprKind::Unary {
                    op: UnOp::Not,
                    operand: Box::new(self.lower_expression(operand)?),
                }
            }
            "unary_expression" => {
                if node_text(self.source, node).trim() == "-2147483648" {
                    ExprKind::Literal(Literal::Int(i32::MIN))
                } else {
                    let operand =
                        first_named_field(node, "operand").or_else(|| node.named_child(0))?;
                    if self.operator(node).as_deref() != Some("-") {
                        self.unsupported(
                            node,
                            "this unary Perl operator is outside Common Core",
                            "use numeric negation `-value` or boolean negation `!condition`",
                        );
                        return None;
                    }
                    ExprKind::Unary {
                        op: UnOp::Neg,
                        operand: Box::new(self.lower_expression(operand)?),
                    }
                }
            }
            "list_expression" | "anonymous_array_expression" => {
                return self.lower_array_value(node);
            }
            "anonymous_hash_expression" => return self.lower_map_value(node),
            "autoquoted_bareword" | "bareword" => {
                ExprKind::Literal(Literal::Str(node_text(self.source, node).trim().to_owned()))
            }
            _ => {
                self.unsupported(
                    node,
                    "this Perl expression is outside Common Core",
                    "rewrite it using literals, lexical variables, operators, or direct calls",
                );
                return None;
            }
        };
        Some(Expr::new(kind, span))
    }

    fn lower_number(&mut self, node: Node<'_>) -> Option<ExprKind> {
        let raw = node_text(self.source, node).replace('_', "");
        if raw.contains(['.', 'e', 'E']) {
            let Ok(value) = raw.parse::<f64>() else {
                self.unsupported(
                    node,
                    "this Perl numeric literal is outside Common Core",
                    "use a finite decimal integer or float literal",
                );
                return None;
            };
            if !value.is_finite() {
                self.unsupported(
                    node,
                    "non-finite floats are outside Common Core",
                    "use a finite decimal float literal",
                );
                return None;
            }
            return Some(ExprKind::Literal(Literal::Float(value)));
        }
        let Ok(value) = raw.parse::<i64>() else {
            self.unsupported(
                node,
                "non-decimal Perl integers are outside Common Core",
                "use a decimal 32-bit integer literal",
            );
            return None;
        };
        let Ok(value) = i32::try_from(value) else {
            self.unsupported(
                node,
                "integer literal is outside the signed 32-bit Common Core range",
                "use a value between -2147483648 and 2147483647",
            );
            return None;
        };
        Some(ExprKind::Literal(Literal::Int(value)))
    }

    fn lower_string(&mut self, node: Node<'_>) -> Option<String> {
        if named_children(node)
            .iter()
            .any(|child| !matches!(child.kind(), "string_content" | "escape_sequence"))
        {
            self.unsupported(
                node,
                "interpolated Perl strings are outside Common Core",
                "build the value explicitly with the `.` string operator",
            );
            return None;
        }
        let raw = node_text(self.source, node);
        let Some(quote) = raw.chars().next() else {
            return Some(String::new());
        };
        if !matches!(quote, '\'' | '"') || !raw.ends_with(quote) {
            self.unsupported(
                node,
                "this Perl string form is outside Common Core",
                "use a single-quoted or double-quoted UTF-8 string literal",
            );
            return None;
        }
        let inner = &raw[quote.len_utf8()..raw.len() - quote.len_utf8()];
        decode_string(inner, quote).or_else(|| {
            self.unsupported(
                node,
                "this Perl string escape is outside Common Core",
                "use UTF-8 text and the escapes `\\\\`, `\\n`, `\\r`, `\\t`, or an escaped quote",
            );
            None
        })
    }

    fn lower_binary(&mut self, node: Node<'_>) -> Option<Expr> {
        let left = first_named_field(node, "left")?;
        let right = first_named_field(node, "right")?;
        let operator = self.operator(node)?;
        let op = match operator.as_str() {
            "+" => BinOp::Add,
            "-" => BinOp::Sub,
            "*" => BinOp::Mul,
            "/" => BinOp::DivFloat,
            "%" => BinOp::RemTrunc,
            "." => BinOp::StrConcat,
            "==" => BinOp::Eq,
            "!=" => BinOp::NotEq,
            "<" => BinOp::Less,
            "<=" => BinOp::LessEq,
            ">" => BinOp::Greater,
            ">=" => BinOp::GreaterEq,
            "&&" => BinOp::And,
            "||" => BinOp::Or,
            _ => {
                self.unsupported(
                    node,
                    "this Perl binary operator is outside Common Core",
                    "use arithmetic, `.`, numeric comparisons, `&&`, or `||`",
                );
                return None;
            }
        };
        let span = node_span(self.source, node);
        Some(Expr::new(
            ExprKind::Binary {
                op,
                left: Box::new(self.lower_expression(left)?),
                right: Box::new(self.lower_expression(right)?),
            },
            span,
        ))
    }

    fn lower_call(&mut self, node: Node<'_>) -> Option<Expr> {
        let function = first_named_field(node, "function")
            .or_else(|| named_children(node).into_iter().next())?;
        let name = node_text(self.source, function).trim();
        let args = self.lower_arguments(node)?;
        let span = node_span(self.source, node);
        if name == "defined" && args.len() == 1 {
            return Some(Expr::new(
                ExprKind::Unary {
                    op: UnOp::Not,
                    operand: Box::new(Expr::new(
                        ExprKind::NilCheck(Box::new(args.into_iter().next()?)),
                        span,
                    )),
                },
                span,
            ));
        }
        if let Some(size) = vector_constructor_size(name) {
            return Some(Expr::new(ExprKind::Vector { size, args }, span));
        }
        if let Some(class) = &self.current_class
            && self
                .class_methods
                .get(class)
                .is_some_and(|methods| methods.contains(name))
        {
            let mut args_with_self = Vec::with_capacity(args.len() + 1);
            args_with_self.push(Expr::new(ExprKind::Var(Symbol::new("self")), span));
            args_with_self.extend(args);
            return Some(Expr::new(
                ExprKind::Call {
                    callee: Callee::Method(Symbol::new(name)),
                    args: args_with_self,
                },
                span,
            ));
        }
        let callee = if let Some(builtin) = self.context.resolve_builtin(name) {
            Callee::Builtin(builtin)
        } else if self.function_names.contains(name) {
            Callee::User(Symbol::new(name))
        } else {
            self.unsupported(
                node,
                "this Perl call does not resolve to a Common Core function",
                "call a declared subroutine or a canonical PolyGL builtin with parentheses",
            );
            return None;
        };
        Some(Expr::new(ExprKind::Call { callee, args }, span))
    }

    fn lower_method_call(&mut self, node: Node<'_>) -> Option<Expr> {
        let receiver = first_named_field(node, "invocant")?;
        let method = first_named_field(node, "method")?;
        let method_name = node_text(self.source, method).trim();
        let mut args = self.lower_arguments(node)?;
        let span = node_span(self.source, node);
        if receiver.kind() == "bareword" && method_name == "new" {
            let class_name = node_text(self.source, receiver).trim();
            if !self.class_names.contains(class_name) {
                self.unsupported_with_code(
                    node,
                    "E0203",
                    "construction is limited to packages declared in this source file",
                    "declare the struct-like package before calling `Name->new(...)`",
                );
                return None;
            }
            return Some(Expr::new(
                ExprKind::Call {
                    callee: Callee::User(Symbol::new(constructor_function_name(class_name))),
                    args,
                },
                span,
            ));
        }
        if !self
            .class_methods
            .values()
            .any(|methods| methods.contains(method_name))
        {
            self.unsupported_with_code(
                node,
                "E0203",
                "dynamic or undeclared Perl method dispatch is outside Common Core",
                "declare the method in a struct-like package or use a plain subroutine",
            );
            return None;
        }
        args.insert(0, self.lower_expression(receiver)?);
        Some(Expr::new(
            ExprKind::Call {
                callee: Callee::Method(Symbol::new(method_name)),
                args,
            },
            span,
        ))
    }

    fn lower_arguments(&mut self, call: Node<'_>) -> Option<Vec<Expr>> {
        let fields = named_field_children(call, "arguments");
        let nodes = if fields.len() == 1 && fields[0].kind() == "list_expression" {
            named_children(fields[0])
        } else {
            fields
        };
        nodes
            .into_iter()
            .map(|argument| self.lower_expression(argument))
            .collect()
    }

    fn lower_array_value(&mut self, node: Node<'_>) -> Option<Expr> {
        let content = collection_content(node);
        let items = named_children(content)
            .into_iter()
            .map(|item| self.lower_expression(item))
            .collect::<Option<Vec<_>>>()?;
        Some(Expr::new(
            ExprKind::Array(items),
            node_span(self.source, node),
        ))
    }

    fn lower_map_value(&mut self, node: Node<'_>) -> Option<Expr> {
        let content = collection_content(node);
        let nodes = named_children(content);
        if !nodes.len().is_multiple_of(2) {
            self.unsupported(
                node,
                "a Common Core Perl hash requires explicit key/value pairs",
                "write pairs such as `(\"key\" => $value)`",
            );
            return None;
        }
        let mut entries = Vec::with_capacity(nodes.len() / 2);
        for pair in nodes.chunks_exact(2) {
            let key = self.lower_map_key(pair[0])?;
            let value = self.lower_expression(pair[1])?;
            entries.push(MapEntry {
                key,
                value,
                span: self
                    .source
                    .span(pair[0].start_byte(), pair[1].end_byte())
                    .expect("map pair range is valid"),
            });
        }
        Some(Expr::new(
            ExprKind::Map(entries),
            node_span(self.source, node),
        ))
    }

    fn lower_map_key(&mut self, node: Node<'_>) -> Option<Expr> {
        if matches!(node.kind(), "autoquoted_bareword" | "bareword") {
            return Some(Expr::new(
                ExprKind::Literal(Literal::Str(node_text(self.source, node).trim().to_owned())),
                node_span(self.source, node),
            ));
        }
        self.lower_expression(node)
    }

    fn lower_index_or_field(&mut self, node: Node<'_>) -> Option<Expr> {
        let base = first_named_field(node, "array")
            .or_else(|| first_named_field(node, "hash"))
            .or_else(|| {
                named_children(node)
                    .into_iter()
                    .find(|child| matches!(child.kind(), "scalar" | "container_variable"))
            })?;
        let key_field = if node.kind() == "array_element_expression" {
            "index"
        } else {
            "key"
        };
        let key = first_named_field(node, key_field)?;
        let base_name = variable_name(self.source, base);
        let field_name = literal_key(self.source, key);
        let span = node_span(self.source, node);
        if let Some(field_name) = field_name
            && (base_name == "self" || (base_name == "event" && is_event_field(&field_name)))
        {
            return Some(Expr::new(
                ExprKind::Field {
                    base: Box::new(Expr::new(ExprKind::Var(Symbol::new(base_name)), span)),
                    field: Symbol::new(field_name),
                },
                span,
            ));
        }
        Some(Expr::new(
            ExprKind::Index {
                base: Box::new(self.lower_expression(base)?),
                index: Box::new(self.lower_expression(key)?),
            },
            span,
        ))
    }

    fn lower_place(&mut self, node: Node<'_>) -> Option<Place> {
        let span = node_span(self.source, node);
        match node.kind() {
            "scalar" => {
                let name = variable_name(self.source, node);
                if !self.declared.contains(&name) {
                    self.unsupported(
                        node,
                        "assignment target is not declared in this Common Core scope",
                        "declare it first with `my $name = value;`",
                    );
                    return None;
                }
                Some(Place {
                    kind: PlaceKind::Var(Symbol::new(name)),
                    span,
                })
            }
            "array_element_expression" | "hash_element_expression" => {
                let expression = self.lower_index_or_field(node)?;
                let kind = match expression.kind {
                    ExprKind::Index { base, index } => PlaceKind::Index {
                        base: *base,
                        index: *index,
                    },
                    ExprKind::Field { base, field } => PlaceKind::Field { base: *base, field },
                    _ => unreachable!("index lowering only returns index or field"),
                };
                Some(Place { kind, span })
            }
            _ => {
                self.unsupported(
                    node,
                    "this Perl assignment target is outside Common Core",
                    "assign to a declared scalar, indexed array/map element, or fixed field",
                );
                None
            }
        }
    }

    fn lower_class(&mut self, package: &PackageSection<'_>) -> Option<Vec<Item>> {
        let mut constructor = None;
        let mut methods = Vec::new();
        for subroutine in &package.subroutines {
            let name = self.subroutine_name(*subroutine)?;
            if name == "new" {
                if constructor.replace(*subroutine).is_some() {
                    self.unsupported_with_code(
                        *subroutine,
                        "E0203",
                        "a Common Core package may define only one constructor",
                        "merge construction into one `sub new`",
                    );
                }
            } else {
                methods.push(*subroutine);
            }
        }
        let span = self.package_span(package);
        let (fields, constructor) = self.lower_constructor(&package.name, constructor, span)?;
        let methods = methods
            .into_iter()
            .filter_map(|method| self.lower_instance_method(&package.name, method))
            .collect();
        Some(vec![
            Item::Struct(StructDef {
                name: Symbol::new(package.name.clone()),
                fields,
                methods,
                span,
            }),
            Item::Function(constructor),
        ])
    }

    fn lower_constructor(
        &mut self,
        class_name: &str,
        constructor: Option<Node<'_>>,
        class_span: Span,
    ) -> Option<(Vec<FieldDef>, Function)> {
        let Some(constructor) = constructor else {
            return Some((
                Vec::new(),
                Function {
                    name: Symbol::new(constructor_function_name(class_name)),
                    params: Vec::new(),
                    return_type: Some(TypeExpr::new(
                        TypeKind::Struct(Symbol::new(class_name)),
                        class_span,
                    )),
                    body: Block {
                        statements: vec![struct_return(class_name, Vec::new(), class_span)],
                        span: class_span,
                    },
                    span: class_span,
                    domain: DomainHint::Auto,
                },
            ));
        };
        self.reject_subroutine_modifiers(constructor)?;
        let body_node = first_named_field(constructor, "body")?;
        let (mut parameters, statements) = self.subroutine_parts(constructor, body_node)?;
        if parameters
            .first()
            .is_none_or(|parameter| parameter.name.as_str() != "class")
        {
            self.unsupported_with_code(
                constructor,
                "E0203",
                "a Perl constructor must destructure `$class` first",
                "start `sub new` with `my ($class, $arg) = @_;`",
            );
            return None;
        }
        parameters.remove(0);
        self.declared = parameters
            .iter()
            .map(|parameter| parameter.name.as_str().to_owned())
            .collect();
        if statements.len() != 2 {
            self.unsupported_with_code(
                constructor,
                "E0203",
                "constructors may only create a fixed hash and bless it",
                "create `my $self = { field => $value };` then `return bless $self, $class;`",
            );
            self.declared.clear();
            return None;
        }
        let Some(hash) = constructor_hash(self.source, statements[0]) else {
            self.unsupported_with_code(
                statements[0],
                "E0203",
                "constructor fields must come from one `$self` hash",
                "write `my $self = { field => $value };`",
            );
            self.declared.clear();
            return None;
        };
        if !node_text(self.source, statements[1])
            .trim()
            .starts_with("return bless ")
        {
            self.unsupported_with_code(
                statements[1],
                "E0203",
                "constructor must return `bless $self, $class`",
                "finish with `return bless $self, $class;`",
            );
            self.declared.clear();
            return None;
        }
        let content = collection_content(hash);
        let nodes = named_children(content);
        if !nodes.len().is_multiple_of(2) {
            self.unsupported_with_code(
                hash,
                "E0203",
                "constructor fields require fixed key/value pairs",
                "give every field one value in the `$self` hash",
            );
            self.declared.clear();
            return None;
        }
        let mut fields = Vec::new();
        let mut values = Vec::new();
        let mut seen = HashSet::new();
        for pair in nodes.chunks_exact(2) {
            let Some(field_name) = literal_key(self.source, pair[0]) else {
                self.unsupported_with_code(
                    pair[0],
                    "E0203",
                    "constructor field names must be static strings or barewords",
                    "use a fixed field name such as `x => $x`",
                );
                continue;
            };
            if !seen.insert(field_name.clone()) {
                self.unsupported_with_code(
                    pair[0],
                    "E0203",
                    "constructor fields must be unique",
                    "keep one initializer for each fixed field",
                );
                continue;
            }
            let value = self.lower_expression(pair[1])?;
            let ty = self
                .annotations
                .take(&field_name, pair[0].start_byte(), self.source)
                .or_else(|| field_type_from_parameter(&value, &parameters));
            fields.push(FieldDef {
                name: Symbol::new(field_name.clone()),
                ty,
                span: node_span(self.source, pair[0]),
            });
            values.push(FieldInit {
                name: Symbol::new(field_name),
                value,
                span: self
                    .source
                    .span(pair[0].start_byte(), pair[1].end_byte())
                    .expect("field initializer range is valid"),
            });
        }
        self.declared.clear();
        let constructor_span = node_span(self.source, constructor);
        Some((
            fields,
            Function {
                name: Symbol::new(constructor_function_name(class_name)),
                params: parameters,
                return_type: Some(TypeExpr::new(
                    TypeKind::Struct(Symbol::new(class_name)),
                    constructor_span,
                )),
                body: Block {
                    statements: vec![struct_return(class_name, values, constructor_span)],
                    span: node_span(self.source, body_node),
                },
                span: constructor_span,
                domain: DomainHint::Auto,
            },
        ))
    }

    fn lower_instance_method(&mut self, class_name: &str, method: Node<'_>) -> Option<Function> {
        self.reject_subroutine_modifiers(method)?;
        let name = self.subroutine_name(method)?;
        let body_node = first_named_field(method, "body")?;
        let (mut params, statements) = self.subroutine_parts(method, body_node)?;
        if params
            .first()
            .is_none_or(|parameter| parameter.name.as_str() != "self")
        {
            self.unsupported_with_code(
                method,
                "E0203",
                "instance methods must destructure `$self` first",
                "start the method with `my ($self, $arg) = @_;`",
            );
            return None;
        }
        params.remove(0);
        let span = node_span(self.source, method);
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
        let mut body = self.lower_block_nodes(&statements, body_node);
        self.current_class = None;
        self.declared.clear();
        ensure_implicit_return(&mut body);
        Some(Function {
            name: Symbol::new(name),
            params,
            return_type: None,
            body,
            span,
            domain: DomainHint::Auto,
        })
    }

    fn reject_subroutine_modifiers(&mut self, node: Node<'_>) -> Option<()> {
        if named_children(node)
            .iter()
            .any(|child| matches!(child.kind(), "prototype" | "signature" | "attrlist"))
        {
            self.unsupported(
                node,
                "Perl signatures, prototypes, and attributes are outside Common Core",
                "use a plain subroutine and destructure required scalar parameters from `@_`",
            );
            return None;
        }
        Some(())
    }

    fn package_span(&self, package: &PackageSection<'_>) -> Span {
        let end = package
            .subroutines
            .last()
            .map_or(package.header.end_byte(), Node::end_byte);
        self.source
            .span(package.header.start_byte(), end)
            .expect("package source range is valid")
    }

    fn subroutine_name(&mut self, node: Node<'_>) -> Option<String> {
        let Some(name) = first_named_field(node, "name") else {
            self.unsupported(
                node,
                "anonymous Perl subroutines are outside Common Core",
                "give the subroutine a static portable name",
            );
            return None;
        };
        Some(node_text(self.source, name).trim().to_owned())
    }

    fn operator(&self, node: Node<'_>) -> Option<String> {
        let mut cursor = node.walk();
        node.children_by_field_name("operator", &mut cursor)
            .map(|operator| node_text(self.source, operator).trim())
            .find(|operator| {
                matches!(
                    *operator,
                    "=" | "+"
                        | "-"
                        | "*"
                        | "/"
                        | "%"
                        | "."
                        | "=="
                        | "!="
                        | "<"
                        | "<="
                        | ">"
                        | ">="
                        | "&&"
                        | "||"
                        | ".."
                        | "!"
                )
            })
            .map(str::to_owned)
    }

    fn unsupported(&mut self, node: Node<'_>, message: &str, suggestion: &str) {
        self.unsupported_with_code(node, "E0200", message, suggestion);
    }

    fn unsupported_with_code(
        &mut self,
        node: Node<'_>,
        code: &str,
        message: &str,
        suggestion: &str,
    ) {
        let span = node_span(self.source, node);
        self.diagnostics.push(
            Diagnostic::new(Severity::Error, code, message, span)
                .with_suggestion(Suggestion::rewrite(span, suggestion)),
        );
    }
}

fn expression_statement_value(statement: Node<'_>) -> Option<Node<'_>> {
    (statement.kind() == "expression_statement")
        .then(|| statement.named_child(0))
        .flatten()
}

fn variable_name(source: &SourceFile, node: Node<'_>) -> String {
    node_text(source, node)
        .trim()
        .trim_start_matches(['$', '@', '%'])
        .trim_start_matches('#')
        .to_owned()
}

fn collection_content(node: Node<'_>) -> Node<'_> {
    if matches!(
        node.kind(),
        "anonymous_array_expression" | "anonymous_hash_expression"
    ) && let Some(content) = named_children(node)
        .into_iter()
        .find(|child| child.kind() == "list_expression")
    {
        return content;
    }
    node
}

fn constructor_hash<'tree>(source: &SourceFile, statement: Node<'tree>) -> Option<Node<'tree>> {
    let expression = expression_statement_value(statement)?;
    if expression.kind() != "assignment_expression" {
        return None;
    }
    let left = first_named_field(expression, "left")?;
    let right = first_named_field(expression, "right")?;
    let variable = first_named_field(left, "variable")?;
    (left.kind() == "variable_declaration"
        && variable.kind() == "scalar"
        && variable_name(source, variable) == "self"
        && right.kind() == "anonymous_hash_expression")
        .then_some(right)
}

fn literal_key(source: &SourceFile, node: Node<'_>) -> Option<String> {
    if matches!(node.kind(), "autoquoted_bareword" | "bareword") {
        return Some(node_text(source, node).trim().to_owned());
    }
    if matches!(
        node.kind(),
        "interpolated_string_literal" | "string_literal"
    ) {
        let raw = node_text(source, node);
        let quote = raw.chars().next()?;
        if matches!(quote, '\'' | '"') && raw.ends_with(quote) {
            return decode_string(&raw[1..raw.len() - 1], quote);
        }
    }
    None
}

fn decode_string(raw: &str, quote: char) -> Option<String> {
    let mut result = String::new();
    let mut characters = raw.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            result.push(character);
            continue;
        }
        let escaped = characters.next()?;
        let decoded = match escaped {
            '\\' => '\\',
            '\'' if quote == '\'' => '\'',
            '"' if quote == '"' => '"',
            'n' if quote == '"' => '\n',
            'r' if quote == '"' => '\r',
            't' if quote == '"' => '\t',
            _ => return None,
        };
        result.push(decoded);
    }
    Some(result)
}

fn allowed_pragma(source: &str) -> bool {
    let source = source.trim();
    source.starts_with("use strict") || source.starts_with("use warnings")
}

fn is_event_field(name: &str) -> bool {
    matches!(name, "kind" | "x" | "y" | "key")
}

fn is_upper_constant(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_uppercase())
        && characters.all(|character| {
            character == '_' || character.is_ascii_uppercase() || character.is_ascii_digit()
        })
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

fn struct_return(class_name: &str, fields: Vec<FieldInit>, span: Span) -> Stmt {
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

fn ensure_implicit_return(body: &mut Block) {
    if body.statements.is_empty() {
        body.statements
            .push(Stmt::new(StmtKind::Return(None), body.span));
        return;
    }
    let index = body.statements.len() - 1;
    let span = body.statements[index].span;
    let appended = match &mut body.statements[index].kind {
        StmtKind::Expr(expression) => {
            body.statements[index].kind = StmtKind::Return(Some(expression.clone()));
            None
        }
        StmtKind::Let { name, .. } => Some(StmtKind::Return(Some(Expr::new(
            ExprKind::Var(name.clone()),
            span,
        )))),
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
                    statements: vec![Stmt::new(
                        StmtKind::Return(Some(Expr::new(ExprKind::Literal(Literal::None), span))),
                        span,
                    )],
                    span,
                });
            }
            None
        }
        StmtKind::Return(_) => None,
        StmtKind::While { .. } | StmtKind::For { .. } | StmtKind::Break | StmtKind::Continue => {
            Some(StmtKind::Return(None))
        }
    };
    if let Some(kind) = appended {
        body.statements.push(Stmt::new(kind, span));
    }
}
