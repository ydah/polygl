use mago_span::HasSpan;
use mago_syntax::cst::{Program, TriviaKind};
use polygl_hir::{OpaqueType, Symbol, TypeExpr, TypeKind};
use polygl_span::{Diagnostic, Diagnostics, Severity, SourceFile, Span, Suggestion};

#[derive(Clone, Debug)]
struct Annotation {
    name: String,
    ty: TypeExpr,
    span: Span,
    offset: usize,
    end_offset: usize,
    used: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Annotations {
    entries: Vec<Annotation>,
}

impl Annotations {
    pub(crate) fn take(
        &mut self,
        name: &str,
        declaration_offset: usize,
        source: &SourceFile,
    ) -> Option<TypeExpr> {
        let candidate = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, annotation)| {
                !annotation.used
                    && annotation.name == name
                    && annotation.offset <= declaration_offset
                    && directive_gap(&source.text()[annotation.end_offset..declaration_offset])
            })
            .max_by_key(|(_, annotation)| annotation.offset)
            .map(|(index, _)| index)?;
        self.entries.get_mut(candidate).map(|annotation| {
            annotation.used = true;
            annotation.ty.clone()
        })
    }

    pub(crate) fn report_unused(&self, diagnostics: &mut Diagnostics) {
        for annotation in self.entries.iter().filter(|annotation| !annotation.used) {
            diagnostics.push(
                Diagnostic::new(
                    Severity::Error,
                    "E0314",
                    format!(
                        "type annotation for `${}` does not match a later declaration",
                        annotation.name
                    ),
                    annotation.span,
                )
                .with_suggestion(Suggestion::rewrite(
                    annotation.span,
                    format!(
                        "move this annotation immediately before the declaration of `${}`",
                        annotation.name
                    ),
                )),
            );
        }
    }
}

fn directive_gap(gap: &str) -> bool {
    let mut remaining = gap.trim_start();
    while !remaining.is_empty() {
        let Some(docblock) = remaining.strip_prefix("/**") else {
            return false;
        };
        let Some(end) = docblock.find("*/") else {
            return false;
        };
        if !docblock[..end].contains("@pgl") {
            return false;
        }
        remaining = docblock[end + 2..].trim_start();
    }
    true
}

pub(crate) fn parse_annotations(
    source: &SourceFile,
    program: &Program<'_>,
    diagnostics: &mut Diagnostics,
) -> Annotations {
    let mut result = Annotations::default();
    for trivia in program
        .trivia
        .iter()
        .filter(|trivia| trivia.kind == TriviaKind::DocBlockComment)
    {
        let raw = String::from_utf8_lossy(trivia.value);
        for line in annotation_lines(&raw) {
            let Some(rest) = line.strip_prefix("@pgl") else {
                continue;
            };
            if rest
                .chars()
                .next()
                .is_some_and(|character| !character.is_whitespace())
            {
                continue;
            }
            let span = source
                .span(
                    trivia.span.start_offset() as usize,
                    trivia.span.end_offset() as usize,
                )
                .expect("Mago trivia must use source byte boundaries");
            let Some((name, type_name)) = rest.trim().split_once(':') else {
                invalid_annotation(
                    diagnostics,
                    span,
                    "type annotation must have the form `/** @pgl $name: type */`",
                );
                continue;
            };
            let name = name.trim().strip_prefix('$').unwrap_or_default();
            let type_name = type_name.trim();
            if !valid_name(name) {
                invalid_annotation(
                    diagnostics,
                    span,
                    "annotation target must be a directly named PHP local",
                );
                continue;
            }
            let Some(kind) = parse_type(type_name, span) else {
                invalid_annotation(
                    diagnostics,
                    span,
                    "unknown @pgl type; use a Common Core type such as int, float, bool, or str",
                );
                continue;
            };
            result.entries.push(Annotation {
                name: name.to_owned(),
                ty: TypeExpr::new(kind, span),
                span,
                offset: trivia.span.start_offset() as usize,
                end_offset: trivia.span.end_offset() as usize,
                used: false,
            });
        }
    }
    result
}

fn annotation_lines(raw: &str) -> impl Iterator<Item = &str> {
    raw.lines().map(|line| {
        line.trim()
            .trim_start_matches("/**")
            .trim_start_matches('*')
            .trim_end_matches("*/")
            .trim()
    })
}

fn parse_type(value: &str, span: Span) -> Option<TypeKind> {
    if let Some(element) = value.strip_suffix("[]") {
        return parse_type(element.trim(), span)
            .map(|kind| TypeKind::Array(Box::new(TypeExpr::new(kind, span))));
    }
    if let Some(inner) = generic_argument(value, "Option") {
        return parse_type(inner, span)
            .map(|kind| TypeKind::Option(Box::new(TypeExpr::new(kind, span))));
    }
    if let Some(inner) = generic_argument(value, "Map") {
        let value = inner.strip_prefix("str,")?.trim();
        return parse_type(value, span)
            .map(|kind| TypeKind::Map(Box::new(TypeExpr::new(kind, span))));
    }
    match value {
        "int" => Some(TypeKind::Int),
        "float" => Some(TypeKind::Float),
        "bool" => Some(TypeKind::Bool),
        "str" => Some(TypeKind::Str),
        "vec2" => Some(TypeKind::Vector(2)),
        "vec3" => Some(TypeKind::Vector(3)),
        "vec4" => Some(TypeKind::Vector(4)),
        "mat2" => Some(TypeKind::Matrix(2)),
        "mat3" => Some(TypeKind::Matrix(3)),
        "mat4" => Some(TypeKind::Matrix(4)),
        "Mesh" => Some(TypeKind::Opaque(OpaqueType::Mesh)),
        "Node" => Some(TypeKind::Opaque(OpaqueType::Node)),
        "Material" => Some(TypeKind::Opaque(OpaqueType::Material)),
        "Texture" => Some(TypeKind::Opaque(OpaqueType::Texture)),
        name if name.chars().next().is_some_and(char::is_uppercase) && valid_name(name) => {
            Some(TypeKind::Struct(Symbol::new(name)))
        }
        _ => None,
    }
}

fn generic_argument<'a>(value: &'a str, outer: &str) -> Option<&'a str> {
    value
        .strip_prefix(outer)?
        .strip_prefix('<')?
        .strip_suffix('>')
        .map(str::trim)
}

fn valid_name(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|first| first == '_' || first.is_alphabetic())
        && chars.all(|character| character == '_' || character.is_alphanumeric())
}

fn invalid_annotation(diagnostics: &mut Diagnostics, span: Span, message: &str) {
    diagnostics.push(
        Diagnostic::new(Severity::Error, "E0314", message, span).with_suggestion(
            Suggestion::rewrite(span, "use `/** @pgl $name: float */` before a declaration"),
        ),
    );
}
