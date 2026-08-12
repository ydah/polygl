//! Command-line compilation pipeline and browser artifact packaging.

use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use polygl_adapter_api::{ADAPTER_API_VERSION, LanguageAdapter};
use polygl_backend_glsl::{GlslArtifacts, SHADER_ABI_VERSION, UniformSource};
use polygl_backend_js::{BuildMode, RUNTIME_ABI_VERSION, SourceMapMode};
use polygl_core::{
    AdapterRegistry, BUILTIN_SCHEMA_VERSION, CompileBudget, CompileError, CompileOptions, Compiler,
};
use polygl_hir::HIR_SCHEMA_VERSION;
use polygl_lir::LIR_SCHEMA_VERSION;
use polygl_span::{Diagnostics, SourceFile, SourceId};

mod artifact;
mod diagnostic_output;
mod serve;

use artifact::{ArtifactFile, prepare_assets, publish};
use diagnostic_output::DiagnosticFormat;

const RUNTIME_BUNDLE: &[u8] = include_bytes!("../assets/runtime.js");
const VERSION: &str = env!("CARGO_PKG_VERSION");
const ARTIFACT_MANIFEST_SCHEMA_VERSION: u32 = 2;
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
      requireRuntimeAbi: true,
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
    kind: CliErrorKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CliErrorKind {
    Other,
    Usage,
    Parse,
    Compile,
    Io,
    Internal,
}

impl CliError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: CliErrorKind::Other,
        }
    }

    fn with_kind(mut self, kind: CliErrorKind) -> Self {
        self.kind = kind;
        self
    }

    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self.kind {
            CliErrorKind::Other => 1,
            CliErrorKind::Usage => 2,
            CliErrorKind::Parse => 3,
            CliErrorKind::Compile => 4,
            CliErrorKind::Io => 5,
            CliErrorKind::Internal => 70,
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
        options: BuildOptions,
    },
    Check {
        source: PathBuf,
        diagnostic_options: DiagnosticOptions,
    },
    Serve {
        source: PathBuf,
        watch: bool,
        port: u16,
    },
    DumpHir {
        source: PathBuf,
    },
    Explain {
        code: polygl_span::DiagnosticCode,
        format: DiagnosticFormat,
    },
    Languages {
        json: bool,
    },
    NewAdapter {
        language: String,
        output: PathBuf,
    },
    Version,
    Help,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct DiagnosticOptions {
    format: DiagnosticFormat,
    color: ColorChoice,
    color_supported: bool,
    warning_policy: WarningPolicy,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

impl DiagnosticOptions {
    const fn color_enabled(&self) -> bool {
        match self.color {
            ColorChoice::Auto => self.color_supported,
            ColorChoice::Always => true,
            ColorChoice::Never => false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct WarningPolicy {
    deny_warnings: bool,
    max_warnings: Option<usize>,
    allowed: Vec<WarningSelector>,
    denied: Vec<WarningSelector>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WarningSelector(String);

impl WarningSelector {
    fn parse(value: &str) -> Result<Self, CliError> {
        let is_exact = value.len() == 5
            && value.starts_with('W')
            && value[1..].bytes().all(|byte| byte.is_ascii_digit())
            && polygl_span::DiagnosticCode::parse(value)
                .is_some_and(|code| code.metadata().severity == polygl_span::Severity::Warning);
        let is_family = value.len() == 5
            && value.starts_with('W')
            && value[1..3].bytes().all(|byte| byte.is_ascii_digit())
            && value.ends_with("xx");
        if !is_exact && !is_family {
            return Err(CliError::new(format!(
                "warning selector `{value}` must be a registered warning code or a family such as W04xx"
            )));
        }
        Ok(Self(if is_family {
            value[..3].to_owned()
        } else {
            value.to_owned()
        }))
    }

    fn matches(&self, code: polygl_span::DiagnosticCode) -> bool {
        code.as_str().starts_with(&self.0)
    }
}

impl WarningPolicy {
    fn is_denied(&self, code: polygl_span::DiagnosticCode) -> bool {
        if self.allowed.iter().any(|selector| selector.matches(code)) {
            return false;
        }
        self.deny_warnings || self.denied.iter().any(|selector| selector.matches(code))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BuildOptions {
    mode: BuildMode,
    source_map: SourceMapMode,
    sources_content: bool,
}

impl BuildOptions {
    const fn check() -> Self {
        Self {
            mode: BuildMode::Debug,
            source_map: SourceMapMode::None,
            sources_content: false,
        }
    }

    pub(crate) const fn development() -> Self {
        Self {
            mode: BuildMode::Debug,
            source_map: SourceMapMode::External,
            sources_content: true,
        }
    }

    const fn compiler(self) -> CompileOptions {
        CompileOptions {
            mode: self.mode,
            source_map: self.source_map,
            sources_content: self.sources_content,
            budget: CompileBudget::standard(),
        }
    }
}

pub fn run(
    args: impl IntoIterator<Item = OsString>,
    output: &mut dyn Write,
) -> Result<(), CliError> {
    run_with_color_support(args, output, false)
}

pub fn run_with_color_support(
    args: impl IntoIterator<Item = OsString>,
    output: &mut dyn Write,
    color_supported: bool,
) -> Result<(), CliError> {
    match parse_args(args).map_err(|error| error.with_kind(CliErrorKind::Usage))? {
        Command::Build {
            source,
            output: destination,
            options,
        } => build(&source, &destination, options, output).map(|_| ()),
        Command::Check {
            source,
            mut diagnostic_options,
        } => {
            diagnostic_options.color_supported = color_supported;
            let source_file = load_source(&source)?;
            let compiler = compiler();
            let adapter = adapter_for_source(compiler.adapters(), &source)?;
            let compiled = compiler
                .compile(&source_file, adapter.id(), BuildOptions::check().compiler())
                .map_err(|error| {
                    compilation_error_with_options(error, &source_file, &diagnostic_options)
                })?;
            write_diagnostics(&compiled.warnings, &source_file, diagnostic_options, output)?;
            Ok(())
        }
        Command::Serve {
            source,
            watch,
            port,
        } => serve::serve(&source, watch, port, output),
        Command::DumpHir { source } => {
            let source_file = load_source(&source)?;
            let compiler = compiler();
            let adapter = adapter_for_source(compiler.adapters(), &source)?;
            let typed = compiler
                .analyze(&source_file, adapter.id())
                .map_err(|error| compilation_error(error, &source_file))?
                .typed;
            output
                .write_all(polygl_hir::dump(typed.module().as_hir()).as_bytes())
                .map_err(|error| CliError::new(format!("failed to write HIR dump: {error}")))
        }
        Command::Explain { code, format } => write_explanation(code, format, output),
        Command::Languages { json } => write_languages(json, output),
        Command::NewAdapter {
            language,
            output: destination,
        } => new_adapter(&language, &destination, output),
        Command::Version => writeln!(output, "polygl {VERSION}")
            .map_err(|error| CliError::new(format!("failed to write version: {error}"))),
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
        Some("check") => parse_check(args),
        Some("dump-hir") => parse_single_source(args, |source| Command::DumpHir { source }),
        Some("explain") => parse_explain(args),
        Some("languages") => parse_languages(args),
        Some("new-adapter") => parse_new_adapter(args),
        Some("--version" | "-V") => {
            ensure_empty(args)?;
            Ok(Command::Version)
        }
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

fn parse_check(mut args: impl Iterator<Item = OsString>) -> Result<Command, CliError> {
    let source = required_path(args.next(), "check requires a source file")?;
    let diagnostic_options = parse_diagnostic_options(args)?;
    Ok(Command::Check {
        source,
        diagnostic_options,
    })
}

fn parse_diagnostic_options(
    mut args: impl Iterator<Item = OsString>,
) -> Result<DiagnosticOptions, CliError> {
    let mut options = DiagnosticOptions::default();
    let mut selected_format = false;
    let mut selected_color = false;
    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("--diagnostic-format") if !selected_format => {
                let value = args.next().ok_or_else(|| {
                    CliError::new("diagnostic format requires human, json, sarif, or lsp")
                })?;
                options.format = parse_diagnostic_format(&value)?;
                selected_format = true;
            }
            Some("--diagnostic-format") => {
                return Err(CliError::new(
                    "diagnostic format may only be specified once",
                ));
            }
            Some("--color") if !selected_color => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError::new("color requires auto, always, or never"))?;
                options.color = match value.to_str() {
                    Some("auto") => ColorChoice::Auto,
                    Some("always") => ColorChoice::Always,
                    Some("never") => ColorChoice::Never,
                    Some(other) => {
                        return Err(CliError::new(format!(
                            "unknown color mode `{other}`; expected auto, always, or never"
                        )));
                    }
                    None => return Err(CliError::new("color mode is not valid UTF-8")),
                };
                selected_color = true;
            }
            Some("--color") => {
                return Err(CliError::new("color mode may only be specified once"));
            }
            Some("--deny-warnings") if !options.warning_policy.deny_warnings => {
                options.warning_policy.deny_warnings = true;
            }
            Some("--deny-warnings") => {
                return Err(CliError::new("deny warnings may only be specified once"));
            }
            Some("--max-warnings") if options.warning_policy.max_warnings.is_none() => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError::new("max warnings requires a non-negative number"))?;
                let value = value
                    .to_str()
                    .ok_or_else(|| CliError::new("max warnings is not valid UTF-8"))?;
                options.warning_policy.max_warnings = Some(value.parse().map_err(|_| {
                    CliError::new(format!(
                        "max warnings `{value}` is not a non-negative number"
                    ))
                })?);
            }
            Some("--max-warnings") => {
                return Err(CliError::new("max warnings may only be specified once"));
            }
            Some("--allow" | "--deny") => {
                let deny = argument == "--deny";
                let value = args
                    .next()
                    .ok_or_else(|| CliError::new("warning control requires a warning selector"))?;
                let value = value
                    .to_str()
                    .ok_or_else(|| CliError::new("warning selector is not valid UTF-8"))?;
                let selector = WarningSelector::parse(value)?;
                if deny {
                    options.warning_policy.denied.push(selector);
                } else {
                    options.warning_policy.allowed.push(selector);
                }
            }
            Some(other) => return Err(CliError::new(format!("unknown check option `{other}`"))),
            None => return Err(CliError::new("check option is not valid UTF-8")),
        }
    }
    Ok(options)
}

fn parse_diagnostic_format(value: &OsString) -> Result<DiagnosticFormat, CliError> {
    match value.to_str() {
        Some("human") => Ok(DiagnosticFormat::Human),
        Some("json") => Ok(DiagnosticFormat::Json),
        Some("sarif") => Ok(DiagnosticFormat::Sarif),
        Some("lsp") => Ok(DiagnosticFormat::Lsp),
        Some(other) => Err(CliError::new(format!(
            "unknown diagnostic format `{other}`; expected human, json, sarif, or lsp"
        ))),
        None => Err(CliError::new("diagnostic format is not valid UTF-8")),
    }
}

fn parse_explain(mut args: impl Iterator<Item = OsString>) -> Result<Command, CliError> {
    let code = args
        .next()
        .ok_or_else(|| CliError::new("explain requires a diagnostic code"))?
        .into_string()
        .map_err(|_| CliError::new("diagnostic code is not valid UTF-8"))?;
    let code = polygl_span::DiagnosticCode::parse(&code)
        .ok_or_else(|| CliError::new(format!("unknown diagnostic code `{code}`")))?;
    let mut format = DiagnosticFormat::Human;
    if let Some(argument) = args.next() {
        if argument != "--diagnostic-format" {
            return Err(CliError::new(format!(
                "unknown explain option `{}`",
                argument.to_string_lossy()
            )));
        }
        format = parse_diagnostic_format(
            &args
                .next()
                .ok_or_else(|| CliError::new("diagnostic format requires a value"))?,
        )?;
    }
    ensure_empty(args)?;
    Ok(Command::Explain { code, format })
}

fn parse_languages(mut args: impl Iterator<Item = OsString>) -> Result<Command, CliError> {
    let json = match args.next() {
        None => false,
        Some(argument) if argument == "--json" => true,
        Some(argument) => {
            return Err(CliError::new(format!(
                "unknown languages option `{}`",
                argument.to_string_lossy()
            )));
        }
    };
    ensure_empty(args)?;
    Ok(Command::Languages { json })
}

fn parse_new_adapter(mut args: impl Iterator<Item = OsString>) -> Result<Command, CliError> {
    let language = args
        .next()
        .ok_or_else(|| CliError::new("new-adapter requires a language identifier"))?
        .into_string()
        .map_err(|_| CliError::new("language identifier is not valid UTF-8"))?;
    if !valid_language_id(&language) {
        return Err(CliError::new(
            "language identifier must start with a lowercase ASCII letter and contain only lowercase letters or digits",
        ));
    }
    let mut output = PathBuf::from(format!("polygl-adapter-{language}"));
    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("-o" | "--output") => {
                output = required_path(args.next(), "output option requires a directory")?;
            }
            Some(other) => {
                return Err(CliError::new(format!(
                    "unknown new-adapter option `{other}`"
                )));
            }
            None => return Err(CliError::new("new-adapter option is not valid UTF-8")),
        }
    }
    Ok(Command::NewAdapter { language, output })
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
    let mut source_map = None;
    let mut sources_content = false;
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
            Some("--source-map") if source_map.is_none() => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError::new("source map option requires a mode"))?;
                source_map = Some(match value.to_str() {
                    Some("none") => SourceMapMode::None,
                    Some("external") => SourceMapMode::External,
                    Some("inline") => SourceMapMode::Inline,
                    Some(other) => {
                        return Err(CliError::new(format!(
                            "unknown source map mode `{other}`; expected none, external, or inline"
                        )));
                    }
                    None => return Err(CliError::new("source map mode is not valid UTF-8")),
                });
            }
            Some("--source-map") => {
                return Err(CliError::new("source map mode may only be specified once"));
            }
            Some("--sources-content") if !sources_content => sources_content = true,
            Some("--sources-content") => {
                return Err(CliError::new("sources content may only be specified once"));
            }
            Some(other) => return Err(CliError::new(format!("unknown build option `{other}`"))),
            None => return Err(CliError::new("build option is not valid UTF-8")),
        }
    }
    let source_map = source_map.unwrap_or(if mode == BuildMode::Debug {
        SourceMapMode::External
    } else {
        SourceMapMode::None
    });
    if sources_content && source_map == SourceMapMode::None {
        return Err(CliError::new(
            "--sources-content requires an external or inline source map",
        ));
    }
    Ok(Command::Build {
        source,
        output,
        options: BuildOptions {
            mode,
            source_map,
            sources_content,
        },
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

pub(crate) struct BuildReport {
    pub(crate) watched_paths: Vec<PathBuf>,
}

fn build(
    source_path: &Path,
    destination: &Path,
    options: BuildOptions,
    messages: &mut dyn Write,
) -> Result<BuildReport, CliError> {
    let source = load_source(source_path)?;
    let compiler = compiler();
    let adapter = adapter_for_source(compiler.adapters(), source_path)?;
    let compiled = compiler
        .compile(&source, adapter.id(), options.compiler())
        .map_err(|error| compilation_error(error, &source))?;
    let assets = prepare_assets(source_path, &compiled.assets)?;
    write_diagnostics(
        &compiled.warnings,
        &source,
        DiagnosticOptions::default(),
        messages,
    )?;

    let shader_module = render_shader_module(&compiled.shaders, &source, options.mode)?;
    let mut files = vec![
        ArtifactFile::new("app.js", compiled.javascript.javascript.into_bytes()),
        ArtifactFile::new("shaders.js", shader_module.into_bytes()),
        ArtifactFile::new("runtime.js", RUNTIME_BUNDLE.to_vec()),
        ArtifactFile::new("index.html", INDEX_HTML.as_bytes().to_vec()),
    ];
    if let Some(source_map) = compiled.javascript.source_map {
        files.push(ArtifactFile::new("app.js.map", source_map.into_bytes()));
    }
    files.extend(assets.files);
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let manifest = render_artifact_manifest(&source, adapter, options, &files)?;
    files.push(ArtifactFile::new("polygl-manifest.json", manifest));
    let mut watched_paths = Vec::with_capacity(assets.source_paths.len() + 1);
    watched_paths.push(source_path.to_path_buf());
    watched_paths.extend(assets.source_paths);
    publish(destination, files)?;
    Ok(BuildReport { watched_paths })
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
        "export const shaderBundle = Object.freeze({{shaderAbi:{SHADER_ABI_VERSION},debug:{},shaders:Object.freeze([{}])}});\n",
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

fn load_source(source_path: &Path) -> Result<SourceFile, CliError> {
    let bytes = fs::read(source_path).map_err(|error| {
        CliError::new(format!(
            "failed to read source {}: {error}",
            source_path.display()
        ))
        .with_kind(CliErrorKind::Io)
    })?;
    let source_name = normalized_source_name(source_path)?;
    SourceFile::from_bytes(SourceId::new(0), source_name, bytes)
        .map_err(|error| CliError::new(error.to_string()))
}

fn compiler() -> Compiler<'static> {
    Compiler::standard()
}

fn adapter_for_source<'adapter>(
    registry: &'adapter AdapterRegistry<'adapter>,
    source_path: &Path,
) -> Result<&'adapter dyn LanguageAdapter, CliError> {
    match source_path
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some(extension) => registry.for_extension(extension).ok_or_else(|| {
            CliError::new(format!(
                "unsupported source extension `.{extension}`; supported extensions are {}",
                supported_extensions(registry)
            ))
        }),
        None => Err(CliError::new(format!(
            "source file must have one of these extensions: {}",
            supported_extensions(registry)
        ))),
    }
}

fn supported_extensions(registry: &AdapterRegistry<'_>) -> String {
    registry
        .iter()
        .flat_map(LanguageAdapter::file_extensions)
        .map(|extension| format!("`.{extension}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_artifact_manifest(
    source: &SourceFile,
    adapter: &dyn LanguageAdapter,
    options: BuildOptions,
    files: &[ArtifactFile],
) -> Result<Vec<u8>, CliError> {
    let mut features = adapter
        .capabilities()
        .iter()
        .map(|feature| feature.as_str())
        .collect::<Vec<_>>();
    features.sort_unstable();
    let artifacts = files
        .iter()
        .map(|file| {
            let path = file
                .relative_path
                .components()
                .map(|component| component.as_os_str().to_str())
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| CliError::new("artifact manifest path is not valid UTF-8"))?
                .join("/");
            Ok(serde_json::json!({
                "blake3": blake3::hash(&file.contents).to_hex().to_string(),
                "path": path,
                "size": file.contents.len(),
            }))
        })
        .collect::<Result<Vec<_>, CliError>>()?;
    let manifest = serde_json::json!({
        "adapter": {
            "apiVersion": ADAPTER_API_VERSION,
            "features": features,
            "id": adapter.id(),
        },
        "artifacts": artifacts,
        "compiler": {
            "name": "polygl",
            "version": VERSION,
        },
        "options": {
            "mode": match options.mode {
                BuildMode::Debug => "debug",
                BuildMode::Release => "release",
            },
            "sourceMap": match options.source_map {
                SourceMapMode::None => "none",
                SourceMapMode::External => "external",
                SourceMapMode::Inline => "inline",
            },
            "sourcesContent": options.sources_content,
        },
        "runtimeAbi": RUNTIME_ABI_VERSION,
        "shaderAbi": SHADER_ABI_VERSION,
        "schemaVersion": ARTIFACT_MANIFEST_SCHEMA_VERSION,
        "schemas": {
            "builtins": BUILTIN_SCHEMA_VERSION,
            "hir": HIR_SCHEMA_VERSION,
            "lir": LIR_SCHEMA_VERSION,
        },
        "source": {
            "blake3": blake3::hash(source.text().as_bytes()).to_hex().to_string(),
            "path": source.name(),
        },
    });
    let mut output = serde_json::to_vec_pretty(&manifest).map_err(|error| {
        CliError::new(format!("failed to serialize artifact manifest: {error}"))
    })?;
    output.push(b'\n');
    Ok(output)
}

fn normalized_source_name(source_path: &Path) -> Result<String, CliError> {
    let canonical_source = source_path.canonicalize().map_err(|error| {
        CliError::new(format!(
            "failed to resolve source path {}: {error}",
            source_path.display()
        ))
    })?;
    let project_relative = std::env::current_dir()
        .ok()
        .and_then(|directory| directory.canonicalize().ok())
        .and_then(|directory| {
            canonical_source
                .strip_prefix(directory)
                .ok()
                .map(Path::to_path_buf)
        });
    let display_path = project_relative
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| {
            source_path
                .file_name()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("source"))
        });
    let components = display_path
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if components.is_empty() {
        return Err(CliError::new(format!(
            "source path {} has no portable file name",
            source_path.display()
        )));
    }
    Ok(components.join("/"))
}

fn compilation_error(error: CompileError, source: &SourceFile) -> CliError {
    let kind = compile_error_kind(&error);
    CliError::new(error.render(source)).with_kind(kind)
}

fn compilation_error_with_options(
    error: CompileError,
    source: &SourceFile,
    options: &DiagnosticOptions,
) -> CliError {
    let Some(diagnostics) = error.diagnostics() else {
        let kind = compile_error_kind(&error);
        return CliError::new(error.to_string()).with_kind(kind);
    };
    match diagnostic_output::render(diagnostics, source, options.format, options.color_enabled()) {
        Ok(rendered) => CliError::new(rendered).with_kind(compile_error_kind(&error)),
        Err(render_error) => CliError::new(format!(
            "failed to render compilation diagnostics: {render_error}"
        ))
        .with_kind(CliErrorKind::Internal),
    }
}

fn compile_error_kind(error: &CompileError) -> CliErrorKind {
    if error.diagnostics().is_some_and(|diagnostics| {
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == polygl_span::DiagnosticCode::E0100)
    }) {
        return CliErrorKind::Parse;
    }
    match error {
        CompileError::Frontend(_) | CompileError::Split(_) => CliErrorKind::Compile,
        CompileError::Configuration(_) | CompileError::UnsupportedLanguage(_) => {
            CliErrorKind::Usage
        }
        CompileError::JavaScript(_)
        | CompileError::Glsl(_)
        | CompileError::InvalidDiagnostics { .. } => CliErrorKind::Internal,
    }
}

fn write_diagnostics(
    diagnostics: &Diagnostics,
    source: &SourceFile,
    options: DiagnosticOptions,
    output: &mut dyn Write,
) -> Result<(), CliError> {
    if diagnostics.is_empty() {
        return Ok(());
    }
    let rendered =
        diagnostic_output::render(diagnostics, source, options.format, options.color_enabled())
            .map_err(|error| CliError::new(format!("failed to render diagnostics: {error}")))?;
    output
        .write_all(rendered.as_bytes())
        .map_err(|error| CliError::new(format!("failed to write diagnostics: {error}")))?;
    let warning_count = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == polygl_span::Severity::Warning)
        .count();
    let denied_count = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.severity == polygl_span::Severity::Warning
                && options.warning_policy.is_denied(diagnostic.code)
        })
        .count();
    if denied_count > 0 {
        return Err(CliError::new(format!(
            "compilation produced {denied_count} denied warning(s)"
        ))
        .with_kind(CliErrorKind::Compile));
    }
    if let Some(maximum) = options.warning_policy.max_warnings
        && warning_count > maximum
    {
        return Err(CliError::new(format!(
            "compilation produced {warning_count} warning(s), exceeding --max-warnings {maximum}"
        ))
        .with_kind(CliErrorKind::Compile));
    }
    Ok(())
}

fn write_languages(json: bool, output: &mut dyn Write) -> Result<(), CliError> {
    let compiler = compiler();
    if json {
        let languages = compiler
            .adapters()
            .iter()
            .map(|adapter| {
                serde_json::json!({
                    "id": adapter.id(),
                    "extensions": adapter.file_extensions(),
                    "features": adapter.capabilities().iter().map(|feature| feature.as_str()).collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>();
        serde_json::to_writer_pretty(&mut *output, &languages)
            .map_err(|error| CliError::new(format!("failed to write languages JSON: {error}")))?;
        writeln!(output)
            .map_err(|error| CliError::new(format!("failed to write languages JSON: {error}")))?;
        return Ok(());
    }
    for adapter in compiler.adapters().iter() {
        writeln!(
            output,
            "{}\t{}",
            adapter.id(),
            adapter
                .file_extensions()
                .iter()
                .map(|extension| format!(".{extension}"))
                .collect::<Vec<_>>()
                .join(",")
        )
        .map_err(|error| CliError::new(format!("failed to write language list: {error}")))?;
    }
    Ok(())
}

fn write_explanation(
    code: polygl_span::DiagnosticCode,
    format: DiagnosticFormat,
    output: &mut dyn Write,
) -> Result<(), CliError> {
    let metadata = code.metadata();
    if format == DiagnosticFormat::Human {
        return writeln!(
            output,
            "{code}: {}\n\n{}\n\nproducer: {}\nfixability: {:?}\nintroduced: {}",
            metadata.title,
            metadata.description,
            metadata.producer,
            metadata.fixability,
            metadata.introduced,
        )
        .map_err(|error| CliError::new(format!("failed to write explanation: {error}")));
    }
    let value = serde_json::json!({
        "code": code.as_str(),
        "title": metadata.title,
        "description": metadata.description,
        "producer": metadata.producer,
        "fixability": format!("{:?}", metadata.fixability),
        "introduced": metadata.introduced,
    });
    serde_json::to_writer_pretty(&mut *output, &value)
        .map_err(|error| CliError::new(format!("failed to write explanation: {error}")))?;
    writeln!(output).map_err(|error| CliError::new(format!("failed to write explanation: {error}")))
}

fn new_adapter(language: &str, destination: &Path, output: &mut dyn Write) -> Result<(), CliError> {
    if destination.exists() {
        return Err(CliError::new(format!(
            "refusing to overwrite existing adapter directory {}",
            destination.display()
        )));
    }
    let source_directory = destination.join("src");
    fs::create_dir_all(&source_directory).map_err(|error| {
        CliError::new(format!(
            "failed to create adapter directory {}: {error}",
            destination.display()
        ))
    })?;
    let crate_name = format!("polygl-adapter-{language}");
    let manifest = format!(
        "[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\nlicense = \"MIT OR Apache-2.0\"\n\n[dependencies]\npolygl-adapter-api = \"={VERSION}\"\npolygl-hir = \"={VERSION}\"\npolygl-span = \"={VERSION}\"\n\n[lints.rust]\nunsafe_code = \"forbid\"\n"
    );
    let adapter_name = format!("{}Adapter", pascal_case(language));
    let implementation = format!(
        "use polygl_adapter_api::{{FeatureTag, LanguageAdapter, LowerCtx}};\nuse polygl_hir::Module;\nuse polygl_span::{{Diagnostic, Diagnostics, Severity, SourceFile, Suggestion}};\n\n#[derive(Clone, Copy, Debug, Default)]\npub struct {adapter_name};\n\nimpl LanguageAdapter for {adapter_name} {{\n    fn id(&self) -> &'static str {{\n        \"{language}\"\n    }}\n\n    fn file_extensions(&self) -> &'static [&'static str] {{\n        &[\"{language}\"]\n    }}\n\n    fn lower(\n        &self,\n        source: &SourceFile,\n        _context: &mut LowerCtx<'_>,\n    ) -> Result<Module, Diagnostics> {{\n        let span = source.span(0, source.len()).expect(\"complete source span\");\n        let mut diagnostics = Diagnostics::new();\n        diagnostics.push(\n            Diagnostic::new(\n                Severity::Error,\n                \"E0200\",\n                \"the {language} adapter lowering is not implemented\",\n                span,\n            )\n            .with_suggestion(Suggestion::rewrite(\n                span,\n                \"implement parser-specific lowering to Common Core HIR\",\n            )),\n        );\n        Err(diagnostics)\n    }}\n\n    fn capabilities(&self) -> &'static [FeatureTag] {{\n        &[]\n    }}\n}}\n"
    );
    write_file(&destination.join("Cargo.toml"), manifest.as_bytes())?;
    write_file(&source_directory.join("lib.rs"), implementation.as_bytes())?;
    writeln!(
        output,
        "created {} for the `{language}` adapter",
        destination.display()
    )
    .map_err(|error| CliError::new(format!("failed to write scaffold result: {error}")))
}

fn write_file(path: &Path, contents: &[u8]) -> Result<(), CliError> {
    fs::write(path, contents)
        .map_err(|error| CliError::new(format!("failed to write {}: {error}", path.display())))
}

fn valid_language_id(language: &str) -> bool {
    let mut characters = language.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && characters.all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
}

fn pascal_case(language: &str) -> String {
    let mut characters = language.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };
    first.to_ascii_uppercase().to_string() + characters.as_str()
}

fn usage() -> String {
    "\
usage:
  polygl build <source.rb|source.php|source.pl> [-o <directory>] [--debug | --release] [--source-map <none|external|inline>] [--sources-content]
  polygl serve <source.rb|source.php|source.pl> [--port <port>] [--watch]
  polygl check <source.rb|source.php|source.pl> [--diagnostic-format <human|json|sarif|lsp>] [--deny-warnings] [--allow <Wcode|WxxFamily>] [--deny <Wcode|WxxFamily>] [--max-warnings <count>]
  polygl dump-hir <source.rb|source.php|source.pl>
  polygl explain <diagnostic-code> [--diagnostic-format <human|json>]
  polygl languages [--json]
  polygl new-adapter <language> [-o <directory>]
  polygl --version
"
    .to_owned()
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{
        BuildMode, BuildOptions, CliError, CliErrorKind, Command, SourceMapMode, VERSION,
        parse_args, run,
    };

    static NEXT_TEMPORARY: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn prints_the_workspace_package_version() {
        assert_eq!(
            parse_args(arguments(["--version"])).unwrap(),
            Command::Version
        );
        assert_eq!(parse_args(arguments(["-V"])).unwrap(), Command::Version);

        let mut output = Vec::new();
        run(arguments(["--version"]), &mut output).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            format!("polygl {VERSION}\n")
        );
    }

    #[test]
    fn parses_build_and_source_map_modes_and_rejects_conflicts() {
        assert_eq!(
            parse_args(arguments(["build", "main.rb", "-o", "web", "--release"])).unwrap(),
            Command::Build {
                source: "main.rb".into(),
                output: "web".into(),
                options: BuildOptions {
                    mode: BuildMode::Release,
                    source_map: SourceMapMode::None,
                    sources_content: false,
                },
            }
        );
        assert_eq!(
            parse_args(arguments([
                "build",
                "main.rb",
                "--source-map",
                "inline",
                "--sources-content",
            ]))
            .unwrap(),
            Command::Build {
                source: "main.rb".into(),
                output: "dist".into(),
                options: BuildOptions {
                    mode: BuildMode::Debug,
                    source_map: SourceMapMode::Inline,
                    sources_content: true,
                },
            }
        );
        let error = parse_args(arguments(["build", "main.rb", "--debug", "--release"]))
            .unwrap_err()
            .to_string();
        assert!(error.contains("only be specified once"));
        let error = parse_args(arguments([
            "build",
            "main.rb",
            "--source-map",
            "none",
            "--sources-content",
        ]))
        .unwrap_err()
        .to_string();
        assert!(error.contains("requires an external or inline"));

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
    fn classifies_usage_parse_compile_io_and_internal_failures() {
        assert_eq!(
            run(arguments(["unknown"]), &mut Vec::new())
                .unwrap_err()
                .exit_code(),
            2
        );
        assert_eq!(
            run(
                arguments(["check", "definitely-missing.rb"]),
                &mut Vec::new()
            )
            .unwrap_err()
            .exit_code(),
            5
        );

        let temporary = temporary_directory();
        let invalid_syntax = temporary.join("syntax.rb");
        fs::write(&invalid_syntax, "def").unwrap();
        assert_eq!(
            run(
                arguments(["check", invalid_syntax.to_str().unwrap()]),
                &mut Vec::new(),
            )
            .unwrap_err()
            .exit_code(),
            3
        );
        let invalid_type = temporary.join("type.rb");
        fs::write(
            &invalid_type,
            "def setup\n  value = 1\n  value = \"text\"\nend\n",
        )
        .unwrap();
        assert_eq!(
            run(
                arguments(["check", invalid_type.to_str().unwrap()]),
                &mut Vec::new(),
            )
            .unwrap_err()
            .exit_code(),
            4
        );
        assert_eq!(
            CliError::new("internal")
                .with_kind(CliErrorKind::Internal)
                .exit_code(),
            70
        );
        fs::remove_dir_all(temporary).unwrap();
    }

    #[test]
    fn lists_languages_and_scaffolds_an_adapter_without_overwriting() {
        let mut languages = Vec::new();
        run(arguments(["languages"]), &mut languages).unwrap();
        assert_eq!(
            String::from_utf8(languages).unwrap(),
            "ruby\t.rb\nphp\t.php\nperl\t.pl\n"
        );
        let mut languages_json = Vec::new();
        run(arguments(["languages", "--json"]), &mut languages_json).unwrap();
        let languages: serde_json::Value = serde_json::from_slice(&languages_json).unwrap();
        assert_eq!(languages[0]["id"], "ruby");
        assert_eq!(languages[0]["extensions"][0], "rb");

        let temporary = temporary_directory();
        let adapter = temporary.join("polygl-adapter-toy");
        let mut output = Vec::new();
        run(
            arguments(["new-adapter", "toy", "-o", adapter.to_str().unwrap()]),
            &mut output,
        )
        .unwrap();
        let manifest = fs::read_to_string(adapter.join("Cargo.toml")).unwrap();
        assert!(manifest.contains("name = \"polygl-adapter-toy\""));
        assert!(manifest.contains("polygl-adapter-api = \"=0.1.0\""));
        assert!(manifest.contains("polygl-hir = \"=0.1.0\""));
        assert!(manifest.contains("polygl-span = \"=0.1.0\""));
        let implementation = fs::read_to_string(adapter.join("src/lib.rs")).unwrap();
        assert!(implementation.contains("pub struct ToyAdapter;"));
        assert!(implementation.contains("impl LanguageAdapter for ToyAdapter"));
        assert!(
            run(
                arguments(["new-adapter", "toy", "-o", adapter.to_str().unwrap(),]),
                &mut Vec::new(),
            )
            .unwrap_err()
            .to_string()
            .contains("refusing to overwrite")
        );
        fs::remove_dir_all(temporary).unwrap();
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
        let debug_map: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(debug.join("app.js.map")).unwrap()).unwrap();
        assert_eq!(debug_map["sources"][0], "triangle.rb");
        assert!(debug_map.get("sourcesContent").is_none());
        assert!(debug.join("runtime.js").is_file());
        let debug_shaders = fs::read_to_string(debug.join("shaders.js")).unwrap();
        assert!(debug_shaders.contains("shaderAbi:1"));
        assert!(debug_shaders.contains("debug:true"));
        assert!(debug_shaders.contains("name:\"plasma\""));
        assert!(debug_shaders.contains("name:\"u_time\""));
        assert!(debug_shaders.contains("#version 300 es"));
        assert!(
            fs::read_to_string(debug.join("index.html"))
                .unwrap()
                .contains("from \"./shaders.js\"")
        );
        let debug_manifest_bytes = fs::read(debug.join("polygl-manifest.json")).unwrap();
        let debug_manifest: serde_json::Value =
            serde_json::from_slice(&debug_manifest_bytes).unwrap();
        assert_eq!(debug_manifest["schemaVersion"], 2);
        assert_eq!(debug_manifest["compiler"]["version"], VERSION);
        assert_eq!(debug_manifest["adapter"]["id"], "ruby");
        assert_eq!(debug_manifest["adapter"]["apiVersion"], 1);
        assert_eq!(debug_manifest["runtimeAbi"], 2);
        assert_eq!(debug_manifest["shaderAbi"], 1);
        assert_eq!(debug_manifest["schemas"]["hir"], 1);
        assert_eq!(debug_manifest["schemas"]["lir"], 1);
        assert_eq!(debug_manifest["schemas"]["builtins"], 2);
        assert_eq!(debug_manifest["source"]["path"], "triangle.rb");
        assert_eq!(debug_manifest["options"]["mode"], "debug");
        assert_eq!(debug_manifest["options"]["sourceMap"], "external");
        assert_eq!(debug_manifest["options"]["sourcesContent"], false);
        let artifacts = debug_manifest["artifacts"].as_array().unwrap();
        let paths = artifacts
            .iter()
            .map(|artifact| artifact["path"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert!(paths.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(!paths.contains(&"polygl-manifest.json"));
        for artifact in artifacts {
            let contents = fs::read(debug.join(artifact["path"].as_str().unwrap())).unwrap();
            assert_eq!(artifact["size"], contents.len());
            assert_eq!(
                artifact["blake3"],
                blake3::hash(&contents).to_hex().to_string()
            );
        }

        let repeated = temporary.join("debug-repeated");
        run(
            arguments([
                "build",
                source.to_str().unwrap(),
                "-o",
                repeated.to_str().unwrap(),
            ]),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(
            debug_manifest_bytes,
            fs::read(repeated.join("polygl-manifest.json")).unwrap()
        );
        for path in paths {
            assert_eq!(
                fs::read(debug.join(path)).unwrap(),
                fs::read(repeated.join(path)).unwrap()
            );
        }

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
        let release_javascript = fs::read_to_string(release.join("app.js")).unwrap();
        assert!(!release_javascript.contains("__pglSpans"));
        assert!(!release_javascript.contains("sourceMappingURL"));
        assert!(!release.join("app.js.map").exists());
        let release_manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(release.join("polygl-manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(release_manifest["options"]["mode"], "release");
        assert_eq!(release_manifest["options"]["sourceMap"], "none");
        assert!(
            fs::read_to_string(release.join("shaders.js"))
                .unwrap()
                .contains("debug:false")
        );

        let inline = temporary.join("inline");
        run(
            arguments([
                "build",
                source.to_str().unwrap(),
                "-o",
                inline.to_str().unwrap(),
                "--release",
                "--source-map",
                "inline",
                "--sources-content",
            ]),
            &mut Vec::new(),
        )
        .unwrap();
        assert!(!inline.join("app.js.map").exists());
        assert!(
            fs::read_to_string(inline.join("app.js"))
                .unwrap()
                .contains("sourceMappingURL=data:application/json;charset=utf-8;base64,")
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
        assert!(javascript.contains("__pglRuntime.mapFromEntries"));
        assert!(javascript.contains("__pglRuntime.mapGet"));
        assert!(!javascript.contains("Object.fromEntries"));
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
        assert!(
            javascript
                .contains(r#"__pglRuntime.structFromEntries([["x", x], ["y", __pglFunction_"#)
        );
        assert!(javascript.contains("[\"x\"] ="));
        assert!(javascript.contains("__pglRuntime.circle"));
        assert!(javascript.contains("return 99;"));
        fs::remove_dir_all(temporary).unwrap();
    }

    #[test]
    fn builds_php_browser_artifacts() {
        let temporary = temporary_directory();
        let source = temporary.join("triangle.php");
        fs::write(
            &source,
            r#"<?php
function setup() {
    size(320, 180);
    background(0.1, 0.2, 0.3);
    fill(1.0, 0.0, 0.0);
    triangle(10.0, 10.0, 50.0, 10.0, 30.0, 40.0);
}
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
        assert!(javascript.contains("__pglRuntime.background"));
        assert!(javascript.contains("__pglRuntime.triangle"));
        assert!(output.join("index.html").is_file());
        fs::remove_dir_all(temporary).unwrap();
    }

    #[test]
    fn builds_perl_browser_artifacts() {
        let temporary = temporary_directory();
        let source = temporary.join("triangle.pl");
        fs::write(
            &source,
            r#"use strict;
use warnings;

sub setup {
    size(320, 180);
    background(0.1, 0.2, 0.3);
    fill(1.0, 0.0, 0.0);
    triangle(10.0, 10.0, 50.0, 10.0, 30.0, 40.0);
}
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
        assert!(javascript.contains("__pglRuntime.background"));
        assert!(javascript.contains("__pglRuntime.triangle"));
        assert!(output.join("index.html").is_file());
        fs::remove_dir_all(temporary).unwrap();
    }

    #[test]
    fn copies_literal_texture_assets_and_rejects_unpackageable_inputs() {
        let temporary = temporary_directory();
        let asset_directory = temporary.join("assets");
        fs::create_dir(&asset_directory).unwrap();
        let texture_bytes = [0_u8, 1, 2, 3, 0xff];
        fs::write(asset_directory.join("checker.bin"), texture_bytes).unwrap();
        let source = temporary.join("textured.rb");
        fs::write(
            &source,
            "def setup\n  texture_load(\"assets/checker.bin\")\nend\n",
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
        assert_eq!(
            fs::read(output.join("assets/checker.bin")).unwrap(),
            texture_bytes
        );

        let missing = temporary.join("missing.rb");
        fs::write(
            &missing,
            "def setup\n  texture_load(\"assets/missing.png\")\nend\n",
        )
        .unwrap();
        let error = run(
            arguments([
                "build",
                missing.to_str().unwrap(),
                "-o",
                temporary.join("missing-web").to_str().unwrap(),
            ]),
            &mut Vec::new(),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("failed to read texture asset"));
        assert!(error.contains("assets/missing.png"));

        let dynamic = temporary.join("dynamic.rb");
        fs::write(
            &dynamic,
            "def setup\n  path = \"assets/checker.bin\"\n  texture_load(path)\nend\n",
        )
        .unwrap();
        let error = run(
            arguments(["check", dynamic.to_str().unwrap()]),
            &mut Vec::new(),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("E0501"));
        assert!(error.contains("string literal asset path"));

        let unsafe_path = temporary.join("unsafe.rb");
        fs::write(
            &unsafe_path,
            "def setup\n  texture_load(\"../secret.png\")\nend\n",
        )
        .unwrap();
        let error = run(
            arguments(["check", unsafe_path.to_str().unwrap()]),
            &mut Vec::new(),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("E0501"));
        assert!(error.contains("not portable"));
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

        let json_error = run(
            arguments([
                "check",
                invalid.to_str().unwrap(),
                "--diagnostic-format",
                "json",
            ]),
            &mut Vec::new(),
        )
        .unwrap_err()
        .to_string();
        let json: serde_json::Value = serde_json::from_str(&json_error).unwrap();
        assert_eq!(json["diagnostics"][0]["code"], "E0200");
        assert_eq!(
            json["diagnostics"][0]["fixes"][0]["applicability"],
            "maybe-incorrect"
        );

        let colored = run(
            arguments(["check", invalid.to_str().unwrap(), "--color", "always"]),
            &mut Vec::new(),
        )
        .unwrap_err()
        .to_string();
        assert!(colored.contains("\u{1b}["));
        assert!(!json_error.contains("\u{1b}["));

        let sarif_error = run(
            arguments([
                "check",
                invalid.to_str().unwrap(),
                "--diagnostic-format",
                "sarif",
            ]),
            &mut Vec::new(),
        )
        .unwrap_err()
        .to_string();
        let sarif: serde_json::Value = serde_json::from_str(&sarif_error).unwrap();
        assert_eq!(sarif["version"], "2.1.0");
        assert_eq!(sarif["runs"][0]["results"][0]["ruleId"], "E0200");

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
        let denied = run(
            arguments(["check", warning.to_str().unwrap(), "--deny", "W04xx"]),
            &mut Vec::new(),
        )
        .unwrap_err()
        .to_string();
        assert!(denied.contains("1 denied warning"));
        run(
            arguments([
                "check",
                warning.to_str().unwrap(),
                "--deny-warnings",
                "--allow",
                "W0401",
            ]),
            &mut Vec::new(),
        )
        .unwrap();
        let too_many = run(
            arguments(["check", warning.to_str().unwrap(), "--max-warnings", "0"]),
            &mut Vec::new(),
        )
        .unwrap_err()
        .to_string();
        assert!(too_many.contains("exceeding --max-warnings 0"));

        let mut explanation = Vec::new();
        run(arguments(["explain", "E0406"]), &mut explanation).unwrap();
        let explanation = String::from_utf8(explanation).unwrap();
        assert!(explanation.contains("unsafe GPU integer divisor"));
        assert!(explanation.contains("provably nonzero"));
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
