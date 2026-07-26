use std::collections::{HashMap, HashSet};

use mago_span::{HasSpan, Span as MagoSpan};
use mago_syntax::cst::{Block as PhpBlock, DirectVariable, Program, Statement};
use mago_syntax::walker::Walker;
use polygl_adapter_api::LowerCtx;
use polygl_hir::{Block, Module};
use polygl_span::{Diagnostic, Diagnostics, Severity, SourceFile, Span, Suggestion};

use crate::annotation::Annotations;

pub(crate) struct Lowerer<'source, 'context, 'resolver> {
    pub(crate) source: &'source SourceFile,
    pub(crate) context: &'context mut LowerCtx<'resolver>,
    pub(crate) diagnostics: Diagnostics,
    pub(crate) declared: HashSet<String>,
    pub(crate) annotations: Annotations,
    pub(crate) loop_depth: usize,
    pub(crate) temporary_index: usize,
    pub(crate) source_names: HashSet<String>,
    pub(crate) constant_names: HashSet<String>,
    pub(crate) class_names: HashSet<String>,
    pub(crate) field_names: HashSet<String>,
    pub(crate) class_methods: HashMap<String, HashSet<String>>,
    pub(crate) current_class: Option<String>,
    pub(crate) shader_annotation_anchor: Option<usize>,
}

impl<'source, 'context, 'resolver> Lowerer<'source, 'context, 'resolver> {
    pub(crate) fn new(
        source: &'source SourceFile,
        context: &'context mut LowerCtx<'resolver>,
        annotations: Annotations,
        diagnostics: Diagnostics,
    ) -> Self {
        Self {
            source,
            context,
            diagnostics,
            declared: HashSet::new(),
            annotations,
            loop_depth: 0,
            temporary_index: 0,
            source_names: HashSet::new(),
            constant_names: HashSet::new(),
            class_names: HashSet::new(),
            field_names: HashSet::new(),
            class_methods: HashMap::new(),
            current_class: None,
            shader_annotation_anchor: None,
        }
    }

    pub(crate) fn lower_program(mut self, program: &Program<'_>) -> Result<Module, Diagnostics> {
        let mut items = Vec::new();
        VariableCollector.walk_program(program, &mut self.source_names);
        for statement in program.statements.iter() {
            if let Statement::Class(class) = statement {
                self.register_class_shape(class);
            } else if let Statement::Constant(constant) = statement {
                for item in constant.items.iter() {
                    self.constant_names.insert(self.name(item.name.value));
                }
            }
        }
        for statement in program.statements.iter() {
            if let Statement::Class(class) = statement
                && let Some(class_items) = self.lower_class(class)
            {
                items.extend(class_items);
            }
        }
        for statement in program.statements.iter() {
            match statement {
                Statement::OpeningTag(_) | Statement::ClosingTag(_) | Statement::Noop(_) => {}
                Statement::Class(_) => {}
                Statement::Function(function) => {
                    if let Some(item) = self.lower_function(function) {
                        items.push(item);
                    }
                }
                Statement::Constant(constant) => {
                    if let Some(constants) = self.lower_constants(constant) {
                        items.extend(constants);
                    }
                }
                Statement::Inline(inline) if inline.value.iter().all(u8::is_ascii_whitespace) => {}
                Statement::Interface(_) | Statement::Trait(_) | Statement::Enum(_) => self
                    .unsupported_with_code(
                        statement.span(),
                        "E0203",
                        "interfaces, traits, and enums are outside the struct-like Common Core class subset",
                        "replace this declaration with composition or a plain top-level function",
                    ),
                _ => self.unsupported(
                    statement.span(),
                    "PHP top-level executable statements and declarations are outside Common Core",
                    "move executable code into `function setup()` and keep one source file",
                ),
            }
        }
        let module = Module {
            items,
            span: self.span(program.span()),
        };
        self.annotations.report_unused(&mut self.diagnostics);
        if self.diagnostics.has_errors() {
            Err(self.diagnostics)
        } else {
            Ok(module)
        }
    }

    pub(crate) fn lower_block(&mut self, block: &PhpBlock<'_>) -> Block {
        self.lower_statements(block.statements.as_slice(), block.span())
    }

    pub(crate) fn lower_statements(
        &mut self,
        statements: &[Statement<'_>],
        fallback: MagoSpan,
    ) -> Block {
        let lowered = statements
            .iter()
            .filter_map(|statement| self.lower_statement(statement))
            .flatten()
            .collect();
        Block {
            statements: lowered,
            span: self.span(fallback),
        }
    }

    pub(crate) fn lower_nested_statements(
        &mut self,
        statements: &[Statement<'_>],
        fallback: MagoSpan,
    ) -> Block {
        let outer = self.declared.clone();
        let block = self.lower_statements(statements, fallback);
        self.declared = outer;
        block
    }

    pub(crate) fn unsupported(&mut self, span: MagoSpan, message: &str, suggestion: &str) {
        self.unsupported_with_code(span, "E0200", message, suggestion);
    }

    pub(crate) fn unsupported_with_code(
        &mut self,
        span: MagoSpan,
        code: &str,
        message: &str,
        suggestion: &str,
    ) {
        let span = self.span(span);
        self.diagnostics.push(
            Diagnostic::new(Severity::Error, code, message, span)
                .with_suggestion(Suggestion::rewrite(span, suggestion)),
        );
    }

    pub(crate) fn span(&self, span: MagoSpan) -> Span {
        self.source
            .span(span.start_offset() as usize, span.end_offset() as usize)
            .expect("Mago nodes must use source byte boundaries")
    }

    pub(crate) fn name(&self, value: &[u8]) -> String {
        String::from_utf8_lossy(value).into_owned()
    }

    pub(crate) fn variable_name(&self, value: &[u8]) -> String {
        self.name(value).trim_start_matches('$').to_owned()
    }

    pub(crate) fn temporary(&mut self, purpose: &str) -> String {
        loop {
            let index = self.temporary_index;
            self.temporary_index += 1;
            let name = format!("__pgl_{purpose}_{index}");
            if !self.declared.contains(&name)
                && !self.source_names.contains(&name)
                && !self.constant_names.contains(&name)
            {
                return name;
            }
        }
    }

    pub(crate) fn annotation_for(
        &mut self,
        name: &str,
        span: MagoSpan,
    ) -> Option<polygl_hir::TypeExpr> {
        self.annotations
            .take(name, span.start_offset() as usize, self.source)
    }
}

struct VariableCollector;

impl<'ast, 'arena> Walker<'ast, 'arena, HashSet<String>> for VariableCollector {
    fn walk_in_direct_variable(
        &self,
        variable: &'ast DirectVariable<'arena>,
        names: &mut HashSet<String>,
    ) {
        names.insert(
            String::from_utf8_lossy(variable.name)
                .trim_start_matches('$')
                .to_owned(),
        );
    }
}
