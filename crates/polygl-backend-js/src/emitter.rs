use std::collections::{HashMap, HashSet};

use polygl_lir::{
    BinaryOp, Block, CallTarget, Domain, EntryKind, Expr, ExprKind, Literal, Module, Place,
    PlaceKind, Statement, StatementKind, UnaryOp,
};
use polygl_span::Span;
use polygl_types::Type;

use crate::source_map::{SourceCatalog, SourceMapBuilder};
use crate::{BuildMode, EmitError};

pub(crate) struct Emitted {
    pub(crate) body: String,
    pub(crate) mappings: SourceMapBuilder,
    mode: BuildMode,
    spans: Vec<Span>,
}

impl Emitted {
    pub(crate) fn header(
        &self,
        runtime_module: &str,
        catalog: &SourceCatalog<'_>,
    ) -> Result<String, EmitError> {
        let runtime_module =
            serde_json::to_string(runtime_module).expect("serializing a Rust string cannot fail");
        let mut header = format!(
            "import * as __pglRuntime from {runtime_module};\n\
             const __pglIsFalsy = (value) => value == null || value === false;\n\
             const __pglArithmeticError = (message, location) => {{\n\
             \x20 const error = new RangeError(message);\n\
             \x20 if (location !== undefined) error.polyglLocation = location;\n\
             \x20 return error;\n\
             }};\n\
             const __pglIntDivide = (left, right, location) => {{\n\
             \x20 if (right === 0) throw __pglArithmeticError(\"integer division by zero\", location);\n\
             \x20 return Math.floor(left / right) | 0;\n\
             }};\n\
             const __pglFloorRemainder = (left, right) => {{\n\
             \x20 const remainder = left % right;\n\
             \x20 return remainder !== 0 && (remainder < 0) !== (right < 0) ? remainder + right : remainder;\n\
             }};\n\
             const __pglIntFloorRemainder = (left, right, location) => {{\n\
             \x20 if (right === 0) throw __pglArithmeticError(\"integer remainder by zero\", location);\n\
             \x20 return __pglFloorRemainder(left, right) | 0;\n\
             }};\n\
             const __pglIntTruncatingRemainder = (left, right, location) => {{\n\
             \x20 if (right === 0) throw __pglArithmeticError(\"integer remainder by zero\", location);\n\
             \x20 return (left % right) | 0;\n\
             }};\n"
        );
        if self.mode == BuildMode::Debug {
            header.push_str("const __pglSpans = Object.freeze([\n");
            for span in &self.spans {
                let location = catalog.locate(*span)?;
                let entry = serde_json::json!({
                    "source": location.source.name(),
                    "line": location.line,
                    "column": location.utf16_column,
                    "start": span.start(),
                    "end": span.end(),
                });
                header.push_str("  Object.freeze(");
                header.push_str(&entry.to_string());
                header.push_str("),\n");
            }
            header.push_str("]);\n");
        }
        header.push('\n');
        Ok(header)
    }
}

pub(crate) struct Emitter<'source> {
    mode: BuildMode,
    catalog: &'source SourceCatalog<'source>,
    body: String,
    line: usize,
    column: usize,
    indent: usize,
    mappings: SourceMapBuilder,
    spans: Vec<Span>,
    span_ids: HashMap<Span, usize>,
    next_temporary: usize,
    scopes: Vec<HashMap<String, String>>,
    binding_counts: HashMap<String, usize>,
    used_bindings: HashSet<String>,
}

impl<'source> Emitter<'source> {
    pub(crate) fn new(mode: BuildMode, catalog: &'source SourceCatalog<'source>) -> Self {
        Self {
            mode,
            catalog,
            body: String::new(),
            line: 0,
            column: 0,
            indent: 0,
            mappings: SourceMapBuilder::default(),
            spans: Vec::new(),
            span_ids: HashMap::new(),
            next_temporary: 0,
            scopes: Vec::new(),
            binding_counts: HashMap::new(),
            used_bindings: HashSet::new(),
        }
    }

    pub(crate) fn emit(mut self, program: &Module) -> Result<Emitted, EmitError> {
        let mut wrote_section = false;
        for constant in program
            .constants
            .iter()
            .filter(|constant| constant.domain != Domain::Gpu)
        {
            self.indent();
            self.mark(constant.span)?;
            self.write("const ");
            self.write(&constant_identifier(&constant.name));
            self.write(" = ");
            self.expression(&constant.value)?;
            self.write(";");
            self.newline();
            wrote_section = true;
        }
        if wrote_section {
            self.newline();
        }

        wrote_section = false;
        for function in program
            .functions
            .iter()
            .filter(|function| function.domain != Domain::Gpu)
        {
            self.indent();
            self.mark(function.span)?;
            self.write("function ");
            self.write(&function_identifier(&function.name));
            self.begin_callable();
            self.push_scope();
            self.parameters(
                function
                    .params
                    .iter()
                    .map(|parameter| parameter.name.as_str()),
            );
            self.write(" ");
            self.function_body(&function.body)?;
            self.pop_scope();
            self.end_callable();
            self.newline();
            self.newline();
            wrote_section = true;
        }

        for entry in program
            .entries
            .iter()
            .filter(|entry| entry.domain == Domain::Host)
        {
            self.indent();
            self.mark(entry.span)?;
            self.write("export function ");
            self.write(entry_name(&entry.kind));
            self.begin_callable();
            self.push_scope();
            self.parameters(entry.params.iter().map(|parameter| parameter.name.as_str()));
            self.write(" ");
            self.function_body(&entry.body)?;
            self.pop_scope();
            self.end_callable();
            self.newline();
            self.newline();
            wrote_section = true;
        }

        if !wrote_section && self.body.is_empty() {
            self.write("export {};");
            self.newline();
        }

        Ok(Emitted {
            body: self.body,
            mappings: self.mappings,
            mode: self.mode,
            spans: self.spans,
        })
    }

    fn parameters<'name>(&mut self, names: impl Iterator<Item = &'name str>) {
        self.write("(");
        for (index, name) in names.enumerate() {
            if index > 0 {
                self.write(", ");
            }
            let identifier = self.fresh_binding(name);
            self.write(&identifier);
            self.bind(name, identifier);
        }
        self.write(")");
    }

    fn block(&mut self, block: &Block) -> Result<(), EmitError> {
        self.write("{");
        self.newline();
        self.indent += 1;
        self.push_scope();
        for statement in &block.statements {
            self.statement(statement)?;
        }
        self.pop_scope();
        self.indent -= 1;
        self.indent();
        self.write("}");
        Ok(())
    }

    fn function_body(&mut self, block: &Block) -> Result<(), EmitError> {
        self.write("{");
        self.newline();
        self.indent += 1;
        self.indent();
        self.block(block)?;
        self.newline();
        self.indent -= 1;
        self.indent();
        self.write("}");
        Ok(())
    }

    fn statement(&mut self, statement: &Statement) -> Result<(), EmitError> {
        self.indent();
        self.mark(statement.span)?;
        match &statement.kind {
            StatementKind::Let { name, init, .. } => {
                let identifier = self.fresh_binding(name);
                self.write("let ");
                self.write(&identifier);
                self.write(" = ");
                self.expression(init)?;
                self.write(";");
                self.newline();
                self.bind(name, identifier);
            }
            StatementKind::Assign { target, value } => {
                self.assignment(target, value)?;
                self.write(";");
                self.newline();
            }
            StatementKind::Expr(expression) => {
                self.expression(expression)?;
                self.write(";");
                self.newline();
            }
            StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                self.write("if (");
                self.expression(condition)?;
                self.write(") ");
                self.block(then_block)?;
                if let Some(else_block) = else_block {
                    self.write(" else ");
                    self.block(else_block)?;
                }
                self.newline();
            }
            StatementKind::While { condition, body } => {
                self.write("while (");
                self.expression(condition)?;
                self.write(") ");
                self.block(body)?;
                self.newline();
            }
            StatementKind::For {
                variable,
                range,
                body,
            } => self.range_loop(variable, range, body)?,
            StatementKind::Return(value) => {
                self.write("return");
                if let Some(value) = value {
                    self.write(" ");
                    self.expression(value)?;
                }
                self.write(";");
                self.newline();
            }
            StatementKind::Break => {
                self.write("break;");
                self.newline();
            }
            StatementKind::Continue => {
                self.write("continue;");
                self.newline();
            }
        }
        Ok(())
    }

    fn range_loop(
        &mut self,
        variable: &str,
        range: &polygl_lir::Range,
        body: &Block,
    ) -> Result<(), EmitError> {
        let suffix = self.temporary();
        let start = format!("__pglRangeStart{suffix}");
        let end = format!("__pglRangeEnd{suffix}");
        let done = format!("__pglRangeDone{suffix}");
        let index = format!("__pglRangeIndex{suffix}");

        self.write("{");
        self.newline();
        self.indent += 1;
        self.indent();
        self.write("const ");
        self.write(&start);
        self.write(" = ");
        self.expression(&range.start)?;
        self.write(";");
        self.newline();
        self.indent();
        self.write("const ");
        self.write(&end);
        self.write(" = ");
        self.expression(&range.end)?;
        self.write(";");
        self.newline();
        self.indent();
        self.write("for (let ");
        self.write(&index);
        self.write(" = ");
        self.write(&start);
        if range.inclusive {
            self.write(", ");
            self.write(&done);
            self.write(" = false; !");
            self.write(&done);
            self.write(" && ");
            self.write(&index);
            self.write(" <= ");
            self.write(&end);
        } else {
            self.write("; ");
            self.write(&index);
            self.write(" < ");
            self.write(&end);
        }
        self.write("; ");
        self.write(&index);
        self.write(" = (");
        self.write(&index);
        self.write(" + 1) | 0) {");
        self.newline();
        self.indent += 1;
        if range.inclusive {
            self.indent();
            self.write(&done);
            self.write(" = ");
            self.write(&index);
            self.write(" === ");
            self.write(&end);
            self.write(";");
            self.newline();
        }
        self.indent();
        self.write("let ");
        self.push_scope();
        let variable_identifier = self.fresh_binding(variable);
        self.bind(variable, variable_identifier.clone());
        self.write(&variable_identifier);
        self.write(" = ");
        self.write(&index);
        self.write(";");
        self.newline();
        self.indent();
        self.block(body)?;
        self.newline();
        self.pop_scope();
        self.indent -= 1;
        self.indent();
        self.write("}");
        self.newline();
        self.indent -= 1;
        self.indent();
        self.write("}");
        self.newline();
        Ok(())
    }

    fn assignment(&mut self, target: &Place, value: &Expr) -> Result<(), EmitError> {
        match &target.kind {
            PlaceKind::Variable(name) => {
                self.write(&self.binding(name));
                self.write(" = ");
                self.expression(value)?;
            }
            PlaceKind::Index { base, index }
                if self.mode == BuildMode::Debug && requires_bounds_check(&base.ty) =>
            {
                let span = self.span_id(target.span)?;
                let suffix = self.temporary();
                let base_temporary = format!("__pglIndexBase{suffix}");
                let index_temporary = format!("__pglIndexValue{suffix}");
                self.write("((");
                self.write(&base_temporary);
                self.write(", ");
                self.write(&index_temporary);
                self.write(") => (__pglRuntime.checkIndex(");
                self.write(&base_temporary);
                self.write(", ");
                self.write(&index_temporary);
                self.write(", __pglSpans[");
                self.write(&span.to_string());
                self.write("]), ");
                self.write(&base_temporary);
                self.write("[");
                self.write(&index_temporary);
                self.write("] = ");
                self.expression(value)?;
                self.write("))(");
                self.expression(base)?;
                self.write(", ");
                self.expression(index)?;
                self.write(")");
            }
            PlaceKind::Index { base, index } => {
                self.write("(");
                self.expression(base)?;
                self.write(")[");
                self.expression(index)?;
                self.write("] = ");
                self.expression(value)?;
            }
            PlaceKind::Field { base, field } => {
                if self.mode == BuildMode::Debug {
                    let span = self.span_id(target.span)?;
                    self.write("__pglRuntime.requireNonNil(");
                    self.expression(base)?;
                    self.write(", __pglSpans[");
                    self.write(&span.to_string());
                    self.write("])");
                } else {
                    self.write("(");
                    self.expression(base)?;
                    self.write(")");
                }
                self.write("[");
                self.write(&json_string(field));
                self.write("] = ");
                self.expression(value)?;
            }
        }
        Ok(())
    }

    fn expression(&mut self, expression: &Expr) -> Result<(), EmitError> {
        self.mark(expression.span)?;
        match &expression.kind {
            ExprKind::Literal(literal) => self.literal(literal),
            ExprKind::Variable(name) => self.write(&self.binding(name)),
            ExprKind::Constant(name) => self.write(&constant_identifier(name)),
            ExprKind::Binary { op, left, right } => {
                self.binary(*op, left, right, &expression.ty, expression.span)?;
            }
            ExprKind::Unary { op, operand } => self.unary(*op, operand, &expression.ty)?,
            ExprKind::Call { target, args } => {
                match target {
                    CallTarget::Function(name) => self.write(&function_identifier(name)),
                    CallTarget::Runtime(operation) => {
                        self.write("__pglRuntime.");
                        self.write(operation.as_str());
                    }
                }
                self.write("(");
                self.expressions(args)?;
                self.write(")");
            }
            ExprKind::Index { base, index }
                if self.mode == BuildMode::Debug && requires_bounds_check(&base.ty) =>
            {
                let span = self.span_id(expression.span)?;
                self.write("__pglRuntime.checkedIndex(");
                self.expression(base)?;
                self.write(", ");
                self.expression(index)?;
                self.write(", __pglSpans[");
                self.write(&span.to_string());
                self.write("])");
            }
            ExprKind::Index { base, index } => {
                self.write("(");
                self.expression(base)?;
                self.write(")[");
                self.expression(index)?;
                self.write("]");
            }
            ExprKind::Field { base, field } => {
                if self.mode == BuildMode::Debug {
                    let span = self.span_id(expression.span)?;
                    self.write("__pglRuntime.requireNonNil(");
                    self.expression(base)?;
                    self.write(", __pglSpans[");
                    self.write(&span.to_string());
                    self.write("])");
                } else {
                    self.write("(");
                    self.expression(base)?;
                    self.write(")");
                }
                self.write("[");
                self.write(&json_string(field));
                self.write("]");
            }
            ExprKind::ArrayLength(value) => {
                self.write("(");
                self.expression(value)?;
                self.write(").length");
            }
            ExprKind::Array(items) => {
                self.write("[");
                self.expressions(items)?;
                self.write("]");
            }
            ExprKind::Map(entries) => {
                self.write("Object.fromEntries([");
                for (index, entry) in entries.iter().enumerate() {
                    if index > 0 {
                        self.write(", ");
                    }
                    self.write("[");
                    self.expression(&entry.key)?;
                    self.write(", ");
                    self.expression(&entry.value)?;
                    self.write("]");
                }
                self.write("])");
            }
            ExprKind::Struct { fields, .. } => {
                self.write("{");
                for (index, field) in fields.iter().enumerate() {
                    if index > 0 {
                        self.write(", ");
                    }
                    self.write(&json_string(&field.name));
                    self.write(": ");
                    self.expression(&field.value)?;
                }
                self.write("}");
            }
            ExprKind::Vector { args, .. } => {
                self.write("new Float32Array([");
                self.expressions(args)?;
                self.write("])");
            }
            ExprKind::IsNil(value) => {
                self.write("(");
                self.expression(value)?;
                self.write(" == null)");
            }
            ExprKind::IsFalsy(value) => {
                self.write("__pglIsFalsy(");
                self.expression(value)?;
                self.write(")");
            }
        }
        Ok(())
    }

    fn expressions(&mut self, expressions: &[Expr]) -> Result<(), EmitError> {
        for (index, expression) in expressions.iter().enumerate() {
            if index > 0 {
                self.write(", ");
            }
            self.expression(expression)?;
        }
        Ok(())
    }

    fn binary(
        &mut self,
        operator: BinaryOp,
        left: &Expr,
        right: &Expr,
        result_type: &Type,
        span: Span,
    ) -> Result<(), EmitError> {
        if result_type == &Type::Int {
            match operator {
                BinaryOp::Add | BinaryOp::Subtract => {
                    self.write("((");
                    self.expression(left)?;
                    self.write(if operator == BinaryOp::Add {
                        " + "
                    } else {
                        " - "
                    });
                    self.expression(right)?;
                    self.write(") | 0)");
                    return Ok(());
                }
                BinaryOp::Multiply => {
                    self.write("Math.imul(");
                    self.expression(left)?;
                    self.write(", ");
                    self.expression(right)?;
                    self.write(")");
                    return Ok(());
                }
                BinaryOp::IntegerDivide => {
                    self.write("__pglIntDivide(");
                    self.expression(left)?;
                    self.write(", ");
                    self.expression(right)?;
                    self.debug_location_argument(span)?;
                    self.write(")");
                    return Ok(());
                }
                BinaryOp::FloorRemainder => {
                    self.write("__pglIntFloorRemainder(");
                    self.expression(left)?;
                    self.write(", ");
                    self.expression(right)?;
                    self.debug_location_argument(span)?;
                    self.write(")");
                    return Ok(());
                }
                BinaryOp::TruncatingRemainder => {
                    self.write("__pglIntTruncatingRemainder(");
                    self.expression(left)?;
                    self.write(", ");
                    self.expression(right)?;
                    self.debug_location_argument(span)?;
                    self.write(")");
                    return Ok(());
                }
                _ => {}
            }
        }
        if operator == BinaryOp::FloorRemainder {
            self.write("__pglFloorRemainder(");
            self.expression(left)?;
            self.write(", ");
            self.expression(right)?;
            self.write(")");
            return Ok(());
        }

        self.write("(");
        self.expression(left)?;
        self.write(match operator {
            BinaryOp::Add | BinaryOp::StringConcat => " + ",
            BinaryOp::Subtract => " - ",
            BinaryOp::Multiply => " * ",
            BinaryOp::IntegerDivide | BinaryOp::FloatDivide => " / ",
            BinaryOp::FloorRemainder => {
                unreachable!("floor remainder is emitted as a helper call")
            }
            BinaryOp::TruncatingRemainder => " % ",
            BinaryOp::Equal => " === ",
            BinaryOp::NotEqual => " !== ",
            BinaryOp::Less => " < ",
            BinaryOp::LessEqual => " <= ",
            BinaryOp::Greater => " > ",
            BinaryOp::GreaterEqual => " >= ",
            BinaryOp::And => " && ",
            BinaryOp::Or => " || ",
        });
        self.expression(right)?;
        self.write(")");
        Ok(())
    }

    fn unary(
        &mut self,
        operator: UnaryOp,
        operand: &Expr,
        result_type: &Type,
    ) -> Result<(), EmitError> {
        match (operator, result_type) {
            (UnaryOp::Negate, Type::Int) => {
                self.write("((-");
                self.expression(operand)?;
                self.write(") | 0)");
            }
            (UnaryOp::Negate, _) => {
                self.write("(-");
                self.expression(operand)?;
                self.write(")");
            }
            (UnaryOp::Not, _) => {
                self.write("(!");
                self.expression(operand)?;
                self.write(")");
            }
        }
        Ok(())
    }

    fn literal(&mut self, literal: &Literal) {
        match literal {
            Literal::Int(value) => self.write(&value.to_string()),
            Literal::Float(value) if value.is_nan() => self.write("Number.NaN"),
            Literal::Float(value) if *value == f64::INFINITY => {
                self.write("Number.POSITIVE_INFINITY");
            }
            Literal::Float(value) if *value == f64::NEG_INFINITY => {
                self.write("Number.NEGATIVE_INFINITY");
            }
            Literal::Float(value) => self.write(&value.to_string()),
            Literal::Bool(value) => self.write(if *value { "true" } else { "false" }),
            Literal::Str(value) => self.write(&json_string(value)),
            Literal::None => self.write("null"),
        }
    }

    fn span_id(&mut self, span: Span) -> Result<usize, EmitError> {
        self.catalog.locate(span)?;
        if let Some(id) = self.span_ids.get(&span) {
            return Ok(*id);
        }
        let id = self.spans.len();
        self.spans.push(span);
        self.span_ids.insert(span, id);
        Ok(id)
    }

    fn debug_location_argument(&mut self, span: Span) -> Result<(), EmitError> {
        if self.mode == BuildMode::Debug {
            let id = self.span_id(span)?;
            self.write(", __pglSpans[");
            self.write(&id.to_string());
            self.write("]");
        }
        Ok(())
    }

    fn mark(&mut self, span: Span) -> Result<(), EmitError> {
        self.catalog.locate(span)?;
        self.mappings.add(self.line, self.column, span);
        Ok(())
    }

    fn temporary(&mut self) -> usize {
        let temporary = self.next_temporary;
        self.next_temporary += 1;
        temporary
    }

    fn begin_callable(&mut self) {
        debug_assert!(self.scopes.is_empty());
        self.binding_counts.clear();
        self.used_bindings.clear();
    }

    fn end_callable(&mut self) {
        debug_assert!(self.scopes.is_empty());
        self.binding_counts.clear();
        self.used_bindings.clear();
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop().expect("emitter scopes are balanced");
    }

    fn fresh_binding(&mut self, name: &str) -> String {
        let count = self.binding_counts.entry(name.to_owned()).or_default();
        let base = local_identifier(name);
        loop {
            let identifier = if *count == 0 {
                base.clone()
            } else {
                format!("{base}${count}")
            };
            *count += 1;
            if self.used_bindings.insert(identifier.clone()) {
                return identifier;
            }
        }
    }

    fn bind(&mut self, name: &str, identifier: String) {
        self.scopes
            .last_mut()
            .expect("bindings are emitted inside a lexical scope")
            .insert(name.to_owned(), identifier);
    }

    fn binding(&self, name: &str) -> String {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .cloned()
            .unwrap_or_else(|| local_identifier(name))
    }

    fn indent(&mut self) {
        self.write(&"  ".repeat(self.indent));
    }

    fn write(&mut self, value: &str) {
        self.body.push_str(value);
        if let Some((_, last)) = value.rsplit_once('\n') {
            self.line += value.bytes().filter(|byte| *byte == b'\n').count();
            self.column = last.encode_utf16().count();
        } else {
            self.column += value.encode_utf16().count();
        }
    }

    fn newline(&mut self) {
        self.body.push('\n');
        self.line += 1;
        self.column = 0;
    }
}

fn entry_name(kind: &EntryKind) -> &str {
    match kind {
        EntryKind::Setup => "setup",
        EntryKind::Frame => "frame",
        EntryKind::OnEvent => "on_event",
        EntryKind::Vertex(_) | EntryKind::Fragment(_) => {
            unreachable!("the JavaScript backend emits only Host entries")
        }
    }
}

fn function_identifier(name: &str) -> String {
    encoded_identifier("__pglFunction", name)
}

fn constant_identifier(name: &str) -> String {
    encoded_identifier("__pglConstant", name)
}

fn local_identifier(name: &str) -> String {
    if is_safe_identifier(name) && !name.starts_with("__pgl") {
        name.to_owned()
    } else {
        encoded_identifier("__pglLocal", name)
    }
}

fn encoded_identifier(prefix: &str, name: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(prefix.len() + 1 + name.len() * 2);
    encoded.push_str(prefix);
    encoded.push('_');
    for byte in name.bytes() {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 15)]));
    }
    encoded
}

fn is_safe_identifier(name: &str) -> bool {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && characters.all(|character| {
            character == '_' || character == '$' || character.is_ascii_alphanumeric()
        })
        && !is_reserved_word(name)
}

fn is_reserved_word(name: &str) -> bool {
    matches!(
        name,
        "await"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "eval"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "implements"
            | "import"
            | "in"
            | "instanceof"
            | "interface"
            | "arguments"
            | "let"
            | "new"
            | "null"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "return"
            | "static"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
    )
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a Rust string cannot fail")
}

fn requires_bounds_check(ty: &Type) -> bool {
    matches!(ty, Type::Array(_) | Type::Vector(_) | Type::Matrix(_))
}
