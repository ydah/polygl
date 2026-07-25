use crate::{
    ConstDef, DomainHint, EntryPoint, FieldDef, Function, Item, Module, OpaqueType, Param,
    StructDef, TypeExpr, TypeKind,
};

mod expr;
mod stmt;

#[must_use]
pub fn dump(module: &Module) -> String {
    let mut dumper = Dumper::default();
    dumper.line("module");
    dumper.open();
    for item in &module.items {
        dumper.item(item);
    }
    dumper.close();
    dumper.output
}

#[must_use]
pub fn normalized_dump(module: &Module) -> String {
    dump(&module.normalized())
}

#[derive(Default)]
struct Dumper {
    output: String,
    indent: usize,
}

impl Dumper {
    fn item(&mut self, item: &Item) {
        match item {
            Item::Function(item) => self.function(item),
            Item::Struct(item) => self.struct_def(item),
            Item::Const(item) => self.const_def(item),
            Item::Entry(item) => self.entry(item),
        }
    }

    fn function(&mut self, function: &Function) {
        let params = parameters(&function.params);
        let result = function
            .return_type
            .as_ref()
            .map(|ty| format!(" -> {}", type_name(ty)))
            .unwrap_or_default();
        self.line(format!(
            "fn {}({params}){result} [{}]",
            function.name,
            domain_name(function.domain)
        ));
        self.block(&function.body);
    }

    fn struct_def(&mut self, definition: &StructDef) {
        self.line(format!("struct {}", definition.name));
        self.open();
        for field in &definition.fields {
            self.field(field);
        }
        for method in &definition.methods {
            self.function(method);
        }
        self.close();
    }

    fn field(&mut self, field: &FieldDef) {
        let ty = field
            .ty
            .as_ref()
            .map(|ty| format!(": {}", type_name(ty)))
            .unwrap_or_default();
        self.line(format!("field {}{ty};", field.name));
    }

    fn const_def(&mut self, definition: &ConstDef) {
        let ty = definition
            .ty
            .as_ref()
            .map(|ty| format!(": {}", type_name(ty)))
            .unwrap_or_default();
        let value = self.expression(&definition.value);
        self.line(format!("const {}{ty} = {value};", definition.name));
    }

    fn entry(&mut self, entry: &EntryPoint) {
        self.line(format!(
            "entry {}({}) [{}]",
            entry.kind.canonical_name(),
            parameters(&entry.params),
            domain_name(entry.kind.domain())
        ));
        self.block(&entry.body);
    }

    fn line(&mut self, value: impl AsRef<str>) {
        for _ in 0..self.indent {
            self.output.push_str("  ");
        }
        self.output.push_str(value.as_ref());
        self.output.push('\n');
    }

    fn open(&mut self) {
        self.line("{");
        self.indent += 1;
    }

    fn close(&mut self) {
        self.indent -= 1;
        self.line("}");
    }
}

fn parameters(params: &[Param]) -> String {
    params
        .iter()
        .map(|param| {
            param.ty.as_ref().map_or_else(
                || param.name.to_string(),
                |ty| format!("{}: {}", param.name, type_name(ty)),
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn type_name(ty: &TypeExpr) -> String {
    match &ty.kind {
        TypeKind::Unit => "void".to_owned(),
        TypeKind::Int => "int".to_owned(),
        TypeKind::Float => "float".to_owned(),
        TypeKind::Bool => "bool".to_owned(),
        TypeKind::Str => "str".to_owned(),
        TypeKind::Array(inner) => format!("{}[]", type_name(inner)),
        TypeKind::Map(inner) => format!("Map<str, {}>", type_name(inner)),
        TypeKind::Option(inner) => format!("Option<{}>", type_name(inner)),
        TypeKind::Struct(name) => name.to_string(),
        TypeKind::Vector(size) => format!("vec{size}"),
        TypeKind::Matrix(size) => format!("mat{size}"),
        TypeKind::Opaque(kind) => opaque_name(*kind).to_owned(),
    }
}

const fn domain_name(domain: DomainHint) -> &'static str {
    match domain {
        DomainHint::Auto => "auto",
        DomainHint::Host => "host",
        DomainHint::Gpu => "gpu",
    }
}

const fn opaque_name(kind: OpaqueType) -> &'static str {
    match kind {
        OpaqueType::Mesh => "Mesh",
        OpaqueType::Node => "Node",
        OpaqueType::Material => "Material",
        OpaqueType::Texture => "Texture",
    }
}
