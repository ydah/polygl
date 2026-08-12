use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use polygl_core::{BuildMode, CompileOptions, Compiler, SourceMapMode};
use polygl_span::{SourceFile, SourceId};
use serde::Deserialize;
use serde_json::Value;

use crate::{ConformanceError, ConformanceLanguage};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

#[derive(Deserialize)]
struct SemanticCase {
    id: String,
    language: ConformanceLanguage,
    source: String,
    #[serde(default)]
    debug: bool,
    outcome: SemanticOutcome,
}

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
enum SemanticOutcome {
    Events { value: Value },
    ExecutionError { contains: String },
}

/// Compiles declarative source fixtures to JavaScript and executes their host
/// entry point in a fresh Node process with an observable runtime contract.
pub fn verify_host_semantics(root: &Path) -> Result<usize, ConformanceError> {
    let manifest_path = root.join("semantic-cases.json");
    let raw = fs::read_to_string(&manifest_path)?;
    let cases: Vec<SemanticCase> = serde_json::from_str(&raw).map_err(|error| {
        ConformanceError::InvalidManifest(format!("{}: {error}", manifest_path.display()))
    })?;
    let mut ids = std::collections::HashSet::new();
    for case in &cases {
        if !ids.insert(case.id.as_str()) {
            return Err(ConformanceError::InvalidManifest(format!(
                "duplicate host semantic case `{}`",
                case.id
            )));
        }
        verify_case(root, case)?;
    }
    Ok(cases.len())
}

fn verify_case(root: &Path, case: &SemanticCase) -> Result<(), ConformanceError> {
    let relative = checked_source_path(&case.id, &case.source)?;
    let path = root.join(relative);
    let bytes = fs::read(&path)?;
    let source =
        SourceFile::from_bytes(SourceId::new(0), case.source.clone(), bytes).map_err(|error| {
            ConformanceError::Compile {
                case: case.id.clone(),
                message: error.to_string(),
            }
        })?;
    let options = CompileOptions {
        mode: if case.debug {
            BuildMode::Debug
        } else {
            BuildMode::Release
        },
        source_map: SourceMapMode::None,
        sources_content: false,
        budget: polygl_core::CompileBudget::standard(),
    };
    let compiled = Compiler::standard()
        .compile(&source, case.language.id(), options)
        .map_err(|error| ConformanceError::Compile {
            case: case.id.clone(),
            message: error.render(&source),
        })?;
    let temporary = TemporaryDirectory::new()?;
    fs::write(
        temporary.path.join("app.js"),
        compiled.javascript.javascript,
    )?;
    fs::write(temporary.path.join("runtime.js"), RUNTIME_MOCK)?;
    fs::write(temporary.path.join("runner.mjs"), NODE_RUNNER)?;
    fs::write(
        temporary.path.join("package.json"),
        "{\"private\":true,\"type\":\"module\"}\n",
    )?;
    let output = Command::new("node")
        .arg("runner.mjs")
        .current_dir(&temporary.path)
        .output()
        .map_err(|error| ConformanceError::Compile {
            case: case.id.clone(),
            message: format!("failed to start Node semantic runner: {error}"),
        })?;
    match &case.outcome {
        SemanticOutcome::Events { value } if output.status.success() => {
            let actual: Value = serde_json::from_slice(&output.stdout).map_err(|error| {
                ConformanceError::Compile {
                    case: case.id.clone(),
                    message: format!(
                        "semantic runner returned invalid JSON: {error}: {}",
                        String::from_utf8_lossy(&output.stdout)
                    ),
                }
            })?;
            if &actual != value {
                return Err(ConformanceError::Compile {
                    case: case.id.clone(),
                    message: format!("semantic events differ: expected {value}, got {actual}"),
                });
            }
            Ok(())
        }
        SemanticOutcome::Events { .. } => Err(ConformanceError::Compile {
            case: case.id.clone(),
            message: format!(
                "semantic runner failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        }),
        SemanticOutcome::ExecutionError { contains } if !output.status.success() => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.contains(contains) {
                return Err(ConformanceError::Compile {
                    case: case.id.clone(),
                    message: format!("semantic error did not contain `{contains}`: {stderr}"),
                });
            }
            Ok(())
        }
        SemanticOutcome::ExecutionError { contains } => Err(ConformanceError::Compile {
            case: case.id.clone(),
            message: format!("expected execution error containing `{contains}`, but it succeeded"),
        }),
    }
}

fn checked_source_path<'a>(case: &str, source: &'a str) -> Result<&'a Path, ConformanceError> {
    let path = Path::new(source);
    if path.is_absolute()
        || !path.starts_with("semantic-cases")
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ConformanceError::InvalidManifest(format!(
            "host semantic case `{case}` has non-portable source path `{source}`"
        )));
    }
    Ok(path)
}

struct TemporaryDirectory {
    path: PathBuf,
}

impl TemporaryDirectory {
    fn new() -> Result<Self, ConformanceError> {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "polygl-host-semantics-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        Ok(Self { path })
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

const NODE_RUNNER: &str = r#"
import * as app from "./app.js";
import { events } from "./runtime.js";

if (typeof app.setup !== "function") {
  throw new Error("generated module does not export setup");
}
await app.setup();
const normalized = JSON.stringify(events, (_key, value) => {
  if (typeof value !== "number") return value;
  if (Number.isNaN(value)) return "NaN";
  if (value === Infinity) return "+Infinity";
  if (value === -Infinity) return "-Infinity";
  if (Object.is(value, -0)) return "-0";
  return value;
});
process.stdout.write(normalized);
"#;

const RUNTIME_MOCK: &str = r#"
export const events = [];
export function background(...args) { events.push(["background", ...args]); }
export function floorToInt(value) { return Math.floor(value) | 0; }
export function roundToInt(value) { return Math.round(value) | 0; }
export function truncToInt(value) { return Math.trunc(value) | 0; }
export function mapFromEntries(entries) { return new Map(entries); }
export function mapGet(map, key) {
  if (!map.has(key)) throw new RangeError(`missing map key ${JSON.stringify(key)}`);
  return map.get(key);
}
export function mapSet(map, key, value) { map.set(key, value); return value; }
export function structFromEntries(entries) {
  const value = Object.create(null);
  for (const [key, field] of entries) value[key] = field;
  return value;
}
export function checkIndex(value, index) {
  if (!Number.isInteger(index) || index < 0 || index >= value.length) {
    throw new RangeError(`array index ${index} out of bounds for length ${value.length}`);
  }
}
export function checkedIndex(value, index) { checkIndex(value, index); return value[index]; }
export function requireNonNil(value) {
  if (value == null) throw new TypeError("unexpected absence value");
  return value;
}
"#;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::{fs, process::Command};

    use polygl_core::{BuildMode, CompileBudget, CompileOptions, Compiler, SourceMapMode};
    use polygl_span::{SourceFile, SourceId};

    use super::{TemporaryDirectory, verify_host_semantics};

    #[test]
    fn declarative_host_semantics_pass() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = manifest
            .parent()
            .expect("runner lives directly under conformance");
        assert_eq!(verify_host_semantics(root).unwrap(), 9);
    }

    #[test]
    fn standard_source_map_consumer_resolves_unicode_columns() {
        let text = "def setup\n  text(\"😀\", 0.0, 0.0); background(1.0, 0.0, 0.0)\nend\n";
        let source = SourceFile::new(SourceId::new(0), "unicode.rb", text);
        let compiled = Compiler::standard()
            .compile(
                &source,
                "ruby",
                CompileOptions {
                    mode: BuildMode::Debug,
                    source_map: SourceMapMode::External,
                    sources_content: true,
                    budget: CompileBudget::standard(),
                },
            )
            .unwrap();
        let temporary = TemporaryDirectory::new().unwrap();
        fs::write(
            temporary.path.join("app.js"),
            compiled.javascript.javascript,
        )
        .unwrap();
        fs::write(
            temporary.path.join("app.js.map"),
            compiled.javascript.source_map.unwrap(),
        )
        .unwrap();
        fs::write(
            temporary.path.join("consume.mjs"),
            r#"
import { readFileSync } from "node:fs";
import { SourceMap } from "node:module";
const generated = readFileSync("app.js", "utf8").split("\n");
const line = generated.findIndex((value) => value.includes("__pglRuntime.background"));
const column = generated[line].indexOf("__pglRuntime.background");
const payload = JSON.parse(readFileSync("app.js.map", "utf8"));
process.stdout.write(JSON.stringify(new SourceMap(payload).findEntry(line, column)));
"#,
        )
        .unwrap();
        let output = Command::new("node")
            .arg("consume.mjs")
            .current_dir(&temporary.path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let entry: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let original_prefix = "  text(\"😀\", 0.0, 0.0); ";
        assert_eq!(entry["originalSource"], "unicode.rb");
        assert_eq!(entry["originalLine"], 1);
        assert_eq!(
            entry["originalColumn"],
            original_prefix.encode_utf16().count()
        );
    }

    #[test]
    fn generated_integer_programs_preserve_safe_metamorphic_relations() {
        let mut state = 0x6d2b_79f5_u32;
        let mut ruby = String::from("def setup\n");
        let mut expected = Vec::new();
        for _ in 0..128 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let left = state as i32;
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let mut right = state as i32;
            if right == 0 || (left == i32::MIN && right == -1) {
                right = 1;
            }
            ruby.push_str(&format!(
                "  background(({left} + {right}) - {right}, ({left} / {right}) * {right} + ({left} % {right}), {left})\n"
            ));
            expected.push(serde_json::json!(["background", left, left, left]));
        }
        ruby.push_str("end\n");

        let source = SourceFile::new(SourceId::new(0), "generated-property.rb", ruby);
        let compiled = Compiler::standard()
            .compile(
                &source,
                "ruby",
                CompileOptions {
                    mode: BuildMode::Release,
                    source_map: SourceMapMode::None,
                    sources_content: false,
                    budget: CompileBudget::standard(),
                },
            )
            .unwrap();
        let temporary = TemporaryDirectory::new().unwrap();
        fs::write(
            temporary.path.join("app.js"),
            compiled.javascript.javascript,
        )
        .unwrap();
        fs::write(temporary.path.join("runtime.js"), super::RUNTIME_MOCK).unwrap();
        fs::write(temporary.path.join("runner.mjs"), super::NODE_RUNNER).unwrap();
        fs::write(
            temporary.path.join("package.json"),
            "{\"private\":true,\"type\":\"module\"}\n",
        )
        .unwrap();
        let output = Command::new("node")
            .arg("runner.mjs")
            .current_dir(&temporary.path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let actual: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(actual, serde_json::Value::Array(expected));
    }
}
