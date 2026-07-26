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
    pub(crate) fn parse(source: &SourceFile) -> Result<Self, Diagnostics> {
        let mut result = Self::default();
        let mut diagnostics = Diagnostics::new();
        let mut offset = 0;
        for line in source.text().split_inclusive('\n') {
            let line_without_newline = line.trim_end_matches(['\r', '\n']);
            let leading = line_without_newline.len() - line_without_newline.trim_start().len();
            let trimmed = line_without_newline.trim();
            let Some(rest) = trimmed
                .strip_prefix('#')
                .map(str::trim_start)
                .and_then(|comment| comment.strip_prefix("@pgl"))
            else {
                offset += line.len();
                continue;
            };
            if rest
                .chars()
                .next()
                .is_some_and(|character| !character.is_whitespace())
            {
                offset += line.len();
                continue;
            }
            let span = source
                .span(offset + leading, offset + line_without_newline.len())
                .expect("line boundaries are UTF-8 boundaries");
            let directive = rest.trim();
            let Some((name, type_name)) = directive.split_once(':') else {
                invalid(
                    &mut diagnostics,
                    span,
                    "type annotation must have the form `# @pgl $name: type`",
                );
                offset += line.len();
                continue;
            };
            let name = name.trim().strip_prefix('$').unwrap_or(name.trim());
            if !is_portable_identifier(name) {
                invalid(
                    &mut diagnostics,
                    span,
                    "annotation target must be a scalar identifier",
                );
                offset += line.len();
                continue;
            }
            let Some(ty) = parse_annotation_type(type_name.trim(), span) else {
                invalid(
                    &mut diagnostics,
                    span,
                    "unknown @pgl type; use a Common Core type such as int, float, bool, or str",
                );
                offset += line.len();
                continue;
            };
            result.entries.push(Annotation {
                name: name.to_owned(),
                ty,
                span,
                offset: offset + leading,
                end_offset: offset + line_without_newline.len(),
                used: false,
            });
            offset += line.len();
        }
        if diagnostics.has_errors() {
            Err(diagnostics)
        } else {
            Ok(result)
        }
    }

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
                        "move this annotation immediately before the declaration using `${}`",
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

fn invalid(diagnostics: &mut Diagnostics, span: Span, message: &str) {
    diagnostics.push(
        Diagnostic::new(Severity::Error, "E0314", message, span).with_suggestion(
            Suggestion::rewrite(span, "use `# @pgl $name: float` before a declaration"),
        ),
    );
}
