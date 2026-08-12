---
---

# Diagnostic codes

PolyGL diagnostics use stable codes so editor integrations and conformance
tests do not need to match prose. Error messages may become more specific
without changing a code. A code changes only when the category or required
source rewrite changes.

`polygl_span::DiagnosticCode` and its metadata are the normative registry.
Every diagnostic stores that closed enum rather than an arbitrary string; the
registry fixes severity, title, producer, fixability, and introduction version.
Conformance manifests reject expected codes that are not in the registry.

## Code ranges

| Range | Owner |
|---|---|
| E00xx | compiler configuration |
| E01xx | source parsing |
| E02xx | syntax outside Common Core |
| E03xx | literals, names, types, and specialization |
| E04xx | GPU subset and shader ABI |
| E05xx | public API and asset misuse |
| W03xx | numeric portability (unassigned) |
| W04xx | GPU precision and performance |

Codes not listed below are unassigned and must not be emitted. E02xx errors
always carry a human-applicable rewrite suggestion. Other errors carry a
suggestion when the compiler can identify a safe or useful correction. Parser
errors may omit one because the originating parser often cannot determine the
intended syntax. Warnings use notes when there is no single source rewrite.

## General and adapter diagnostics

| Code | Meaning | Suggestion |
|---|---|---|
| E0001 | invalid compiler configuration | not required |
| E0100 | source-language parse error | parser-dependent |
| E0200 | source syntax or behavior is outside Common Core | required |
| E0202 | block or closure use is outside the non-escaping whitelist | required |
| E0203 | class feature is outside the fixed struct-like subset | required |

## Type diagnostics

| Code | Meaning | Suggestion |
|---|---|---|
| E0300 | integer literal is outside the Common Core i32 range | required |
| E0301 | condition does not have type `bool` | required |
| E0302 | loose equality is unavailable; reserved for adapters such as PHP | required |
| E0303 | inferred and required types are incompatible | required |
| E0305 | referenced name, type, field, or function is unknown | required |
| E0306 | declaration shape, field set, or argument count is invalid | required |
| E0310 | a function exceeds the per-function specialization limit | required |
| E0311 | reassignment changes a binding's type or writes a constant | required |
| E0312 | a type remains unresolved or would contain itself | required |
| E0313 | recursive function specialization cannot be inferred | required |
| E0314 | a source annotation is malformed, misplaced, or unmatched | required |

E0302 is assigned by the Common Core contract but has no producer until an
adapter with loose equality is enabled. It must not be used for typed equality
mismatches; those are E0303.

## GPU and shader diagnostics

| Code | Meaning | Suggestion |
|---|---|---|
| E0401 | recursive or cyclic GPU dependency | required |
| E0402 | value or type has no GPU representation | required |
| E0403 | dynamic collection storage is used in GPU code | required |
| E0404 | Host-only declaration or builtin is reached from GPU code | required |
| E0405 | shader pair, stage, varying ABI, attribute, or material reference is invalid | required |
| E0406 | integer divisor in GPU code is not provably nonzero | required |
| W0401 | shared float code may differ between Host f64 and GPU f32 | note |
| W0402 | compiler-visible GPU loop exceeds 1024 iterations | note |

## Public API diagnostics

| Code | Meaning | Suggestion |
|---|---|---|
| E0501 | `texture_load` path is dynamic, non-relative, non-portable, or collides with a generated artifact | required |

## Suggestions

A replacement attached to a diagnostic is machine-applicable, including an
empty replacement for deletion. A rewrite without replacement is guidance for
a human edit. Tools must not present human-applicable guidance as an automatic
fix.

Every adapter rejection in E0200–E0299 must explain how to express the same
intent with accepted syntax, or how to move the behavior outside the compiled
program. New adapters should route these diagnostics through one helper so the
suggestion requirement cannot be forgotten.
