use polygl_span::{Diagnostic, Severity, Span, Suggestion};

use crate::solver::{InferType, SolveError};

use super::Analyzer;

impl Analyzer {
    pub(super) fn solve_error(&mut self, error: SolveError, span: Span, code: &str) {
        match error {
            SolveError::Mismatch { expected, actual } => self.diagnostics.push(
                Diagnostic::new(
                    Severity::Error,
                    code,
                    format!("expected `{expected}`, found `{actual}`"),
                    span,
                )
                .with_suggestion(Suggestion::rewrite(
                    span,
                    format!("convert the value explicitly to `{expected}`"),
                )),
            ),
            SolveError::Infinite(variable) => self.diagnostics.push(
                Diagnostic::new(
                    Severity::Error,
                    "E0312",
                    format!("type variable ?{variable} would contain itself"),
                    span,
                )
                .with_suggestion(Suggestion::rewrite(
                    span,
                    "add a concrete @pgl type annotation",
                )),
            ),
            SolveError::Unresolved(ty) => self.unresolved_error(&ty, span, None),
        }
    }

    pub(super) fn solve_or_unresolved(
        &mut self,
        error: SolveError,
        span: Span,
        name: Option<&str>,
    ) {
        match error {
            SolveError::Unresolved(ty) => self.unresolved_error(&ty, span, name),
            error => self.solve_error(error, span, "E0303"),
        }
    }

    pub(super) fn unresolved_error(&mut self, ty: &InferType, span: Span, name: Option<&str>) {
        let target = name.map_or_else(|| "value".to_owned(), |name| format!("`{name}`"));
        self.diagnostics.push(
            Diagnostic::new(
                Severity::Error,
                "E0312",
                format!("the type of {target} remains unresolved ({ty})"),
                span,
            )
            .with_suggestion(Suggestion::rewrite(
                span,
                name.map_or_else(
                    || "add a source-language @pgl type annotation".to_owned(),
                    |name| format!("add an `@pgl {name}: type` annotation"),
                ),
            )),
        );
    }

    pub(super) fn reassignment_error(&mut self, error: SolveError, span: Span) {
        match error {
            SolveError::Mismatch { expected, actual } => self.diagnostics.push(
                Diagnostic::new(
                    Severity::Error,
                    "E0311",
                    format!(
                        "reassignment changes the variable type from `{expected}` to `{actual}`"
                    ),
                    span,
                )
                .with_suggestion(Suggestion::rewrite(
                    span,
                    format!(
                        "keep the assigned value `{expected}` or convert `{actual}` explicitly"
                    ),
                )),
            ),
            error => self.solve_error(error, span, "E0311"),
        }
    }

    pub(super) fn condition_error(&mut self, actual: &InferType, span: Span) {
        self.diagnostics.push(
            Diagnostic::new(
                Severity::Error,
                "E0301",
                format!("condition must have type `bool`, found `{actual}`"),
                span,
            )
            .with_suggestion(Suggestion::rewrite(
                span,
                "compare the value explicitly to produce a boolean",
            )),
        );
    }

    pub(super) fn unit_value_error(&mut self, span: Span) {
        self.diagnostics.push(
            Diagnostic::new(
                Severity::Error,
                "E0303",
                "`void` cannot be used as a value",
                span,
            )
            .with_suggestion(Suggestion::rewrite(
                span,
                "call the void function as a standalone statement",
            )),
        );
    }

    pub(super) fn unknown_type_error(&mut self, name: &str, span: Span) {
        self.diagnostics.push(
            Diagnostic::new(
                Severity::Error,
                "E0305",
                format!("unknown struct type `{name}`"),
                span,
            )
            .with_suggestion(Suggestion::rewrite(
                span,
                format!("define struct `{name}` before using this annotation"),
            )),
        );
    }

    pub(super) fn reserved_type_error(&mut self, name: &str, span: Span) {
        self.diagnostics.push(
            Diagnostic::new(
                Severity::Error,
                "E0306",
                format!("struct type `{name}` is reserved by Common Core"),
                span,
            )
            .with_suggestion(Suggestion::rewrite(
                span,
                format!("rename this struct instead of redefining `{name}`"),
            )),
        );
    }

    pub(super) fn invalid_dimension_error(&mut self, kind: &str, size: u8, span: Span) {
        self.diagnostics.push(
            Diagnostic::new(
                Severity::Error,
                "E0303",
                format!("{kind} dimension must be between 2 and 4, found `{size}`"),
                span,
            )
            .with_suggestion(Suggestion::rewrite(
                span,
                format!("use {kind}2, {kind}3, or {kind}4"),
            )),
        );
    }

    pub(super) fn loop_control_error(&mut self, keyword: &str, span: Span) {
        self.diagnostics.push(
            Diagnostic::new(
                Severity::Error,
                "E0200",
                format!("`{keyword}` is only valid inside a loop"),
                span,
            )
            .with_suggestion(Suggestion::rewrite(
                span,
                format!("move `{keyword}` into a loop body"),
            )),
        );
    }

    pub(super) fn constant_assignment_error(&mut self, name: &str, span: Span) {
        self.diagnostics.push(
            Diagnostic::new(
                Severity::Error,
                "E0311",
                format!("constant `{name}` cannot be assigned"),
                span,
            )
            .with_suggestion(Suggestion::rewrite(
                span,
                format!("assign to a local variable instead of `{name}`"),
            )),
        );
    }

    pub(super) fn unknown_variable_error(&mut self, name: &str, span: Span) {
        self.diagnostics.push(
            Diagnostic::new(
                Severity::Error,
                "E0305",
                format!("unknown local variable `{name}`"),
                span,
            )
            .with_suggestion(Suggestion::rewrite(
                span,
                format!("assign `{name}` before using it"),
            )),
        );
    }

    pub(super) fn unknown_function_error(&mut self, name: &str, span: Span) {
        self.diagnostics.push(
            Diagnostic::new(
                Severity::Error,
                "E0305",
                format!("unknown function `{name}`"),
                span,
            )
            .with_suggestion(Suggestion::rewrite(
                span,
                format!("define `{name}` or use a registered builtin"),
            )),
        );
    }

    pub(super) fn arity_error(&mut self, name: &str, expected: usize, actual: usize, span: Span) {
        self.diagnostics.push(
            Diagnostic::new(
                Severity::Error,
                "E0306",
                format!("`{name}` expects {expected} arguments, found {actual}"),
                span,
            )
            .with_suggestion(Suggestion::rewrite(
                span,
                format!("pass exactly {expected} arguments"),
            )),
        );
    }

    pub(super) fn arity_range_error(
        &mut self,
        name: &str,
        required: usize,
        total: usize,
        actual: usize,
        span: Span,
    ) {
        self.diagnostics.push(
            Diagnostic::new(
                Severity::Error,
                "E0306",
                format!(
                    "`{name}` expects between {required} and {total} arguments, found {actual}"
                ),
                span,
            )
            .with_suggestion(Suggestion::rewrite(
                span,
                format!("pass between {required} and {total} arguments"),
            )),
        );
    }

    pub(super) fn unknown_struct_field_error(&mut self, structure: &str, field: &str, span: Span) {
        self.diagnostics.push(
            Diagnostic::new(
                Severity::Error,
                "E0305",
                format!("`{structure}` has no field named `{field}`"),
                span,
            )
            .with_suggestion(Suggestion::rewrite(
                span,
                format!("remove `{field}` or use a declared field"),
            )),
        );
    }

    pub(super) fn duplicate_struct_field_error(&mut self, field: &str, span: Span) {
        self.diagnostics.push(
            Diagnostic::new(
                Severity::Error,
                "E0306",
                format!("field `{field}` is initialized more than once"),
                span,
            )
            .with_suggestion(Suggestion::rewrite(
                span,
                format!("keep only one initializer for `{field}`"),
            )),
        );
    }

    pub(super) fn duplicate_declaration_error(&mut self, kind: &str, name: &str, span: Span) {
        self.diagnostics.push(
            Diagnostic::new(
                Severity::Error,
                "E0306",
                format!("{kind} `{name}` is declared more than once"),
                span,
            )
            .with_suggestion(Suggestion::rewrite(
                span,
                format!("rename or remove the duplicate {kind} `{name}`"),
            )),
        );
    }

    pub(super) fn missing_struct_field_error(&mut self, structure: &str, field: &str, span: Span) {
        self.diagnostics.push(
            Diagnostic::new(
                Severity::Error,
                "E0306",
                format!("`{structure}` is missing required field `{field}`"),
                span,
            )
            .with_suggestion(Suggestion::rewrite(span, format!("initialize `{field}`"))),
        );
    }

    pub(super) fn instance_limit_error(&mut self, name: &str, limit: usize, span: Span) {
        self.diagnostics.push(
            Diagnostic::new(
                Severity::Error,
                "E0310",
                format!("`{name}` exceeds the limit of {limit} type instances"),
                span,
            )
            .with_suggestion(Suggestion::rewrite(
                span,
                format!("add @pgl annotations or split `{name}` into type-specific functions"),
            )),
        );
    }

    pub(super) fn recursive_error(&mut self, name: &str, span: Span) {
        self.diagnostics.push(
            Diagnostic::new(
                Severity::Error,
                "E0313",
                format!("cannot infer recursive instance of `{name}`"),
                span,
            )
            .with_suggestion(Suggestion::rewrite(
                span,
                "rewrite the recursion as a loop or add an explicitly typed helper",
            )),
        );
    }

    pub(super) fn configuration_error(&mut self, span: Span) {
        self.diagnostics.push(Diagnostic::new(
            Severity::Error,
            "E0001",
            "type instance limit must be greater than zero",
            span,
        ));
    }
}
