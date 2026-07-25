# 0014: Separate the builtin registry from pipeline orchestration

- Status: Accepted
- Date: 2026-07-25

## Context

Type inference needs canonical builtin signatures to constrain HIR calls.
Pipeline orchestration will in turn depend on type inference. Keeping
`BuiltinTable` inside `polygl-core` would therefore require
`polygl-types -> polygl-core -> polygl-types` once the pipeline is assembled.

HIR cannot own the registry because HIR deliberately contains only opaque
`BuiltinId` values and must remain independent of adapters, analysis, and
runtime metadata. Duplicating signatures in the type checker would violate the
single-source-of-truth contract and allow compiler/runtime drift.

## Decision

Create `polygl-builtins` as the canonical owner of builtin signatures, builtin
struct schemas, domains, defaults, runtime operation names, validation, and the
`BuiltinResolver` implementation. It depends only on the adapter API and HIR.

Make `polygl-types` depend directly on `polygl-builtins`. Keep compatibility
re-exports in `polygl-core` while core remains the public orchestration facade.
The intended dependency direction is:

`polygl-hir <- polygl-adapter-api <- polygl-builtins <- polygl-types <- polygl-core`

## Consequences

Pipeline orchestration can depend on type inference without a crate cycle, and
all stages still consume one validated registry. Adapters can continue to
receive the resolver through the core facade, while analysis code declares its
actual metadata dependency directly.

The workspace gains one small crate and core temporarily exposes types it does
not own. Removing those compatibility re-exports would be a later public API
change.
