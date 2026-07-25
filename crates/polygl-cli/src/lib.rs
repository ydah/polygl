//! Command-line compilation pipeline and browser artifact packaging.

use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use polygl_adapter_api::{LanguageAdapter, LowerCtx};
use polygl_adapter_ruby::RubyAdapter;
use polygl_backend_js::{BuildMode, JavaScriptBackend};
use polygl_core::BuiltinTable;
use polygl_span::{Diagnostics, SourceFile, SourceId};
use polygl_types::TypedModule;

const RUNTIME_BUNDLE: &[u8] = include_bytes!("../assets/runtime.js");
const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>PolyGL sketch</title>
  <style>
    html, body { margin: 0; min-height: 100%; background: #111; }
    body { display: grid; place-items: center; }
    canvas { display: block; }
  </style>
</head>
<body>
  <canvas id="polygl-canvas" width="640" height="480"></canvas>
  <script type="module">
    import { showRuntimeError, start } from "./runtime.js";
    start(() => import("./app.js")).catch((error) => {
      console.error(error);
      showRuntimeError(error);
    });
  </script>
</body>
</html>
"#;

#[derive(Debug)]
pub struct CliError {
    message: String,
}

impl CliError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for CliError {}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Command {
    Build {
        source: PathBuf,
        output: PathBuf,
        mode: BuildMode,
    },
    Check {
        source: PathBuf,
    },
    DumpHir {
        source: PathBuf,
    },
    Help,
}

pub fn run(
    args: impl IntoIterator<Item = OsString>,
    output: &mut dyn Write,
) -> Result<(), CliError> {
    match parse_args(args)? {
        Command::Build {
            source,
            output,
            mode,
        } => build(&source, &output, mode),
        Command::Check { source } => {
            compile_frontend(&source)?;
            Ok(())
        }
        Command::DumpHir { source } => {
            let (_, typed) = compile_frontend(&source)?;
            output
                .write_all(polygl_hir::dump(typed.as_hir()).as_bytes())
                .map_err(|error| CliError::new(format!("failed to write HIR dump: {error}")))
        }
        Command::Help => output
            .write_all(usage().as_bytes())
            .map_err(|error| CliError::new(format!("failed to write help: {error}"))),
    }
}

fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<Command, CliError> {
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        return Err(CliError::new(usage()));
    };
    match command.to_str() {
        Some("build") => parse_build(args),
        Some("check") => parse_single_source(args, |source| Command::Check { source }),
        Some("dump-hir") => parse_single_source(args, |source| Command::DumpHir { source }),
        Some("help" | "--help" | "-h") => {
            ensure_empty(args)?;
            Ok(Command::Help)
        }
        Some(other) => Err(CliError::new(format!(
            "unknown command `{other}`\n\n{}",
            usage()
        ))),
        None => Err(CliError::new("command name is not valid UTF-8")),
    }
}

fn parse_build(mut args: impl Iterator<Item = OsString>) -> Result<Command, CliError> {
    let source = required_path(args.next(), "build requires a source file")?;
    let mut output = PathBuf::from("dist");
    let mut mode = BuildMode::Debug;
    let mut selected_mode = false;
    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("-o" | "--output") => {
                output = required_path(args.next(), "output option requires a directory")?;
            }
            Some("--debug") => {
                if selected_mode {
                    return Err(CliError::new("build mode may only be specified once"));
                }
                mode = BuildMode::Debug;
                selected_mode = true;
            }
            Some("--release") => {
                if selected_mode {
                    return Err(CliError::new("build mode may only be specified once"));
                }
                mode = BuildMode::Release;
                selected_mode = true;
            }
            Some(other) => return Err(CliError::new(format!("unknown build option `{other}`"))),
            None => return Err(CliError::new("build option is not valid UTF-8")),
        }
    }
    Ok(Command::Build {
        source,
        output,
        mode,
    })
}

fn parse_single_source(
    mut args: impl Iterator<Item = OsString>,
    command: impl FnOnce(PathBuf) -> Command,
) -> Result<Command, CliError> {
    let source = required_path(args.next(), "command requires a source file")?;
    ensure_empty(args)?;
    Ok(command(source))
}

fn required_path(value: Option<OsString>, message: &str) -> Result<PathBuf, CliError> {
    value
        .map(PathBuf::from)
        .ok_or_else(|| CliError::new(message))
}

fn ensure_empty(mut args: impl Iterator<Item = OsString>) -> Result<(), CliError> {
    if let Some(argument) = args.next() {
        return Err(CliError::new(format!(
            "unexpected argument `{}`",
            argument.to_string_lossy()
        )));
    }
    Ok(())
}

fn build(source_path: &Path, output: &Path, mode: BuildMode) -> Result<(), CliError> {
    let (source, typed) = compile_frontend(source_path)?;
    let lir = polygl_lir::lower(&typed);
    let artifacts = JavaScriptBackend::new(mode)
        .generate(&lir, std::slice::from_ref(&source))
        .map_err(|error| CliError::new(format!("JavaScript generation failed: {error}")))?;

    fs::create_dir_all(output).map_err(|error| {
        CliError::new(format!(
            "failed to create output directory {}: {error}",
            output.display()
        ))
    })?;
    write_artifact(&output.join("app.js"), artifacts.javascript.as_bytes())?;
    write_artifact(&output.join("app.js.map"), artifacts.source_map.as_bytes())?;
    write_artifact(&output.join("runtime.js"), RUNTIME_BUNDLE)?;
    write_artifact(&output.join("index.html"), INDEX_HTML.as_bytes())
}

fn compile_frontend(source_path: &Path) -> Result<(SourceFile, TypedModule), CliError> {
    match source_path
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("rb") => {}
        Some(extension) => {
            return Err(CliError::new(format!(
                "unsupported source extension `.{extension}`; M1 supports `.rb`"
            )));
        }
        None => return Err(CliError::new("source file must have a `.rb` extension")),
    }

    let bytes = fs::read(source_path).map_err(|error| {
        CliError::new(format!(
            "failed to read source {}: {error}",
            source_path.display()
        ))
    })?;
    let source = SourceFile::from_bytes(
        SourceId::new(0),
        source_path.to_string_lossy().into_owned(),
        bytes,
    )
    .map_err(|error| CliError::new(error.to_string()))?;

    let mut context = LowerCtx::new(&BuiltinTable);
    let hir = RubyAdapter
        .lower(&source, &mut context)
        .map_err(|diagnostics| diagnostic_error(&diagnostics, &source))?;
    let typed = polygl_types::analyze(&hir)
        .map_err(|diagnostics| diagnostic_error(&diagnostics, &source))?;
    Ok((source, typed))
}

fn diagnostic_error(diagnostics: &Diagnostics, source: &SourceFile) -> CliError {
    match diagnostics.render(source) {
        Ok(rendered) => CliError::new(rendered),
        Err(error) => CliError::new(format!("failed to render diagnostics: {error}")),
    }
}

fn write_artifact(path: &Path, contents: &[u8]) -> Result<(), CliError> {
    fs::write(path, contents).map_err(|error| {
        CliError::new(format!(
            "failed to write build artifact {}: {error}",
            path.display()
        ))
    })
}

fn usage() -> String {
    "\
usage:
  polygl build <source.rb> [-o <directory>] [--debug | --release]
  polygl check <source.rb>
  polygl dump-hir <source.rb>
"
    .to_owned()
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{BuildMode, Command, parse_args, run};

    static NEXT_TEMPORARY: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn parses_build_modes_and_rejects_conflicts() {
        assert_eq!(
            parse_args(arguments(["build", "main.rb", "-o", "web", "--release"])).unwrap(),
            Command::Build {
                source: "main.rb".into(),
                output: "web".into(),
                mode: BuildMode::Release,
            }
        );
        let error = parse_args(arguments(["build", "main.rb", "--debug", "--release"]))
            .unwrap_err()
            .to_string();
        assert!(error.contains("only be specified once"));
    }

    #[test]
    fn builds_debug_and_release_browser_artifacts() {
        let temporary = temporary_directory();
        let source = temporary.join("triangle.rb");
        fs::write(
            &source,
            "def setup\n  size(320, 180)\n  fill(1.0, 0.0, 0.0)\n  triangle(10.0, 10.0, 50.0, 10.0, 30.0, 40.0)\nend\n",
        )
        .unwrap();

        let debug = temporary.join("debug");
        run(
            arguments([
                "build",
                source.to_str().unwrap(),
                "-o",
                debug.to_str().unwrap(),
            ]),
            &mut Vec::new(),
        )
        .unwrap();
        let debug_javascript = fs::read_to_string(debug.join("app.js")).unwrap();
        assert!(debug_javascript.contains("const __pglSpans"));
        assert!(debug_javascript.contains("__pglRuntime.triangle"));
        assert!(debug.join("app.js.map").is_file());
        assert!(debug.join("runtime.js").is_file());
        assert!(
            fs::read_to_string(debug.join("index.html"))
                .unwrap()
                .contains("start(() => import(\"./app.js\"))")
        );

        let release = temporary.join("release");
        run(
            arguments([
                "build",
                source.to_str().unwrap(),
                "-o",
                release.to_str().unwrap(),
                "--release",
            ]),
            &mut Vec::new(),
        )
        .unwrap();
        assert!(
            !fs::read_to_string(release.join("app.js"))
                .unwrap()
                .contains("__pglSpans")
        );
        fs::remove_dir_all(temporary).unwrap();
    }

    #[test]
    fn check_renders_source_diagnostics_and_dump_hir_is_typed() {
        let temporary = temporary_directory();
        let invalid = temporary.join("invalid.rb");
        fs::write(&invalid, "define_method(:setup) { 1 }\n").unwrap();
        let error = run(
            arguments(["check", invalid.to_str().unwrap()]),
            &mut Vec::new(),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("E0200"));
        assert!(error.contains("invalid.rb"));
        assert!(error.contains("regular `def name` declaration"));

        let valid = temporary.join("valid.rb");
        fs::write(&valid, "def setup\n  value = 1\nend\n").unwrap();
        let mut output = Vec::new();
        run(
            arguments(["dump-hir", valid.to_str().unwrap()]),
            &mut output,
        )
        .unwrap();
        let dump = String::from_utf8(output).unwrap();
        assert!(dump.contains("entry setup() [host]"));
        assert!(dump.contains("let value: int = 1;"));
        fs::remove_dir_all(temporary).unwrap();
    }

    fn arguments<const N: usize>(items: [&str; N]) -> impl Iterator<Item = OsString> {
        items.into_iter().map(OsString::from)
    }

    fn temporary_directory() -> std::path::PathBuf {
        let sequence = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("polygl-cli-test-{}-{sequence}", std::process::id()));
        fs::create_dir(&path).unwrap();
        path
    }
}
