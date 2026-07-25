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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Artifacts {
    pub javascript: String,
    pub source_map: String,
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
}

impl JavaScriptBackend {
    #[must_use]
    pub fn new(mode: BuildMode) -> Self {
        Self {
            mode,
            output_name: "app.js".to_owned(),
            runtime_module: "./runtime.js".to_owned(),
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
        let source_map_name = format!("{}.map", self.output_name);

        output.source_map = emitted
            .mappings
            .to_json(&self.output_name, header_lines, &catalog)?;
        output.javascript = format!(
            "{header}{}\n//# sourceMappingURL={source_map_name}\n",
            emitted.body.trim_end()
        );
        Ok(())
    }
}
