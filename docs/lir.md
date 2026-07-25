# Low-level intermediate representation

`polygl-lir` is the backend input boundary. `polygl_lir::lower` accepts only a
checked `polygl_types::TypedModule`, so successful LIR has concrete expression,
binding, parameter, field, constant, and function-result types.

## Structure

LIR retains blocks, `if`, `while`, range `for`, `break`, and `continue`.
Functions carry a resolved `Host`, `Gpu`, or `Shared` domain and user calls name
their generated monomorphic target. Resolution combines explicit hints,
entry-point reachability, builtin domains, constant dependencies, and
transitive user calls. Constants also carry a resolved domain, and constant
references are distinct from shadowing local-variable references. A
builtin-constrained function or constant retains that constraint even when an
invalid cross-domain use reaches it, leaving the later GPU validation pass able
to diagnose the domain mismatch. Entries have their canonical kind and domain.
Source spans remain on modules, declarations, blocks, statements, places,
ranges, expressions, map entries, and field initializers for source maps and
debug checks.

LIR does not expose `BuiltinId`. Lowering resolves every builtin through
`polygl-builtins` and stores its `RuntimeOp`; omitted optional arguments are
materialized from the validated registry defaults. This leaves code generators
independent of source-language aliases and default-argument rules.

A typed `return` whose expression has type `void` is normalized into an
expression statement followed by an empty return. This preserves side effects
while keeping `void` out of value positions.

## Minimal optimization

Lowering folds literal arithmetic, comparisons, boolean operations, string
concatenation, and nil/falsy checks when the result is unambiguous. Float
folding is limited to resolved Host code, whose f64 semantics match the
compiler; GPU and Shared float expressions remain intact because GPU execution
uses f32. Signed integer overflow, non-finite float results, division, and
remainder are left for the runtime so lowering does not invent behavior or
erase runtime failures. Expression statements that are only a literal or
variable read are removed; calls, indexing, field access, and arithmetic
statements are retained because they may have effects or debug checks.

The IR remains structured rather than SSA as decided in
[ADR 0002](decisions/0002-two-level-structured-ir.md).
