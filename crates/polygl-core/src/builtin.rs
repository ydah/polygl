use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltinTier {
    Core,
    Tier1,
}

impl BuiltinTier {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Core => "Core",
            Self::Tier1 => "Tier 1",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Domain {
    Host,
    Gpu,
    Both,
}

impl Domain {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Host => "Host",
            Self::Gpu => "GPU",
            Self::Both => "Host/GPU",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltinType {
    Void,
    Int,
    Float,
    Bool,
    Str,
}

impl BuiltinType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::Int => "int",
            Self::Float => "float",
            Self::Bool => "bool",
            Self::Str => "str",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DefaultValue {
    Int(i32),
    Float(f64),
    Bool(bool),
}

impl fmt::Display for DefaultValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(value) => value.fmt(formatter),
            Self::Float(value) => value.fmt(formatter),
            Self::Bool(value) => value.fmt(formatter),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parameter {
    pub name: &'static str,
    pub ty: BuiltinType,
    pub default: Option<DefaultValue>,
}

impl Parameter {
    #[must_use]
    pub const fn required(name: &'static str, ty: BuiltinType) -> Self {
        Self {
            name,
            ty,
            default: None,
        }
    }

    #[must_use]
    pub const fn optional(name: &'static str, ty: BuiltinType, default: DefaultValue) -> Self {
        Self {
            name,
            ty,
            default: Some(default),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Signature {
    pub params: &'static [Parameter],
    pub result: BuiltinType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RuntimeOp(&'static str);

impl RuntimeOp {
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Builtin {
    pub id: BuiltinId,
    pub name: &'static str,
    pub tier: BuiltinTier,
    pub signature: Signature,
    pub domain: Domain,
    pub runtime_op: RuntimeOp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltinId {
    Floor,
    Round,
    Trunc,
    Size,
    Background,
    Fill,
    Stroke,
    NoStroke,
    Rect,
    Circle,
    Line,
    Triangle,
    Text,
    PushMatrix,
    PopMatrix,
    Translate,
    Rotate,
    Scale,
    Width,
    Height,
    Time,
    MouseX,
    MouseY,
    KeyDown,
    Random,
}
