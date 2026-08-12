use std::collections::BTreeMap;

use polygl_span::{
    Applicability, Diagnostics, RenderError, Severity, SourceFile, StructuredDiagnostic,
};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DiagnosticFormat {
    #[default]
    Human,
    Json,
    Sarif,
    Lsp,
}

pub(crate) fn render(
    diagnostics: &Diagnostics,
    source: &SourceFile,
    format: DiagnosticFormat,
    color: bool,
) -> Result<String, RenderError> {
    match format {
        DiagnosticFormat::Human => diagnostics.render_with_color(source, color),
        DiagnosticFormat::Json => render_json(diagnostics.structured(source)?),
        DiagnosticFormat::Sarif => render_sarif(diagnostics.structured(source)?),
        DiagnosticFormat::Lsp => render_lsp(diagnostics.structured(source)?),
    }
}

fn render_json(diagnostics: Vec<StructuredDiagnostic>) -> Result<String, RenderError> {
    encode(&json!({
        "schemaVersion": 1,
        "diagnostics": diagnostics.iter().map(structured_json).collect::<Vec<_>>(),
    }))
}

fn structured_json(diagnostic: &StructuredDiagnostic) -> Value {
    json!({
        "code": diagnostic.code.as_str(),
        "severity": severity_name(diagnostic.severity),
        "message": diagnostic.message,
        "source": diagnostic.source,
        "range": range_json(diagnostic.primary_range),
        "labels": diagnostic.labels.iter().map(|label| json!({
            "message": label.message,
            "range": range_json(label.range),
        })).collect::<Vec<_>>(),
        "notes": diagnostic.notes,
        "fixes": diagnostic.suggestions.iter().map(|suggestion| json!({
            "message": suggestion.message,
            "range": range_json(suggestion.range),
            "replacement": suggestion.replacement,
            "applicability": suggestion.applicability.as_str(),
        })).collect::<Vec<_>>(),
    })
}

fn render_sarif(diagnostics: Vec<StructuredDiagnostic>) -> Result<String, RenderError> {
    let mut rules = BTreeMap::new();
    for diagnostic in &diagnostics {
        rules.entry(diagnostic.code).or_insert_with(|| {
            let metadata = diagnostic.code.metadata();
            json!({
                "id": diagnostic.code.as_str(),
                "name": metadata.title,
                "shortDescription": { "text": metadata.title },
                "fullDescription": { "text": metadata.description },
                "helpUri": format!("https://ydah.github.io/polygl/errors/#{}", diagnostic.code.as_str().to_ascii_lowercase()),
                "properties": {
                    "introduced": metadata.introduced,
                    "producer": metadata.producer,
                },
            })
        });
    }
    let rule_indices = rules
        .keys()
        .enumerate()
        .map(|(index, code)| (*code, index))
        .collect::<BTreeMap<_, _>>();
    let results = diagnostics
        .iter()
        .map(|diagnostic| {
            let fixes = diagnostic
                .suggestions
                .iter()
                .filter_map(|suggestion| {
                    let replacement = suggestion.replacement.as_ref()?;
                    (suggestion.applicability == Applicability::MachineApplicable).then(|| {
                        json!({
                            "description": { "text": suggestion.message },
                            "artifactChanges": [{
                                "artifactLocation": { "uri": diagnostic.source },
                                "replacements": [{
                                    "deletedRegion": sarif_region(suggestion.range),
                                    "insertedContent": { "text": replacement },
                                }],
                            }],
                        })
                    })
                })
                .collect::<Vec<_>>();
            let mut result = json!({
                "ruleId": diagnostic.code.as_str(),
                "ruleIndex": rule_indices[&diagnostic.code],
                "level": sarif_level(diagnostic.severity),
                "message": { "text": diagnostic.message },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": diagnostic.source },
                        "region": sarif_region(diagnostic.primary_range),
                    },
                }],
            });
            if !fixes.is_empty() {
                result["fixes"] = Value::Array(fixes);
            }
            result
        })
        .collect::<Vec<_>>();
    encode(&json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": {
                "name": "PolyGL",
                "informationUri": "https://ydah.github.io/polygl/",
                "rules": rules.into_values().collect::<Vec<_>>(),
            }},
            "results": results,
        }],
    }))
}

fn render_lsp(diagnostics: Vec<StructuredDiagnostic>) -> Result<String, RenderError> {
    encode(&Value::Array(
        diagnostics
            .iter()
            .map(|diagnostic| {
                json!({
                    "range": lsp_range(diagnostic.primary_range),
                    "severity": match diagnostic.severity {
                        Severity::Error => 1,
                        Severity::Warning => 2,
                        Severity::Note => 3,
                    },
                    "code": diagnostic.code.as_str(),
                    "source": "polygl",
                    "message": diagnostic.message,
                    "data": {
                        "source": diagnostic.source,
                        "notes": diagnostic.notes,
                        "fixes": diagnostic.suggestions.iter().map(|suggestion| json!({
                            "title": suggestion.message,
                            "edit": {
                                "range": lsp_range(suggestion.range),
                                "newText": suggestion.replacement,
                            },
                            "applicability": suggestion.applicability.as_str(),
                        })).collect::<Vec<_>>(),
                    },
                })
            })
            .collect(),
    ))
}

fn range_json(range: polygl_span::DiagnosticRange) -> Value {
    json!({
        "bytes": { "start": range.byte_start, "end": range.byte_end },
        "utf16": lsp_range(range),
    })
}

fn lsp_range(range: polygl_span::DiagnosticRange) -> Value {
    json!({
        "start": { "line": range.start.line, "character": range.start.character },
        "end": { "line": range.end.line, "character": range.end.character },
    })
}

fn sarif_region(range: polygl_span::DiagnosticRange) -> Value {
    json!({
        "startLine": range.start.line + 1,
        "startColumn": range.start.character + 1,
        "endLine": range.end.line + 1,
        "endColumn": range.end.character + 1,
        "byteOffset": range.byte_start,
        "byteLength": range.byte_end - range.byte_start,
    })
}

const fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Note => "note",
    }
}

const fn sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Note => "note",
    }
}

fn encode(value: &Value) -> Result<String, RenderError> {
    let mut encoded = serde_json::to_string_pretty(value)
        .expect("serializing structured diagnostics to JSON cannot fail");
    encoded.push('\n');
    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use polygl_span::{Diagnostic, Diagnostics, Severity, SourceFile, SourceId, Suggestion};

    use super::{DiagnosticFormat, render};

    fn diagnostics() -> (SourceFile, Diagnostics) {
        let source = SourceFile::new(SourceId::new(1), "unicode.php", "😀 $x == 1");
        let operator = source.span(8, 10).unwrap();
        let mut diagnostics = Diagnostics::new();
        diagnostics.push(
            Diagnostic::new(Severity::Error, "E0302", "loose equality", operator)
                .with_suggestion(Suggestion::new(operator, "===", "use strict equality"))
                .with_suggestion(Suggestion::rewrite(operator, "rewrite the comparison")),
        );
        (source, diagnostics)
    }

    #[test]
    fn emits_json_without_parsing_human_text() {
        let (source, diagnostics) = diagnostics();
        let value: serde_json::Value = serde_json::from_str(
            &render(&diagnostics, &source, DiagnosticFormat::Json, false).unwrap(),
        )
        .unwrap();
        assert_eq!(value["diagnostics"][0]["code"], "E0302");
        assert_eq!(
            value["diagnostics"][0]["range"]["utf16"]["start"]["character"],
            6
        );
        assert_eq!(
            value["diagnostics"][0]["fixes"].as_array().unwrap().len(),
            2
        );
    }

    #[test]
    fn emits_valid_sarif_with_only_safe_automatic_fixes() {
        let (source, diagnostics) = diagnostics();
        let value: serde_json::Value = serde_json::from_str(
            &render(&diagnostics, &source, DiagnosticFormat::Sarif, false).unwrap(),
        )
        .unwrap();
        assert_eq!(value["version"], "2.1.0");
        assert_eq!(value["runs"][0]["results"][0]["ruleId"], "E0302");
        assert_eq!(
            value["runs"][0]["results"][0]["fixes"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn emits_lsp_utf16_ranges_and_fix_data() {
        let (source, diagnostics) = diagnostics();
        let value: serde_json::Value = serde_json::from_str(
            &render(&diagnostics, &source, DiagnosticFormat::Lsp, false).unwrap(),
        )
        .unwrap();
        assert_eq!(value[0]["range"]["start"]["character"], 6);
        assert_eq!(value[0]["severity"], 1);
        assert_eq!(value[0]["data"]["fixes"].as_array().unwrap().len(), 2);
    }
}
