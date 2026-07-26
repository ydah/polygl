use std::collections::{HashMap, HashSet};

use polygl_adapter_api::LowerCtx;
use polygl_hir::{Block, Module};
use polygl_span::{Diagnostic, Diagnostics, Severity, SourceFile, Span, Suggestion};
use ruby_prism::{Location, Node, ProgramNode, StatementsNode};

use crate::annotation::Annotations;

pub(crate) struct Lowerer<'source, 'context, 'resolver> {
    pub(crate) source: &'source SourceFile,
    pub(crate) context: &'context mut LowerCtx<'resolver>,
    pub(crate) diagnostics: Diagnostics,
    pub(crate) declared: HashSet<String>,
    pub(crate) annotations: Annotations,
    pub(crate) loop_depth: usize,
    pub(crate) temporary_index: usize,
    pub(crate) class_names: HashSet<String>,
    pub(crate) field_names: HashSet<String>,
    pub(crate) class_methods: HashMap<String, HashSet<String>>,
    pub(crate) current_class: Option<String>,
    pub(crate) function_names: HashSet<String>,
    pub(crate) shader_annotation_anchor: Option<usize>,
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
            declared: HashSet::new(),
            annotations,
            loop_depth: 0,
            temporary_index: 0,
            class_names: HashSet::new(),
            field_names: HashSet::new(),
            class_methods: HashMap::new(),
            current_class: None,
            function_names: HashSet::new(),
            shader_annotation_anchor: None,
        }
    }

    pub(crate) fn lower_program(
        mut self,
        program: &ProgramNode<'_>,
    ) -> Result<Module, Diagnostics> {
        let mut items = Vec::new();
        for node in program.statements().body().iter() {
            if let Some(class) = node.as_class_node() {
                self.register_class_shape(&class);
            } else if let Some(definition) = node.as_def_node() {
                self.function_names
                    .insert(self.name(definition.name().as_slice()));
            }
        }
        for node in program.statements().body().iter() {
            if let Some(class) = node.as_class_node()
                && let Some(class_items) = self.lower_class(&class)
            {
                items.extend(class_items);
            }
        }
        for node in program.statements().body().iter() {
            if let Some(definition) = node.as_def_node() {
                if let Some(item) = self.lower_def(&definition) {
                    items.push(item);
                }
            } else if node
                .as_call_node()
                .is_some_and(|call| self.name(call.name().as_slice()) == "define_method")
            {
                self.unsupported(
                    &node,
                    "`define_method` is outside Common Core",
                    "use a regular `def name` declaration",
                );
            } else if node.as_class_node().is_some() {
                continue;
            } else if node.as_module_node().is_some() {
                self.unsupported(
                    &node,
                    "Ruby modules are outside Common Core",
                    "replace the module with a plain function or a struct-like class",
                );
            } else if node.as_call_node().is_some_and(|call| {
                matches!(
                    self.name(call.name().as_slice()).as_str(),
                    "require" | "require_relative" | "load"
                )
            }) {
                self.unsupported(
                    &node,
                    "loading external Ruby files violates the single-file Common Core",
                    "copy the required Common Core functions into this source file",
                );
            } else {
                self.unsupported(
                    &node,
                    "Ruby top-level executable statements are outside Common Core",
                    "move this statement into `def setup`",
                );
            }
        }
        let module = Module {
            items,
            span: self.span(program.location()),
        };
        self.annotations.report_unused(&mut self.diagnostics);
        if self.diagnostics.has_errors() {
            Err(self.diagnostics)
        } else {
            Ok(module)
        }
    }

    pub(crate) fn lower_body(&mut self, body: Option<Node<'_>>, fallback: Span) -> Block {
        let Some(body) = body else {
            return Block {
                statements: Vec::new(),
                span: fallback,
            };
        };
        if let Some(statements) = body.as_statements_node() {
            self.lower_statements(Some(statements), fallback)
        } else {
            let span = self.span(body.location());
            let statements = self.lower_statement(&body).unwrap_or_default();
            Block { statements, span }
        }
    }

    pub(crate) fn lower_statements(
        &mut self,
        statements: Option<StatementsNode<'_>>,
        fallback: Span,
    ) -> Block {
        let Some(statements) = statements else {
            return Block {
                statements: Vec::new(),
                span: fallback,
            };
        };
        let body = statements.body();
        let lowered = body
            .iter()
            .filter_map(|node| self.lower_statement(&node))
            .flatten()
            .collect();
        Block {
            statements: lowered,
            span: self.span(statements.location()),
        }
    }

    pub(crate) fn lower_nested_statements(
        &mut self,
        statements: Option<StatementsNode<'_>>,
        fallback: Span,
    ) -> Block {
        let outer = self.declared.clone();
        let block = self.lower_statements(statements, fallback);
        self.declared = outer;
        block
    }

    pub(crate) fn unsupported(&mut self, node: &Node<'_>, message: &str, suggestion: &str) {
        self.unsupported_with_code(node, "E0200", message, suggestion);
    }

    pub(crate) fn unsupported_with_code(
        &mut self,
        node: &Node<'_>,
        code: &str,
        message: &str,
        suggestion: &str,
    ) {
        let span = self.span(node.location());
        self.diagnostics.push(
            Diagnostic::new(Severity::Error, code, message, span)
                .with_suggestion(Suggestion::rewrite(span, suggestion)),
        );
    }

    pub(crate) fn temporary(&mut self, purpose: &str) -> String {
        loop {
            let index = self.temporary_index;
            self.temporary_index += 1;
            let name = format!("__pgl_{purpose}_{index}");
            if !self.declared.contains(&name) {
                return name;
            }
        }
    }

    pub(crate) fn span(&self, location: Location<'_>) -> Span {
        self.source
            .span(location.start_offset(), location.end_offset())
            .expect("Prism nodes must use source byte boundaries")
    }

    pub(crate) fn name(&self, bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).into_owned()
    }

    pub(crate) fn annotation_for(
        &mut self,
        name: &str,
        location: Location<'_>,
    ) -> Option<polygl_hir::TypeExpr> {
        self.annotations
            .take(name, location.start_offset(), self.source)
    }

    pub(crate) fn parameter_annotation_for(
        &mut self,
        name: &str,
        definition: Location<'_>,
    ) -> Option<polygl_hir::TypeExpr> {
        self.annotations
            .take_parameter(name, definition.start_offset(), self.source)
    }
}
