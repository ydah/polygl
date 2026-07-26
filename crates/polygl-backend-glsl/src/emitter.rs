use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use polygl_lir::{
    BinaryOp, Block, CallTarget, EntryPoint, Expr, ExprKind, Literal, Module, Place, PlaceKind,
    Statement, StatementKind, StructDef, UnaryOp,
};
use polygl_types::Type;

use crate::{AttributeBinding, EmitError, ShaderStage};

const PREAMBLE: &str = "#version 300 es\nprecision highp float;\nprecision highp int;\n\n";

pub(crate) fn attribute_bindings(entry: &EntryPoint) -> Result<Vec<AttributeBinding>, EmitError> {
    entry
        .params
        .iter()
        .map(|parameter| {
            Ok(AttributeBinding {
                name: parameter.name.clone(),
                glsl_name: attribute_name(&parameter.name)?.to_owned(),
                location: attribute_location(&parameter.name)?,
                ty: parameter.ty.clone(),
            })
        })
        .collect()
}

pub(crate) fn uses_time(module: &Module) -> bool {
    module
        .constants
        .iter()
        .any(|constant| expr_uses_time(&constant.value))
        || module
            .functions
            .iter()
            .any(|function| block_uses_time(&function.body))
        || module
            .entries
            .iter()
            .any(|entry| block_uses_time(&entry.body))
}

pub(crate) struct Emitter<'module> {
    module: &'module Module,
    shader_name: String,
    stage: ShaderStage,
    entry: &'module EntryPoint,
    other_stage: &'module EntryPoint,
    output: String,
    indent: usize,
    scopes: Vec<HashMap<String, String>>,
    local_counter: usize,
    in_entry: bool,
}

impl<'module> Emitter<'module> {
    pub(crate) fn new(
        module: &'module Module,
        shader_name: &str,
        stage: ShaderStage,
        entry: &'module EntryPoint,
        other_stage: &'module EntryPoint,
    ) -> Self {
        Self {
            module,
            shader_name: shader_name.to_owned(),
            stage,
            entry,
            other_stage,
            output: String::new(),
            indent: 0,
            scopes: Vec::new(),
            local_counter: 0,
            in_entry: false,
        }
    }

    pub(crate) fn emit(mut self) -> Result<String, EmitError> {
        self.output.push_str(PREAMBLE);
        self.emit_structs()?;
        self.emit_interface()?;
        self.emit_helpers();
        self.emit_constants()?;
        self.emit_function_prototypes()?;
        self.emit_functions()?;
        self.emit_entry()?;
        Ok(self.output)
    }

    fn emit_structs(&mut self) -> Result<(), EmitError> {
        let mut emitted = HashSet::new();
        let mut visiting = HashSet::new();
        for definition in &self.module.structs {
            self.emit_struct(definition, &mut visiting, &mut emitted)?;
        }
        if !emitted.is_empty() {
            self.output.push('\n');
        }
        Ok(())
    }

    fn emit_struct(
        &mut self,
        definition: &'module StructDef,
        visiting: &mut HashSet<&'module str>,
        emitted: &mut HashSet<&'module str>,
    ) -> Result<(), EmitError> {
        if emitted.contains(definition.name.as_str()) {
            return Ok(());
        }
        if !visiting.insert(definition.name.as_str()) {
            return Err(EmitError::UnsupportedExpression("recursive GPU struct"));
        }
        for field in &definition.fields {
            if let Type::Struct(dependency) = &field.ty {
                let dependency = self
                    .module
                    .structs
                    .iter()
                    .find(|candidate| candidate.name == dependency.as_str())
                    .ok_or_else(|| EmitError::MissingStruct(dependency.to_string()))?;
                self.emit_struct(dependency, visiting, emitted)?;
            }
        }
        visiting.remove(definition.name.as_str());
        self.line(&format!("struct {} {{", struct_name(&definition.name)));
        self.indent += 1;
        for field in &definition.fields {
            self.line(&format!(
                "{} {};",
                glsl_type(&field.ty)?,
                field_name(&field.name)
            ));
        }
        self.indent -= 1;
        self.line("};");
        emitted.insert(definition.name.as_str());
        Ok(())
    }

    fn emit_interface(&mut self) -> Result<(), EmitError> {
        if uses_time(self.module) {
            self.line("uniform float u_time;");
        }
        match self.stage {
            ShaderStage::Vertex => {
                for attribute in attribute_bindings(self.entry)? {
                    self.line(&format!(
                        "layout(location = {}) in {} {};",
                        attribute.location,
                        glsl_type(&attribute.ty)?,
                        attribute.glsl_name
                    ));
                }
                if let Type::Struct(name) = &self.entry.result {
                    let fields = self.struct_definition(name.as_str())?.fields.clone();
                    for field in &fields {
                        self.line(&format!(
                            "{}out {} {};",
                            interpolation_qualifier(&field.ty),
                            glsl_type(&field.ty)?,
                            varying_name(&field.name)
                        ));
                    }
                }
            }
            ShaderStage::Fragment => {
                if let Type::Struct(name) = &self.other_stage.result {
                    let fields = self.struct_definition(name.as_str())?.fields.clone();
                    for field in &fields {
                        self.line(&format!(
                            "{}in {} {};",
                            interpolation_qualifier(&field.ty),
                            glsl_type(&field.ty)?,
                            varying_name(&field.name)
                        ));
                    }
                }
                self.line("out vec4 out_color;");
            }
        }
        self.output.push('\n');
        Ok(())
    }

    fn emit_helpers(&mut self) {
        self.output.push_str(
            "int pgl_int_div(int left, int right) {\n\
             \u{20} if (right == 0) return 0;\n\
             \u{20} if (left == (-2147483647 - 1) && right == -1) return left;\n\
             \u{20} int quotient = left / right;\n\
             \u{20} int remainder = left % right;\n\
             \u{20} return remainder != 0 && ((remainder < 0) != (right < 0)) ? quotient - 1 : quotient;\n\
             }\n\
             int pgl_floor_mod(int left, int right) {\n\
             \u{20} if (right == 0) return 0;\n\
             \u{20} if (left == (-2147483647 - 1) && right == -1) return 0;\n\
             \u{20} int remainder = left % right;\n\
             \u{20} return remainder != 0 && ((remainder < 0) != (right < 0)) ? remainder + right : remainder;\n\
             }\n\
             int pgl_trunc_mod(int left, int right) {\n\
             \u{20} if (right == 0) return 0;\n\
             \u{20} if (left == (-2147483647 - 1) && right == -1) return 0;\n\
             \u{20} return left % right;\n\
             }\n\n",
        );
    }

    fn emit_constants(&mut self) -> Result<(), EmitError> {
        for constant in &self.module.constants {
            let value = self.expression(&constant.value)?;
            self.line(&format!(
                "#define {} ({value})",
                constant_name(&constant.name)
            ));
        }
        if !self.module.constants.is_empty() {
            self.output.push('\n');
        }
        Ok(())
    }

    fn emit_function_prototypes(&mut self) -> Result<(), EmitError> {
        for function in &self.module.functions {
            let result = glsl_type(&function.result)?;
            let parameters = function
                .params
                .iter()
                .map(|parameter| {
                    Ok(format!(
                        "{} {}",
                        glsl_type(&parameter.ty)?,
                        parameter_name(&parameter.name)
                    ))
                })
                .collect::<Result<Vec<_>, EmitError>>()?
                .join(", ");
            self.line(&format!(
                "{result} {}({parameters});",
                function_name(&function.name)
            ));
        }
        if !self.module.functions.is_empty() {
            self.output.push('\n');
        }
        Ok(())
    }

    fn emit_functions(&mut self) -> Result<(), EmitError> {
        for function in &self.module.functions {
            let parameters = function
                .params
                .iter()
                .map(|parameter| {
                    Ok(format!(
                        "{} {}",
                        glsl_type(&parameter.ty)?,
                        parameter_name(&parameter.name)
                    ))
                })
                .collect::<Result<Vec<_>, EmitError>>()?
                .join(", ");
            self.line(&format!(
                "{} {}({parameters}) {{",
                glsl_type(&function.result)?,
                function_name(&function.name)
            ));
            self.indent += 1;
            self.push_scope();
            for parameter in &function.params {
                self.bind(&parameter.name, parameter_name(&parameter.name).to_owned());
            }
            self.emit_statements(&function.body)?;
            self.pop_scope();
            self.indent -= 1;
            self.line("}");
            self.output.push('\n');
        }
        Ok(())
    }

    fn emit_entry(&mut self) -> Result<(), EmitError> {
        self.in_entry = true;
        self.line("void main() {");
        self.indent += 1;
        self.push_scope();
        match self.stage {
            ShaderStage::Vertex => {
                for parameter in &self.entry.params {
                    self.bind(&parameter.name, attribute_name(&parameter.name)?.to_owned());
                }
            }
            ShaderStage::Fragment => {
                if let Type::Struct(name) = &self.other_stage.result {
                    let definition = self.struct_definition(name.as_str())?.clone();
                    let parameter =
                        self.entry
                            .params
                            .first()
                            .ok_or_else(|| EmitError::InvalidStageResult {
                                shader: self.shader_name.clone(),
                                stage: "fragment",
                                ty: self.entry.result.clone(),
                            })?;
                    let local = self.fresh_local(&parameter.name);
                    let values = definition
                        .fields
                        .iter()
                        .map(|field| varying_name(&field.name))
                        .collect::<Vec<_>>();
                    self.line(&format!(
                        "{} {local} = {}({});",
                        struct_name(&definition.name),
                        struct_name(&definition.name),
                        values.join(", ")
                    ));
                    self.bind(&parameter.name, local);
                }
            }
        }
        let body = self.entry.body.clone();
        self.emit_statements(&body)?;
        self.pop_scope();
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    fn emit_statements(&mut self, block: &Block) -> Result<(), EmitError> {
        self.push_scope();
        for statement in &block.statements {
            self.statement(statement)?;
        }
        self.pop_scope();
        Ok(())
    }

    fn statement(&mut self, statement: &Statement) -> Result<(), EmitError> {
        match &statement.kind {
            StatementKind::Let { name, ty, init } => {
                let value = self.expression(init)?;
                let local = self.fresh_local(name);
                self.line(&format!("{} {local} = {value};", glsl_type(ty)?));
                self.bind(name, local);
            }
            StatementKind::Assign { target, value } => {
                let target = self.place(target)?;
                let value = self.expression(value)?;
                self.line(&format!("{target} = {value};"));
            }
            StatementKind::Expr(expression) => {
                let expression = self.expression(expression)?;
                self.line(&format!("{expression};"));
            }
            StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                let condition = self.expression(condition)?;
                self.line(&format!("if ({condition}) {{"));
                self.indent += 1;
                self.emit_statements(then_block)?;
                self.indent -= 1;
                if let Some(else_block) = else_block {
                    self.line("} else {");
                    self.indent += 1;
                    self.emit_statements(else_block)?;
                    self.indent -= 1;
                }
                self.line("}");
            }
            StatementKind::While { condition, body } => {
                let condition = self.expression(condition)?;
                self.line(&format!("while ({condition}) {{"));
                self.indent += 1;
                self.emit_statements(body)?;
                self.indent -= 1;
                self.line("}");
            }
            StatementKind::For {
                variable,
                range,
                body,
            } => {
                let start = self.expression(&range.start)?;
                let end = self.expression(&range.end)?;
                let start_local = self.fresh_local("range_start");
                let end_local = self.fresh_local("range_end");
                let index = self.fresh_local("range_index");
                let local = self.fresh_local(variable);
                self.line("{");
                self.indent += 1;
                self.line(&format!("int {start_local} = {start};"));
                self.line(&format!("int {end_local} = {end};"));
                let done = if range.inclusive {
                    let done = self.fresh_local("range_done");
                    self.line(&format!("bool {done} = false;"));
                    self.line(&format!(
                        "for (int {index} = {start_local}; !{done} && {index} <= {end_local}; {index} += {done} ? 0 : 1) {{"
                    ));
                    Some(done)
                } else {
                    self.line(&format!(
                        "for (int {index} = {start_local}; {index} < {end_local}; {index} += 1) {{"
                    ));
                    None
                };
                self.indent += 1;
                if let Some(done) = &done {
                    self.line(&format!("{done} = {index} == {end_local};"));
                }
                self.line(&format!("int {local} = {index};"));
                self.push_scope();
                self.bind(variable, local);
                self.emit_statements(body)?;
                self.pop_scope();
                self.indent -= 1;
                self.line("}");
                self.indent -= 1;
                self.line("}");
            }
            StatementKind::Return(value) => self.emit_return(value.as_ref())?,
            StatementKind::Break => self.line("break;"),
            StatementKind::Continue => self.line("continue;"),
        }
        Ok(())
    }

    fn emit_return(&mut self, value: Option<&Expr>) -> Result<(), EmitError> {
        if !self.in_entry {
            if let Some(value) = value {
                let value = self.expression(value)?;
                self.line(&format!("return {value};"));
            } else {
                self.line("return;");
            }
            return Ok(());
        }
        let Some(value) = value else {
            return Err(EmitError::InvalidStageResult {
                shader: self.shader_name.clone(),
                stage: stage_name(self.stage),
                ty: Type::Unit,
            });
        };
        let value = self.expression(value)?;
        match self.stage {
            ShaderStage::Vertex => {
                match &self.entry.result {
                    Type::Vector(4) => {
                        self.line(&format!("gl_Position = {value};"));
                        self.line("return;");
                    }
                    Type::Struct(name) => {
                        let definition = self.struct_definition(name.as_str())?.clone();
                        let local = self.fresh_local("vertex_result");
                        self.line(&format!(
                            "{} {local} = {value};",
                            struct_name(&definition.name)
                        ));
                        let clip_field = definition.fields.first().ok_or_else(|| {
                            EmitError::InvalidStageResult {
                                shader: self.shader_name.clone(),
                                stage: "vertex",
                                ty: self.entry.result.clone(),
                            }
                        })?;
                        let clip = field_name(&clip_field.name);
                        self.line(&format!("gl_Position = {local}.{clip};"));
                        for field in &definition.fields {
                            self.line(&format!(
                                "{} = {local}.{};",
                                varying_name(&field.name),
                                field_name(&field.name)
                            ));
                        }
                        self.line("return;");
                    }
                    ty => {
                        return Err(EmitError::InvalidStageResult {
                            shader: self.shader_name.clone(),
                            stage: "vertex",
                            ty: ty.clone(),
                        });
                    }
                }
            }
            ShaderStage::Fragment => {
                if self.entry.result != Type::Vector(4) {
                    return Err(EmitError::InvalidStageResult {
                        shader: self.shader_name.clone(),
                        stage: "fragment",
                        ty: self.entry.result.clone(),
                    });
                }
                self.line(&format!("out_color = {value};"));
                self.line("return;");
            }
        }
        Ok(())
    }

    fn place(&mut self, place: &Place) -> Result<String, EmitError> {
        match &place.kind {
            PlaceKind::Variable(name) => self.binding(name),
            PlaceKind::Index { base, index } => Ok(format!(
                "{}[{}]",
                self.expression(base)?,
                self.expression(index)?
            )),
            PlaceKind::Field { base, field } => {
                Ok(format!("{}.{}", self.expression(base)?, field_name(field)))
            }
        }
    }

    fn expression(&mut self, expression: &Expr) -> Result<String, EmitError> {
        match &expression.kind {
            ExprKind::Literal(literal) => literal_value(literal),
            ExprKind::Variable(name) => self.binding(name),
            ExprKind::Constant(name) => Ok(constant_name(name)),
            ExprKind::Binary { op, left, right } => {
                let left = self.expression(left)?;
                let right = self.expression(right)?;
                binary(*op, &left, &right, &expression.ty)
            }
            ExprKind::Unary { op, operand } => {
                let operand = self.expression(operand)?;
                Ok(format!("({}{operand})", unary(*op)))
            }
            ExprKind::Call { target, args } => {
                let args = args
                    .iter()
                    .map(|argument| self.expression(argument))
                    .collect::<Result<Vec<_>, _>>()?;
                match target {
                    CallTarget::Function(name) => {
                        Ok(format!("{}({})", function_name(name), args.join(", ")))
                    }
                    CallTarget::Runtime(operation) => runtime_call(operation.as_str(), &args),
                }
            }
            ExprKind::Index { base, index } => Ok(format!(
                "{}[{}]",
                self.expression(base)?,
                self.expression(index)?
            )),
            ExprKind::Field { base, field } => {
                Ok(format!("{}.{}", self.expression(base)?, field_name(field)))
            }
            ExprKind::Struct { name, fields } => {
                let definition = self.struct_definition(name)?.clone();
                let values = definition
                    .fields
                    .iter()
                    .map(|definition| {
                        fields
                            .iter()
                            .find(|field| field.name == definition.name)
                            .ok_or_else(|| EmitError::MissingStructField {
                                structure: name.clone(),
                                field: definition.name.clone(),
                            })
                            .and_then(|field| self.expression(&field.value))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(format!("{}({})", struct_name(name), values.join(", ")))
            }
            ExprKind::Vector { size, args } => {
                let args = args
                    .iter()
                    .map(|argument| self.expression(argument))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(format!("vec{size}({})", args.join(", ")))
            }
            ExprKind::IsNil(value) => Ok(format!("(({}), false)", self.expression(value)?)),
            ExprKind::IsFalsy(value) if value.ty == Type::Bool => {
                Ok(format!("(!{})", self.expression(value)?))
            }
            ExprKind::IsFalsy(value) => Ok(format!("(({}), false)", self.expression(value)?)),
            ExprKind::ArrayLength(_) => {
                Err(EmitError::UnsupportedExpression("dynamic array length"))
            }
            ExprKind::Array(_) => Err(EmitError::UnsupportedExpression("dynamic array")),
            ExprKind::Map(_) => Err(EmitError::UnsupportedExpression("map")),
        }
    }

    fn struct_definition(&self, name: &str) -> Result<&StructDef, EmitError> {
        self.module
            .structs
            .iter()
            .find(|definition| definition.name == name)
            .ok_or_else(|| EmitError::MissingStruct(name.to_owned()))
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn bind(&mut self, source: &str, generated: String) {
        self.scopes
            .last_mut()
            .expect("bindings are emitted inside a scope")
            .insert(source.to_owned(), generated);
    }

    fn binding(&self, source: &str) -> Result<String, EmitError> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(source))
            .cloned()
            .ok_or_else(|| EmitError::UnknownBinding(source.to_owned()))
    }

    fn fresh_local(&mut self, source: &str) -> String {
        let local = format!("{}_{}", encoded_name("pgl_l", source), self.local_counter);
        self.local_counter += 1;
        local
    }

    fn line(&mut self, value: &str) {
        let _ = writeln!(self.output, "{}{value}", "  ".repeat(self.indent));
    }
}

fn glsl_type(ty: &Type) -> Result<String, EmitError> {
    match ty {
        Type::Unit => Ok("void".to_owned()),
        Type::Int => Ok("int".to_owned()),
        Type::Float => Ok("float".to_owned()),
        Type::Bool => Ok("bool".to_owned()),
        Type::Vector(size) => Ok(format!("vec{size}")),
        Type::Matrix(size) => Ok(format!("mat{size}")),
        Type::Struct(name) => Ok(struct_name(name.as_str())),
        Type::Opaque(polygl_hir::OpaqueType::Texture) => Ok("sampler2D".to_owned()),
        Type::Str | Type::Array(_) | Type::Map(_) | Type::Option(_) | Type::Opaque(_) => {
            Err(EmitError::UnsupportedExpression("non-GPU type"))
        }
    }
}

fn literal_value(literal: &Literal) -> Result<String, EmitError> {
    match literal {
        Literal::Int(value) => Ok(value.to_string()),
        Literal::Float(value) => float_literal(*value),
        Literal::Bool(value) => Ok(value.to_string()),
        Literal::Str(_) => Err(EmitError::UnsupportedExpression("string literal")),
        Literal::None => Err(EmitError::UnsupportedExpression("nil literal")),
    }
}

fn float_literal(value: f64) -> Result<String, EmitError> {
    let value = value as f32;
    if !value.is_finite() {
        return Err(EmitError::NonFiniteFloat(f64::from(value)));
    }
    let mut literal = value.to_string();
    if !literal.contains(['.', 'e', 'E']) {
        literal.push_str(".0");
    }
    Ok(literal)
}

fn binary(op: BinaryOp, left: &str, right: &str, ty: &Type) -> Result<String, EmitError> {
    let expression = match op {
        BinaryOp::IntegerDivide => format!("pgl_int_div({left}, {right})"),
        BinaryOp::FloorRemainder if *ty == Type::Int => {
            format!("pgl_floor_mod({left}, {right})")
        }
        BinaryOp::FloorRemainder => format!("mod({left}, {right})"),
        BinaryOp::TruncatingRemainder if *ty == Type::Int => {
            format!("pgl_trunc_mod({left}, {right})")
        }
        BinaryOp::TruncatingRemainder if *ty == Type::Float => {
            format!("({left} - trunc({left} / {right}) * {right})")
        }
        BinaryOp::StringConcat => {
            return Err(EmitError::UnsupportedExpression("string concatenation"));
        }
        op => format!("({left} {} {right})", binary_operator(op)),
    };
    Ok(expression)
}

fn binary_operator(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Subtract => "-",
        BinaryOp::Multiply => "*",
        BinaryOp::FloatDivide => "/",
        BinaryOp::TruncatingRemainder => "%",
        BinaryOp::Equal => "==",
        BinaryOp::NotEqual => "!=",
        BinaryOp::Less => "<",
        BinaryOp::LessEqual => "<=",
        BinaryOp::Greater => ">",
        BinaryOp::GreaterEqual => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
        BinaryOp::IntegerDivide | BinaryOp::FloorRemainder | BinaryOp::StringConcat => {
            unreachable!("special binary operations are emitted separately")
        }
    }
}

const fn unary(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Negate => "-",
        UnaryOp::Not => "!",
    }
}

fn runtime_call(operation: &str, args: &[String]) -> Result<String, EmitError> {
    match (operation, args) {
        ("time", []) => Ok("u_time".to_owned()),
        ("floorToInt", [value]) => Ok(format!("int(floor({value}))")),
        ("roundToInt", [value]) => Ok(format!(
            "int(({value}) < 0.0 ? ceil(({value}) - 0.5) : floor(({value}) + 0.5))"
        )),
        ("truncToInt", [value]) => Ok(format!("int(trunc({value}))")),
        _ => Err(EmitError::UnsupportedRuntimeOp(operation.to_owned())),
    }
}

fn attribute_name(name: &str) -> Result<&'static str, EmitError> {
    match name {
        "position" => Ok("a_position"),
        "normal" => Ok("a_normal"),
        "uv" => Ok("a_uv"),
        "color" => Ok("a_color"),
        _ => Err(EmitError::InvalidAttribute(name.to_owned())),
    }
}

fn attribute_location(name: &str) -> Result<u8, EmitError> {
    match name.as_bytes() {
        b"position" => Ok(0),
        b"normal" => Ok(1),
        b"uv" => Ok(2),
        b"color" => Ok(3),
        _ => Err(EmitError::InvalidAttribute(name.to_owned())),
    }
}

fn function_name(name: &str) -> String {
    encoded_name("pgl_fn", name)
}

fn constant_name(name: &str) -> String {
    encoded_name("pgl_c", name)
}

fn parameter_name(name: &str) -> String {
    encoded_name("pgl_p", name)
}

fn struct_name(name: &str) -> String {
    encoded_name("pgl_s", name)
}

fn field_name(name: &str) -> String {
    encoded_name("pgl_f", name)
}

fn varying_name(name: &str) -> String {
    encoded_name("v", name)
}

fn encoded_name(prefix: &str, name: &str) -> String {
    let mut output = prefix.to_owned();
    for byte in name.as_bytes() {
        let _ = write!(output, "_{byte:02x}");
    }
    output
}

const fn interpolation_qualifier(ty: &Type) -> &'static str {
    match ty {
        Type::Int | Type::Bool => "flat ",
        _ => "",
    }
}

const fn stage_name(stage: ShaderStage) -> &'static str {
    match stage {
        ShaderStage::Vertex => "vertex",
        ShaderStage::Fragment => "fragment",
    }
}

fn block_uses_time(block: &Block) -> bool {
    block
        .statements
        .iter()
        .any(|statement| match &statement.kind {
            StatementKind::Let { init, .. } | StatementKind::Expr(init) => expr_uses_time(init),
            StatementKind::Assign { target, value } => {
                place_uses_time(target) || expr_uses_time(value)
            }
            StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                expr_uses_time(condition)
                    || block_uses_time(then_block)
                    || else_block.as_ref().is_some_and(block_uses_time)
            }
            StatementKind::While { condition, body } => {
                expr_uses_time(condition) || block_uses_time(body)
            }
            StatementKind::For { range, body, .. } => {
                expr_uses_time(&range.start) || expr_uses_time(&range.end) || block_uses_time(body)
            }
            StatementKind::Return(value) => value.as_ref().is_some_and(expr_uses_time),
            StatementKind::Break | StatementKind::Continue => false,
        })
}

fn place_uses_time(place: &Place) -> bool {
    match &place.kind {
        PlaceKind::Variable(_) => false,
        PlaceKind::Index { base, index } => expr_uses_time(base) || expr_uses_time(index),
        PlaceKind::Field { base, .. } => expr_uses_time(base),
    }
}

fn expr_uses_time(expression: &Expr) -> bool {
    match &expression.kind {
        ExprKind::Call { target, args } => {
            matches!(target, CallTarget::Runtime(operation) if operation.as_str() == "time")
                || args.iter().any(expr_uses_time)
        }
        ExprKind::Binary { left, right, .. }
        | ExprKind::Index {
            base: left,
            index: right,
        } => expr_uses_time(left) || expr_uses_time(right),
        ExprKind::Unary { operand, .. }
        | ExprKind::Field { base: operand, .. }
        | ExprKind::ArrayLength(operand)
        | ExprKind::IsNil(operand)
        | ExprKind::IsFalsy(operand) => expr_uses_time(operand),
        ExprKind::Array(items) | ExprKind::Vector { args: items, .. } => {
            items.iter().any(expr_uses_time)
        }
        ExprKind::Map(entries) => entries
            .iter()
            .any(|entry| expr_uses_time(&entry.key) || expr_uses_time(&entry.value)),
        ExprKind::Struct { fields, .. } => fields.iter().any(|field| expr_uses_time(&field.value)),
        ExprKind::Literal(_) | ExprKind::Variable(_) | ExprKind::Constant(_) => false,
    }
}
