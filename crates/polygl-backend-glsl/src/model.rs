use polygl_span::Span;
use polygl_types::Type;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GlslArtifacts {
    pub shaders: Vec<ShaderArtifact>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShaderArtifact {
    pub name: String,
    pub vertex: String,
    pub fragment: String,
    pub attributes: Vec<AttributeBinding>,
    pub uniforms: Vec<UniformBinding>,
    pub vertex_span: Span,
    pub fragment_span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttributeBinding {
    pub name: String,
    pub glsl_name: String,
    pub location: u8,
    pub ty: Type,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UniformBinding {
    pub name: String,
    pub glsl_name: String,
    pub ty: Type,
    pub source: UniformSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UniformSource {
    Automatic,
    User,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShaderStage {
    Vertex,
    Fragment,
}
