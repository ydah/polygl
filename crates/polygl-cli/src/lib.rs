//! Command-line compilation pipeline and browser artifact packaging.

use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use polygl_adapter_api::{LanguageAdapter, LowerCtx};
use polygl_adapter_ruby::RubyAdapter;
use polygl_backend_glsl::{GlslArtifacts, GlslBackend, UniformSource};
use polygl_backend_js::{BuildMode, JavaScriptBackend};
use polygl_core::BuiltinTable;
use polygl_span::{Diagnostics, SourceFile, SourceId};
use polygl_types::TypedModule;

mod serve;

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
    import { shaderBundle } from "./shaders.js";
    globalThis.__polyglReady = start(() => import("./app.js"), {
      shaderBundle,
    }).catch((error) => {
      console.error(error);
      showRuntimeError(error);
      throw error;
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
    Serve {
        source: PathBuf,
        watch: bool,
        port: u16,
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
            output: destination,
            mode,
        } => build(&source, &destination, mode, output),
        Command::Check { source } => {
            let (source, typed) = compile_frontend(&source)?;
            let (_, _, warnings) = compile_backends(&source, &typed, BuildMode::Debug)?;
            write_diagnostics(&warnings, &source, output)?;
            Ok(())
        }
        Command::Serve {
            source,
            watch,
            port,
        } => serve::serve(&source, watch, port, output),
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
        Some("serve") => parse_serve(args),
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

fn parse_serve(mut args: impl Iterator<Item = OsString>) -> Result<Command, CliError> {
    let source = required_path(args.next(), "serve requires a source file")?;
    let mut watch = false;
    let mut port = 4173;
    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("--watch") if !watch => watch = true,
            Some("--watch") => return Err(CliError::new("watch may only be specified once")),
            Some("--port") => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError::new("port option requires a number"))?;
                let value = value
                    .to_str()
                    .ok_or_else(|| CliError::new("port is not valid UTF-8"))?;
                port = value.parse::<u16>().map_err(|_| {
                    CliError::new(format!("port `{value}` must be between 0 and 65535"))
                })?;
            }
            Some(other) => return Err(CliError::new(format!("unknown serve option `{other}`"))),
            None => return Err(CliError::new("serve option is not valid UTF-8")),
        }
    }
    Ok(Command::Serve {
        source,
        watch,
        port,
    })
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

fn build(
    source_path: &Path,
    destination: &Path,
    mode: BuildMode,
    messages: &mut dyn Write,
) -> Result<(), CliError> {
    let (source, typed) = compile_frontend(source_path)?;
    let (javascript, shaders, warnings) = compile_backends(&source, &typed, mode)?;
    write_diagnostics(&warnings, &source, messages)?;

    fs::create_dir_all(destination).map_err(|error| {
        CliError::new(format!(
            "failed to create output directory {}: {error}",
            destination.display()
        ))
    })?;
    write_artifact(
        &destination.join("app.js"),
        javascript.javascript.as_bytes(),
    )?;
    write_artifact(
        &destination.join("app.js.map"),
        javascript.source_map.as_bytes(),
    )?;
    write_artifact(
        &destination.join("shaders.js"),
        render_shader_module(&shaders, &source, mode)?.as_bytes(),
    )?;
    write_artifact(&destination.join("runtime.js"), RUNTIME_BUNDLE)?;
    write_artifact(&destination.join("index.html"), INDEX_HTML.as_bytes())
}

fn compile_backends(
    source: &SourceFile,
    typed: &TypedModule,
    mode: BuildMode,
) -> Result<(polygl_backend_js::Artifacts, GlslArtifacts, Diagnostics), CliError> {
    let lir = polygl_lir::lower(typed);
    let split =
        polygl_lir::split(&lir).map_err(|diagnostics| diagnostic_error(&diagnostics, source))?;
    let javascript = JavaScriptBackend::new(mode)
        .generate(&split.host, std::slice::from_ref(source))
        .map_err(|error| CliError::new(format!("JavaScript generation failed: {error}")))?;
    let shaders = GlslBackend::new()
        .generate(&split.gpu)
        .map_err(|error| CliError::new(format!("GLSL generation failed: {error}")))?;
    Ok((javascript, shaders, split.warnings))
}

fn render_shader_module(
    artifacts: &GlslArtifacts,
    source: &SourceFile,
    mode: BuildMode,
) -> Result<String, CliError> {
    let mut shaders = Vec::with_capacity(artifacts.shaders.len());
    for shader in &artifacts.shaders {
        let attributes = shader
            .attributes
            .iter()
            .map(|attribute| {
                Ok(format!(
                    "{{name:{},glslName:{},location:{},type:{}}}",
                    js_string(&attribute.name),
                    js_string(&attribute.glsl_name),
                    attribute.location,
                    js_string(shader_type(&attribute.ty)?),
                ))
            })
            .collect::<Result<Vec<_>, CliError>>()?
            .join(",");
        let uniforms = shader
            .uniforms
            .iter()
            .map(|uniform| {
                Ok(format!(
                    "{{name:{},glslName:{},type:{},source:{}}}",
                    js_string(&uniform.name),
                    js_string(&uniform.glsl_name),
                    js_string(shader_type(&uniform.ty)?),
                    js_string(match uniform.source {
                        UniformSource::Automatic => "automatic",
                        UniformSource::User => "user",
                    }),
                ))
            })
            .collect::<Result<Vec<_>, CliError>>()?
            .join(",");
        shaders.push(format!(
            "{{name:{},vertex:{},fragment:{},attributes:[{}],uniforms:[{}],vertexLocation:{},fragmentLocation:{}}}",
            js_string(&shader.name),
            js_string(&shader.vertex),
            js_string(&shader.fragment),
            attributes,
            uniforms,
            render_location(source, shader.vertex_span)?,
            render_location(source, shader.fragment_span)?,
        ));
    }
    Ok(format!(
        "export const shaderBundle = Object.freeze({{debug:{},shaders:Object.freeze([{}])}});\n",
        mode == BuildMode::Debug,
        shaders.join(","),
    ))
}

fn shader_type(ty: &polygl_types::Type) -> Result<&'static str, CliError> {
    match ty {
        polygl_types::Type::Int => Ok("int"),
        polygl_types::Type::Float => Ok("float"),
        polygl_types::Type::Bool => Ok("bool"),
        polygl_types::Type::Vector(2) => Ok("vec2"),
        polygl_types::Type::Vector(3) => Ok("vec3"),
        polygl_types::Type::Vector(4) => Ok("vec4"),
        polygl_types::Type::Matrix(2) => Ok("mat2"),
        polygl_types::Type::Matrix(3) => Ok("mat3"),
        polygl_types::Type::Matrix(4) => Ok("mat4"),
        polygl_types::Type::Opaque(polygl_hir::OpaqueType::Texture) => Ok("texture"),
        other => Err(CliError::new(format!(
            "cannot package shader binding type `{other}`"
        ))),
    }
}

fn render_location(source: &SourceFile, span: polygl_span::Span) -> Result<String, CliError> {
    span.validate_for(source)
        .map_err(|error| CliError::new(format!("invalid shader source span: {error}")))?;
    let position = source
        .position(span.start())
        .map_err(|error| CliError::new(format!("invalid shader source position: {error}")))?;
    Ok(format!(
        "{{source:{},line:{},column:{},start:{},end:{}}}",
        js_string(source.name()),
        position.line,
        position.scalar_column,
        span.start(),
        span.end(),
    ))
}

fn js_string(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a UTF-8 string cannot fail")
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

fn write_diagnostics(
    diagnostics: &Diagnostics,
    source: &SourceFile,
    output: &mut dyn Write,
) -> Result<(), CliError> {
    if diagnostics.is_empty() {
        return Ok(());
    }
    let rendered = diagnostics
        .render(source)
        .map_err(|error| CliError::new(format!("failed to render diagnostics: {error}")))?;
    output
        .write_all(rendered.as_bytes())
        .map_err(|error| CliError::new(format!("failed to write diagnostics: {error}")))
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
  polygl serve <source.rb> [--port <port>] [--watch]
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

        assert_eq!(
            parse_args(arguments(["serve", "main.rb", "--watch", "--port", "8080"])).unwrap(),
            Command::Serve {
                source: "main.rb".into(),
                watch: true,
                port: 8080,
            }
        );
        assert!(
            parse_args(arguments(["serve", "main.rb", "--port", "invalid"]))
                .unwrap_err()
                .to_string()
                .contains("between 0 and 65535")
        );
    }

    #[test]
    fn builds_debug_and_release_browser_artifacts() {
        let temporary = temporary_directory();
        let source = temporary.join("triangle.rb");
        fs::write(
            &source,
            r#"def setup
  size(320, 180)
  material_shader("plasma")
  fill(1.0, 0.0, 0.0)
  stroke(1.0, 1.0, 1.0)
  push_matrix()
  translate(4.0, 8.0)
  triangle(10.0, 10.0, 50.0, 10.0, 30.0, 40.0)
  text("ready", 4.0, 12.0)
  pop_matrix()
end

def on_event(event)
  if event.kind == "pointerdown"
    line(event.x, event.y, mouse_x(), mouse_y())
  end
end

def vertex_plasma
  vec4(0.0, 0.0, 0.0, 1.0)
end

def fragment_plasma
  vec4(time(), 0.0, 0.0, 1.0)
end
"#,
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
        assert!(debug_javascript.contains("__pglRuntime.pushMatrix"));
        assert!(debug_javascript.contains("__pglRuntime.text(\"ready\""));
        assert!(debug_javascript.contains("[\"kind\"]"));
        assert!(debug_javascript.contains("__pglRuntime.materialShader(\"plasma\")"));
        assert!(debug.join("app.js.map").is_file());
        assert!(debug.join("runtime.js").is_file());
        let debug_shaders = fs::read_to_string(debug.join("shaders.js")).unwrap();
        assert!(debug_shaders.contains("debug:true"));
        assert!(debug_shaders.contains("name:\"plasma\""));
        assert!(debug_shaders.contains("name:\"u_time\""));
        assert!(debug_shaders.contains("#version 300 es"));
        assert!(
            fs::read_to_string(debug.join("index.html"))
                .unwrap()
                .contains("from \"./shaders.js\"")
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
        assert!(
            fs::read_to_string(release.join("shaders.js"))
                .unwrap()
                .contains("debug:false")
        );
        fs::remove_dir_all(temporary).unwrap();
    }

    #[test]
    fn builds_ruby_collections_and_block_sugar() {
        let temporary = temporary_directory();
        let source = temporary.join("collections.rb");
        fs::write(
            &source,
            r#"def setup
  total = 0
  values = [1, 2, 3]
  values[0] = 4
  labels = {left: 5, "right" => 6}
  values.each do |value|
    total = total + value
  end
  2.times do |index|
    total = total + index
  end
  (1..2).each do |value|
    total = total + value
  end
  line(values[0], labels["left"], total, labels[:right])
end
"#,
        )
        .unwrap();
        let output = temporary.join("web");
        run(
            arguments([
                "build",
                source.to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
            ]),
            &mut Vec::new(),
        )
        .unwrap();
        let javascript = fs::read_to_string(output.join("app.js")).unwrap();
        assert!(javascript.contains("Object.fromEntries"));
        assert!(javascript.contains(").length"));
        assert!(javascript.contains("__pglRuntime.checkedIndex"));
        assert!(javascript.contains("__pglRangeIndex"));
        fs::remove_dir_all(temporary).unwrap();
    }

    #[test]
    fn builds_ruby_struct_like_classes() {
        let temporary = temporary_directory();
        let source = temporary.join("classes.rb");
        fs::write(
            &source,
            r#"def seed
  20
end

class Dot
  def initialize(x, y)
    @x = x
    @y = seed()
  end

  def move(dx)
    @x = @x + dx
  end

  def paint
    circle(@x, @y, 2)
  end

  def coordinate
    @x
  end

  def outer
    coordinate
  end

  def x
    99
  end
end

class FloatDot
  def initialize(x)
    @x = x
  end

  def coordinate
    @x
  end
end

def setup
  dot = Dot.new(10, 20)
  floating = FloatDot.new(1.5)
  dot.move(3)
  dot.x = 15
  dot.paint
  line(dot.outer, floating.coordinate, dot.x(), dot.x)
end
"#,
        )
        .unwrap();
        let output = temporary.join("web");
        run(
            arguments([
                "build",
                source.to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
            ]),
            &mut Vec::new(),
        )
        .unwrap();
        let javascript = fs::read_to_string(output.join("app.js")).unwrap();
        assert!(javascript.contains(r#"{"x": x, "y": __pglFunction_"#));
        assert!(javascript.contains("[\"x\"] ="));
        assert!(javascript.contains("__pglRuntime.circle"));
        assert!(javascript.contains("return 99;"));
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

        let missing_shader = temporary.join("missing_shader.rb");
        fs::write(
            &missing_shader,
            "def setup\n  material_shader(\"missing\")\nend\n",
        )
        .unwrap();
        let error = run(
            arguments(["check", missing_shader.to_str().unwrap()]),
            &mut Vec::new(),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("E0405"));
        assert!(error.contains("unknown shader pair `missing`"));

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

        let warning = temporary.join("warning.rb");
        fs::write(
            &warning,
            "def helper\n  time()\nend\n\ndef setup\n  helper()\nend\n\ndef vertex_warning\n  helper()\n  vec4(0.0, 0.0, 0.0, 1.0)\nend\n\ndef fragment_warning\n  vec4(1.0, 1.0, 1.0, 1.0)\nend\n",
        )
        .unwrap();
        let mut warnings = Vec::new();
        run(
            arguments(["check", warning.to_str().unwrap()]),
            &mut warnings,
        )
        .unwrap();
        assert!(String::from_utf8(warnings).unwrap().contains("W0401"));
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
