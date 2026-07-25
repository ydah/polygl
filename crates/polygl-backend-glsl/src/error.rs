use std::error::Error;
use std::fmt;

use polygl_types::Type;

#[derive(Clone, Debug, PartialEq)]
pub enum EmitError {
    IncompletePair(String),
    InvalidStageResult {
        shader: String,
        stage: &'static str,
        ty: Type,
    },
    MissingStruct(String),
    MissingStructField {
        structure: String,
        field: String,
    },
    InvalidAttribute(String),
    UnknownBinding(String),
    UnsupportedRuntimeOp(String),
    UnsupportedExpression(&'static str),
    NonFiniteFloat(f64),
}

impl fmt::Display for EmitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncompletePair(name) => {
                write!(formatter, "shader `{name}` does not contain both stages")
            }
            Self::InvalidStageResult { shader, stage, ty } => {
                write!(
                    formatter,
                    "{stage} entry for shader `{shader}` has invalid result `{ty}`"
                )
            }
            Self::MissingStruct(name) => write!(formatter, "missing GPU struct `{name}`"),
            Self::MissingStructField { structure, field } => {
                write!(
                    formatter,
                    "struct value `{structure}` does not initialize field `{field}`"
                )
            }
            Self::InvalidAttribute(name) => {
                write!(formatter, "`{name}` is not a standard vertex attribute")
            }
            Self::UnknownBinding(name) => write!(formatter, "unknown GPU binding `{name}`"),
            Self::UnsupportedRuntimeOp(name) => {
                write!(formatter, "runtime operation `{name}` has no GLSL lowering")
            }
            Self::UnsupportedExpression(kind) => {
                write!(formatter, "{kind} cannot be emitted as GLSL")
            }
            Self::NonFiniteFloat(value) => {
                write!(formatter, "float literal `{value}` is not finite f32")
            }
        }
    }
}

impl Error for EmitError {}
