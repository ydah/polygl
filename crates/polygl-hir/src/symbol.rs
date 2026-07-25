use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(String);

impl Symbol {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl From<&str> for Symbol {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for Symbol {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Stable registry identity embedded in HIR builtin calls.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BuiltinId(u16);

impl BuiltinId {
    pub const FLOOR: Self = Self(0);
    pub const ROUND: Self = Self(1);
    pub const TRUNC: Self = Self(2);
    pub const SIZE: Self = Self(3);
    pub const BACKGROUND: Self = Self(4);
    pub const FILL: Self = Self(5);
    pub const STROKE: Self = Self(6);
    pub const NO_STROKE: Self = Self(7);
    pub const RECT: Self = Self(8);
    pub const CIRCLE: Self = Self(9);
    pub const LINE: Self = Self(10);
    pub const TRIANGLE: Self = Self(11);
    pub const TEXT: Self = Self(12);
    pub const PUSH_MATRIX: Self = Self(13);
    pub const POP_MATRIX: Self = Self(14);
    pub const TRANSLATE: Self = Self(15);
    pub const ROTATE: Self = Self(16);
    pub const SCALE: Self = Self(17);
    pub const WIDTH: Self = Self(18);
    pub const HEIGHT: Self = Self(19);
    pub const TIME: Self = Self(20);
    pub const MOUSE_X: Self = Self(21);
    pub const MOUSE_Y: Self = Self(22);
    pub const KEY_DOWN: Self = Self(23);
    pub const RANDOM: Self = Self(24);

    pub const ALL: [Self; 25] = [
        Self::FLOOR,
        Self::ROUND,
        Self::TRUNC,
        Self::SIZE,
        Self::BACKGROUND,
        Self::FILL,
        Self::STROKE,
        Self::NO_STROKE,
        Self::RECT,
        Self::CIRCLE,
        Self::LINE,
        Self::TRIANGLE,
        Self::TEXT,
        Self::PUSH_MATRIX,
        Self::POP_MATRIX,
        Self::TRANSLATE,
        Self::ROTATE,
        Self::SCALE,
        Self::WIDTH,
        Self::HEIGHT,
        Self::TIME,
        Self::MOUSE_X,
        Self::MOUSE_Y,
        Self::KEY_DOWN,
        Self::RANDOM,
    ];

    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }
}
