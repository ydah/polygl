use std::fmt;

use crate::Severity;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DiagnosticCode {
    E0001,
    E0100,
    E0200,
    E0202,
    E0203,
    E0300,
    E0301,
    E0302,
    E0303,
    E0305,
    E0306,
    E0310,
    E0311,
    E0312,
    E0313,
    E0314,
    E0401,
    E0402,
    E0403,
    E0404,
    E0405,
    E0406,
    E0501,
    W0401,
    W0402,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fixability {
    None,
    Optional,
    Required,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagnosticMetadata {
    pub severity: Severity,
    pub title: &'static str,
    pub description: &'static str,
    pub producer: &'static str,
    pub fixability: Fixability,
    pub introduced: &'static str,
}

impl DiagnosticCode {
    pub const ALL: [Self; 25] = [
        Self::E0001,
        Self::E0100,
        Self::E0200,
        Self::E0202,
        Self::E0203,
        Self::E0300,
        Self::E0301,
        Self::E0302,
        Self::E0303,
        Self::E0305,
        Self::E0306,
        Self::E0310,
        Self::E0311,
        Self::E0312,
        Self::E0313,
        Self::E0314,
        Self::E0401,
        Self::E0402,
        Self::E0403,
        Self::E0404,
        Self::E0405,
        Self::E0406,
        Self::E0501,
        Self::W0401,
        Self::W0402,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::E0001 => "E0001",
            Self::E0100 => "E0100",
            Self::E0200 => "E0200",
            Self::E0202 => "E0202",
            Self::E0203 => "E0203",
            Self::E0300 => "E0300",
            Self::E0301 => "E0301",
            Self::E0302 => "E0302",
            Self::E0303 => "E0303",
            Self::E0305 => "E0305",
            Self::E0306 => "E0306",
            Self::E0310 => "E0310",
            Self::E0311 => "E0311",
            Self::E0312 => "E0312",
            Self::E0313 => "E0313",
            Self::E0314 => "E0314",
            Self::E0401 => "E0401",
            Self::E0402 => "E0402",
            Self::E0403 => "E0403",
            Self::E0404 => "E0404",
            Self::E0405 => "E0405",
            Self::E0406 => "E0406",
            Self::E0501 => "E0501",
            Self::W0401 => "W0401",
            Self::W0402 => "W0402",
        }
    }

    #[must_use]
    pub const fn metadata(self) -> DiagnosticMetadata {
        let (severity, title, producer, fixability) = match self {
            Self::E0001 => (
                Severity::Error,
                "invalid compiler configuration",
                "compiler",
                Fixability::Optional,
            ),
            Self::E0100 => (
                Severity::Error,
                "source-language parse error",
                "adapter parser",
                Fixability::Optional,
            ),
            Self::E0200 => (
                Severity::Error,
                "syntax outside Common Core",
                "adapter lowerer",
                Fixability::Required,
            ),
            Self::E0202 => (
                Severity::Error,
                "unsupported block or closure",
                "adapter lowerer",
                Fixability::Required,
            ),
            Self::E0203 => (
                Severity::Error,
                "unsupported class feature",
                "adapter lowerer",
                Fixability::Required,
            ),
            Self::E0300 => (
                Severity::Error,
                "integer literal outside i32",
                "adapter literal lowerer",
                Fixability::Required,
            ),
            Self::E0301 => (
                Severity::Error,
                "non-boolean condition",
                "type analyzer",
                Fixability::Required,
            ),
            Self::E0302 => (
                Severity::Error,
                "loose equality",
                "adapter operator lowerer",
                Fixability::Required,
            ),
            Self::E0303 => (
                Severity::Error,
                "incompatible types",
                "type analyzer",
                Fixability::Required,
            ),
            Self::E0305 => (
                Severity::Error,
                "unknown name or field",
                "type analyzer",
                Fixability::Required,
            ),
            Self::E0306 => (
                Severity::Error,
                "invalid declaration shape",
                "type analyzer",
                Fixability::Required,
            ),
            Self::E0310 => (
                Severity::Error,
                "specialization limit exceeded",
                "type analyzer",
                Fixability::Required,
            ),
            Self::E0311 => (
                Severity::Error,
                "invalid reassignment",
                "type analyzer",
                Fixability::Required,
            ),
            Self::E0312 => (
                Severity::Error,
                "unresolved or recursive type",
                "type analyzer",
                Fixability::Required,
            ),
            Self::E0313 => (
                Severity::Error,
                "recursive specialization",
                "type analyzer",
                Fixability::Required,
            ),
            Self::E0314 => (
                Severity::Error,
                "invalid source annotation",
                "adapter annotation parser",
                Fixability::Required,
            ),
            Self::E0401 => (
                Severity::Error,
                "cyclic GPU dependency",
                "LIR split",
                Fixability::Required,
            ),
            Self::E0402 => (
                Severity::Error,
                "value has no GPU representation",
                "LIR split",
                Fixability::Required,
            ),
            Self::E0403 => (
                Severity::Error,
                "dynamic GPU collection",
                "LIR split",
                Fixability::Required,
            ),
            Self::E0404 => (
                Severity::Error,
                "Host dependency reached from GPU",
                "LIR split",
                Fixability::Required,
            ),
            Self::E0405 => (
                Severity::Error,
                "invalid shader ABI",
                "LIR split",
                Fixability::Required,
            ),
            Self::E0406 => (
                Severity::Error,
                "unsafe GPU integer divisor",
                "LIR split",
                Fixability::Required,
            ),
            Self::E0501 => (
                Severity::Error,
                "invalid public asset use",
                "LIR split",
                Fixability::Required,
            ),
            Self::W0401 => (
                Severity::Warning,
                "Host/GPU precision difference",
                "LIR split",
                Fixability::None,
            ),
            Self::W0402 => (
                Severity::Warning,
                "long GPU loop",
                "LIR split",
                Fixability::None,
            ),
        };
        DiagnosticMetadata {
            severity,
            title,
            description: self.description(),
            producer,
            fixability,
            introduced: "0.1.0",
        }
    }

    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::E0001 => "Compiler configuration is invalid or internally inconsistent.",
            Self::E0100 => "The source-language parser could not produce a valid syntax tree.",
            Self::E0200 => "Source syntax or behavior is outside the Common Core subset.",
            Self::E0202 => "A block or closure is outside the non-escaping Common Core whitelist.",
            Self::E0203 => "A class feature is outside the fixed struct-like class subset.",
            Self::E0300 => "An integer literal is outside the Common Core i32 range.",
            Self::E0301 => "A condition does not have the required bool type.",
            Self::E0302 => {
                "Loose equality is unavailable and must be rewritten as strict equality."
            }
            Self::E0303 => "Inferred and required types are incompatible.",
            Self::E0305 => "A referenced name, type, field, or function is unknown.",
            Self::E0306 => "A declaration, field set, or argument list has an invalid shape.",
            Self::E0310 => "A function exceeded the per-function specialization limit.",
            Self::E0311 => "A reassignment changes a binding type or writes a constant.",
            Self::E0312 => "A type remains unresolved or would recursively contain itself.",
            Self::E0313 => "A recursive function specialization cannot be inferred safely.",
            Self::E0314 => "A source annotation is malformed, misplaced, or unmatched.",
            Self::E0401 => "A recursive or cyclic dependency is reachable from GPU code.",
            Self::E0402 => "A value or type has no representation in the GPU subset.",
            Self::E0403 => "GPU code attempts to use dynamically sized collection storage.",
            Self::E0404 => "A Host-only declaration or builtin is reachable from GPU code.",
            Self::E0405 => {
                "A shader pair, stage, varying, attribute, or material violates the shader ABI."
            }
            Self::E0406 => "An integer divisor in GPU code is not provably nonzero.",
            Self::E0501 => {
                "A public asset path is dynamic, non-relative, non-portable, or colliding."
            }
            Self::W0401 => "Shared float code may differ between Host f64 and GPU f32 execution.",
            Self::W0402 => "A compiler-visible GPU loop exceeds the portable iteration threshold.",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|code| code.as_str() == value)
    }

    #[must_use]
    pub fn starts_with(self, prefix: &str) -> bool {
        self.as_str().starts_with(prefix)
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl From<&'static str> for DiagnosticCode {
    fn from(value: &'static str) -> Self {
        Self::parse(value).unwrap_or_else(|| panic!("unregistered diagnostic code `{value}`"))
    }
}

impl PartialEq<str> for DiagnosticCode {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for DiagnosticCode {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::DiagnosticCode;
    use crate::Severity;

    #[test]
    fn registry_codes_are_unique_and_match_their_severity() {
        let mut names = HashSet::new();
        for code in DiagnosticCode::ALL {
            assert!(names.insert(code.as_str()));
            assert_eq!(
                code.metadata().severity,
                if code.as_str().starts_with('E') {
                    Severity::Error
                } else {
                    Severity::Warning
                }
            );
            assert!(!code.metadata().title.is_empty());
            assert!(!code.metadata().description.is_empty());
            assert!(!code.metadata().producer.is_empty());
        }
    }
}
