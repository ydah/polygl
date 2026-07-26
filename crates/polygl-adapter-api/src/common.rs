use polygl_hir::{EntryPointKind, OpaqueType, Symbol, TypeExpr, TypeKind};
use polygl_span::Span;

/// Parses the language-neutral type spelling used by `@pgl` directives.
#[must_use]
pub fn parse_annotation_type(value: &str, span: Span) -> Option<TypeExpr> {
    let kind = if let Some(element) = value.strip_suffix("[]") {
        TypeKind::Array(Box::new(parse_annotation_type(element.trim(), span)?))
    } else if let Some(inner) = generic_argument(value, "Option") {
        TypeKind::Option(Box::new(parse_annotation_type(inner, span)?))
    } else if let Some(inner) = generic_argument(value, "Map") {
        let value = inner.strip_prefix("str,")?.trim();
        TypeKind::Map(Box::new(parse_annotation_type(value, span)?))
    } else {
        match value {
            "int" => TypeKind::Int,
            "float" => TypeKind::Float,
            "bool" => TypeKind::Bool,
            "str" => TypeKind::Str,
            "vec2" => TypeKind::Vector(2),
            "vec3" => TypeKind::Vector(3),
            "vec4" => TypeKind::Vector(4),
            "mat2" => TypeKind::Matrix(2),
            "mat3" => TypeKind::Matrix(3),
            "mat4" => TypeKind::Matrix(4),
            "Mesh" => TypeKind::Opaque(OpaqueType::Mesh),
            "Node" => TypeKind::Opaque(OpaqueType::Node),
            "Material" => TypeKind::Opaque(OpaqueType::Material),
            "Texture" => TypeKind::Opaque(OpaqueType::Texture),
            name if name.chars().next().is_some_and(char::is_uppercase)
                && is_portable_identifier(name) =>
            {
                TypeKind::Struct(Symbol::new(name))
            }
            _ => return None,
        }
    };
    Some(TypeExpr::new(kind, span))
}

/// Returns the canonical entry point kind shared by every adapter.
#[must_use]
pub fn canonical_entry_kind(name: &str) -> Option<EntryPointKind> {
    match name {
        "setup" => Some(EntryPointKind::Setup),
        "frame" => Some(EntryPointKind::Frame),
        "on_event" => Some(EntryPointKind::OnEvent),
        _ => name
            .strip_prefix("vertex_")
            .filter(|name| !name.is_empty())
            .map(|name| EntryPointKind::Vertex(Symbol::new(name)))
            .or_else(|| {
                name.strip_prefix("fragment_")
                    .filter(|name| !name.is_empty())
                    .map(|name| EntryPointKind::Fragment(Symbol::new(name)))
            }),
    }
}

/// Returns the HIR function name reserved for a struct-like class constructor.
#[must_use]
pub fn constructor_function_name(class_name: &str) -> String {
    format!("{class_name}::new")
}

/// Recognizes the canonical Common Core vector constructor spellings.
#[must_use]
pub const fn vector_constructor_size(name: &str) -> Option<u8> {
    match name.as_bytes() {
        b"vec2" => Some(2),
        b"vec3" => Some(3),
        b"vec4" => Some(4),
        _ => None,
    }
}

/// Resolves the fixed automatic shader uniform names from the shader ABI.
#[must_use]
pub fn automatic_uniform_type(name: &str, span: Span) -> Option<TypeExpr> {
    let kind = match name {
        "u_time" => TypeKind::Float,
        "u_resolution" => TypeKind::Vector(2),
        "u_model" | "u_view" | "u_proj" => TypeKind::Matrix(4),
        _ => return None,
    };
    Some(TypeExpr::new(kind, span))
}

#[must_use]
pub fn is_portable_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|first| first == '_' || first.is_alphabetic())
        && chars.all(|character| character == '_' || character.is_alphanumeric())
}

fn generic_argument<'a>(value: &'a str, outer: &str) -> Option<&'a str> {
    value
        .strip_prefix(outer)?
        .strip_prefix('<')?
        .strip_suffix('>')
        .map(str::trim)
}

#[cfg(test)]
mod tests {
    use polygl_hir::{EntryPointKind, OpaqueType, Symbol, TypeKind};
    use polygl_span::{SourceFile, SourceId, Span};

    use super::{
        automatic_uniform_type, canonical_entry_kind, constructor_function_name,
        parse_annotation_type, vector_constructor_size,
    };

    fn span() -> Span {
        SourceFile::new(SourceId::new(1), "test", "annotation")
            .span(4, 9)
            .unwrap()
    }

    #[test]
    fn parses_nested_portable_annotation_types() {
        let parsed = parse_annotation_type("Option<Map<str, Mesh[]>>", span()).unwrap();
        assert_eq!(
            parsed.kind,
            TypeKind::Option(Box::new(polygl_hir::TypeExpr::new(
                TypeKind::Map(Box::new(polygl_hir::TypeExpr::new(
                    TypeKind::Array(Box::new(polygl_hir::TypeExpr::new(
                        TypeKind::Opaque(OpaqueType::Mesh),
                        span(),
                    ))),
                    span(),
                ))),
                span(),
            )))
        );
        assert!(parse_annotation_type("Map<int, float>", span()).is_none());
        assert!(parse_annotation_type("void", span()).is_none());
    }

    #[test]
    fn resolves_only_reserved_automatic_uniforms() {
        assert!(matches!(
            automatic_uniform_type("u_model", span()).map(|ty| ty.kind),
            Some(TypeKind::Matrix(4))
        ));
        assert!(matches!(
            automatic_uniform_type("u_resolution", span()).map(|ty| ty.kind),
            Some(TypeKind::Vector(2))
        ));
        assert!(automatic_uniform_type("tint", span()).is_none());
    }

    #[test]
    fn centralizes_generated_names_and_canonical_entries() {
        assert_eq!(
            canonical_entry_kind("fragment_water"),
            Some(EntryPointKind::Fragment(Symbol::new("water")))
        );
        assert_eq!(canonical_entry_kind("fragment_"), None);
        assert_eq!(constructor_function_name("Point"), "Point::new");
        assert_eq!(vector_constructor_size("vec3"), Some(3));
        assert_eq!(vector_constructor_size("mat3"), None);
    }
}
