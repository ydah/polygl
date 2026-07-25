# 0002: Keep structured HIR and LIR

- Status: Accepted
- Date: 2026-07-25

## Context

Adapters need a source-oriented target that preserves structured control flow,
language-specific lowering decisions, spans, and readable snapshots. The JS
and GLSL ES 3.00 backends both support structured conditions and loops, so
forcing SSA or a control-flow graph at the adapter boundary would add
complexity without enabling a current backend requirement.

Backend lowering still needs a representation in which types, monomorphized
functions, runtime operations, and Host/GPU separation are resolved.

## Decision

Use two structured IR levels. HIR is the public adapter output and retains
source spans, unresolved type expressions, structured statements, and explicit
semantic operators such as `DivInt`, `DivFloat`, and `NilCheck`. LIR remains
structured but contains analyzed, monomorphized, domain-separated operations
ready for code generation.

Keep HIR independent of core to avoid a dependency cycle. Store builtin calls
with an opaque HIR-owned `BuiltinId`; the core registry assigns and resolves
those IDs.

Normalize only semantically unordered top-level declarations. Preserve
constant-initializer order and all executable and aggregate order.

## Consequences

Adapters have a small, inspectable contract and L2/L3 can use deterministic HIR
dumps. Diagnostics and later source maps retain original positions throughout
the pipeline. Shared analysis has one place to resolve types and runtime
operations before either backend runs.

Some optimizations are less convenient than in SSA form, and malformed public
HIR must be validated by the analysis stage. A CFG/SSA representation may be
introduced later as an internal optimization form without changing the adapter
API.
