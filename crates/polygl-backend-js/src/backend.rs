use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use polygl_lir::Module;
use polygl_span::SourceFile;

use crate::EmitError;
use crate::emitter::Emitter;
use crate::source_map::SourceCatalog;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BuildMode {
    #[default]
    Debug,
    Release,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SourceMapMode {
    None,
    #[default]
    External,
    Inline,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Artifacts {
    pub javascript: String,
    pub source_map: Option<String>,
}

pub trait Backend {
    fn emit(
        &self,
        program: &Module,
        sources: &[SourceFile],
        output: &mut Artifacts,
    ) -> Result<(), EmitError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JavaScriptBackend {
    mode: BuildMode,
    output_name: String,
    runtime_module: String,
    source_map_mode: SourceMapMode,
    include_sources_content: bool,
}

impl JavaScriptBackend {
    #[must_use]
    pub fn new(mode: BuildMode) -> Self {
        Self {
            mode,
            output_name: "app.js".to_owned(),
            runtime_module: "./runtime.js".to_owned(),
            source_map_mode: SourceMapMode::External,
            include_sources_content: true,
        }
    }

    #[must_use]
    pub fn with_output_name(mut self, output_name: impl Into<String>) -> Self {
        self.output_name = output_name.into();
        self
    }

    #[must_use]
    pub fn with_runtime_module(mut self, runtime_module: impl Into<String>) -> Self {
        self.runtime_module = runtime_module.into();
        self
    }

    #[must_use]
    pub const fn with_source_map_mode(mut self, mode: SourceMapMode) -> Self {
        self.source_map_mode = mode;
        self
    }

    #[must_use]
    pub const fn with_sources_content(mut self, include: bool) -> Self {
        self.include_sources_content = include;
        self
    }

    pub fn generate(
        &self,
        program: &Module,
        sources: &[SourceFile],
    ) -> Result<Artifacts, EmitError> {
        let mut output = Artifacts::default();
        self.emit(program, sources, &mut output)?;
        Ok(output)
    }
}

impl Default for JavaScriptBackend {
    fn default() -> Self {
        Self::new(BuildMode::Debug)
    }
}

impl Backend for JavaScriptBackend {
    fn emit(
        &self,
        program: &Module,
        sources: &[SourceFile],
        output: &mut Artifacts,
    ) -> Result<(), EmitError> {
        let catalog = SourceCatalog::new(sources)?;
        let emitted = Emitter::new(self.mode, &catalog).emit(program)?;
        let header = emitted.header(&self.runtime_module, &catalog)?;
        let header_lines = header.bytes().filter(|byte| *byte == b'\n').count();
        let mut javascript = format!("{header}{}\n", emitted.body.trim_end());
        match self.source_map_mode {
            SourceMapMode::None => output.source_map = None,
            SourceMapMode::External => {
                output.source_map = Some(emitted.mappings.to_json(
                    &self.output_name,
                    header_lines,
                    &catalog,
                    self.include_sources_content,
                )?);
                javascript.push_str(&format!("//# sourceMappingURL={}.map\n", self.output_name));
            }
            SourceMapMode::Inline => {
                let source_map = emitted.mappings.to_json(
                    &self.output_name,
                    header_lines,
                    &catalog,
                    self.include_sources_content,
                )?;
                output.source_map = None;
                javascript
                    .push_str("//# sourceMappingURL=data:application/json;charset=utf-8;base64,");
                javascript.push_str(&STANDARD.encode(source_map));
                javascript.push('\n');
            }
        }
        output.javascript = javascript;
        Ok(())
    }
}
