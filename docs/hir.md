# High-level intermediate representation

HIR is the sole output contract of language adapters. It is source-oriented
enough for diagnostics and snapshots, but every node has language-independent
semantics. Source-language behavior is expanded before HIR reaches shared
analysis.

## Ownership and dependency direction

`polygl-span` defines source identity and byte spans. `polygl-hir` depends only
on that foundation. `polygl-core` owns the builtin registry and depends on HIR;
HIR must never depend on core, adapters, types, LIR, or a backend.

Builtin calls carry an opaque `BuiltinId`. Raw numeric construction is not
public; production adapters obtain the selected ID through the core registry.
The closed ID set lives in HIR so a call can name a builtin without introducing
a core↔HIR dependency cycle.

## Module schema

A `Module` contains four item kinds:

- `Function`: parameters, optional source type expressions, a body, and a
  Host/GPU/Auto domain hint;
- `StructDef`: fixed fields and statically dispatched instance methods;
- `ConstDef`: a named compile-time value; and
- `EntryPoint`: canonical setup/frame/event or named vertex/fragment body.

Statements retain structured control flow: binding, assignment, expression,
`if`, `while`, range `for`, return, break, and continue. HIR is not SSA and does
not contain closures.

Expressions include typed literals, variables, binary/unary operations, user
and builtin calls, index/field access, homogeneous arrays/maps, struct/vector
construction, and `NilCheck`. `DivInt` and `DivFloat` are distinct operators;
adapters choose one according to `docs/common-core.md`. String concatenation is
also distinct from numeric addition. `FalsyCheck` has the fixed meaning “nil or
false” and is the single-evaluation target for Ruby truthiness lowering.

Every module, item, parameter, type expression, block, statement, place, range,
expression, map entry, and field initializer carries a validated half-open
byte `Span`. Dump output deliberately omits spans so equivalent programs in
different source languages can be compared; the in-memory tree retains them.

## Builders

`HirBuilder` supplies a default validated span and concise constructors for
hand-written HIR, adapter unit tests, and conformance fixtures. Production
adapters may construct public nodes directly when child nodes have different
source spans.

## Dump and normalization

`dump` emits deterministic, human-readable text and distinguishes semantically
different operators such as `/int` and `/float`. It is a snapshot/debug format,
not a source language and not a stable serialization protocol.

`Module::normalize` performs only semantics-preserving top-level declaration
ordering. Constants retain their relative order because initializers may
evaluate calls. It never reorders statements, arguments, operands, aggregate
entries, fields, or methods because those may expose evaluation or layout order.
`normalized_dump` normalizes a clone and therefore does not mutate adapter
output. Normalization is idempotent.

L2 stores ordinary language-specific dumps. L3 compares normalized dumps only
for the Neutral subset defined by `docs/common-core.md`.
