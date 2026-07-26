use mago_span::HasSpan;
use mago_syntax::cst::{Program, TriviaKind};
use polygl_adapter_api::{is_portable_identifier, parse_annotation_type};
use polygl_hir::TypeExpr;
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
            if !is_portable_identifier(name) {
                invalid_annotation(
                    diagnostics,
                    span,
                    "annotation target must be a directly named PHP local",
                );
                continue;
            }
            let Some(ty) = parse_annotation_type(type_name, span) else {
                invalid_annotation(
                    diagnostics,
                    span,
                    "unknown @pgl type; use a Common Core type such as int, float, bool, or str",
                );
                continue;
            };
            result.entries.push(Annotation {
                name: name.to_owned(),
                ty,
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

fn invalid_annotation(diagnostics: &mut Diagnostics, span: Span, message: &str) {
    diagnostics.push(
        Diagnostic::new(Severity::Error, "E0314", message, span).with_suggestion(
            Suggestion::rewrite(span, "use `/** @pgl $name: float */` before a declaration"),
        ),
    );
}
