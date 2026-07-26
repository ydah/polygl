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

    pub(crate) fn take_parameter(
        &mut self,
        name: &str,
        definition_offset: usize,
        source: &SourceFile,
    ) -> Option<TypeExpr> {
        self.take(name, definition_offset, source)
    }

    pub(crate) fn report_unused(&self, diagnostics: &mut Diagnostics) {
        for annotation in self.entries.iter().filter(|annotation| !annotation.used) {
            diagnostics.push(
                Diagnostic::new(
                    Severity::Error,
                    "E0314",
                    format!(
                        "type annotation for `{}` does not match a later declaration",
                        annotation.name
                    ),
                    annotation.span,
                )
                .with_suggestion(Suggestion::rewrite(
                    annotation.span,
                    format!(
                        "move this annotation immediately before the declaration of `{}`",
                        annotation.name
                    ),
                )),
            );
        }
    }
}

fn directive_gap(gap: &str) -> bool {
    gap.lines().all(|line| {
        let trimmed = line.trim();
        trimmed.is_empty() || trimmed.starts_with("# @pgl ")
    })
}

pub(crate) fn parse_annotations(
    source: &SourceFile,
    parsed: &ruby_prism::ParseResult<'_>,
    diagnostics: &mut Diagnostics,
) -> Annotations {
    let mut result = Annotations::default();
    for comment in parsed.comments() {
        let location = comment.location();
        let line_start = source.text()[..location.start_offset()]
            .rfind('\n')
            .map_or(0, |offset| offset + 1);
        if !source.text()[line_start..location.start_offset()]
            .trim()
            .is_empty()
        {
            continue;
        }
        let raw = String::from_utf8_lossy(comment.text());
        let Some(rest) = raw
            .trim()
            .strip_prefix('#')
            .map(str::trim_start)
            .and_then(|comment| comment.strip_prefix("@pgl"))
        else {
            continue;
        };
        if rest
            .chars()
            .next()
            .is_some_and(|character| !character.is_whitespace())
        {
            continue;
        }
        let directive = rest.trim();
        let span = source
            .span(location.start_offset(), location.end_offset())
            .expect("Prism comments must use source byte boundaries");
        let Some((name, type_name)) = directive.split_once(':') else {
            invalid_annotation(
                diagnostics,
                span,
                "type annotation must have the form `# @pgl name: type`",
            );
            continue;
        };
        let name = name.trim();
        let type_name = type_name.trim();
        if !is_portable_identifier(name) {
            invalid_annotation(
                diagnostics,
                span,
                "annotation target must be a local identifier",
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
            offset: location.start_offset(),
            end_offset: location.end_offset(),
            used: false,
        });
    }
    result
}

fn invalid_annotation(diagnostics: &mut Diagnostics, span: Span, message: &str) {
    diagnostics.push(
        Diagnostic::new(Severity::Error, "E0314", message, span).with_suggestion(
            Suggestion::rewrite(span, "use `# @pgl name: float` before a declaration"),
        ),
    );
}
