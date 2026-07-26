---
---

# 0015: Pair source maps with runtime locations

- Status: Accepted
- Date: 2026-07-25

## Context

Browser source maps make generated JavaScript debuggable, but a runtime bounds
or nil failure cannot depend on a DevTools source-map consumer being present.
The overlay needs a direct, deterministic source location. At the same time,
maintaining a second independent location system would risk reporting a
different file or line from the source map.

Debug checks must preserve source evaluation order, and release builds must be
able to remove their cost without losing ordinary source-map support.

## Decision

Require the JavaScript backend to receive the `SourceFile` set corresponding to
LIR spans. Emit an external Source Map v3 document with embedded
`sourcesContent`, using the same spans to build a frozen debug location table.
Checked runtime calls receive entries from that table rather than generated
JavaScript coordinates.

Insert collection bounds and nil-base checks only in debug mode. Evaluate a
collection-write base and index once, run the check before evaluating the
right-hand side, and then perform the write. Keep Source Map v3 output enabled
in both debug and release modes.

Treat missing sources, duplicate source identifiers, and invalid spans as
backend errors. Uniform checks will join this location contract in the shader
ABI pass rather than being synthesized by the Host backend.

## Consequences

DevTools and runtime overlays agree on original source files and positions.
Debug failures remain useful without browser tooling, while release output has
direct collection and field access. Callers must retain source files through
code generation, and the runtime must implement the small checked-access
interface.
