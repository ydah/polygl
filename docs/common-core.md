# PolyGL Common Core v1

This document is the normative language contract between adapters and PolyGL's
shared HIR pipeline. Source-language syntax may differ, but an adapter must
either lower a construct to the semantics below or reject it with a positioned
diagnostic. It must not silently emulate additional source-language behavior.

The keywords **must**, **must not**, **should**, and **may** are normative.

## Program model

A v1 program consists of one UTF-8 entry file. Imports, includes, requires, and
language module systems are outside the Common Core.

The file may define:

- functions with positional parameters and an optional return value;
- the `setup`, `frame`, and `on_event` host entry points;
- named `vertex_*` and `fragment_*` GPU entry points;
- constants and local variables;
- struct-like classes as constrained below; and
- executable entry-point bodies composed from Common Core statements.

The shared HIR has fixed semantics. Adapters are responsible for parsing,
source-language name aliases, desugaring, and preserving intentional language
differences. Every emitted HIR node must retain a source span.

### Entry points

| Canonical name | Signature and invocation | Domain |
|---|---|---|
| `setup` | `setup() -> void`; called once. Assets requested here finish loading before the first frame. | Host |
| `frame` | `frame(dt: float) -> void`; called by `requestAnimationFrame`; `dt` is elapsed seconds. | Host |
| `on_event` | `on_event(ev: Event) -> void`; called for normalized input events. | Host |
| `vertex_<name>` | Named vertex shader entry point; its complete ABI is defined by `docs/shader-abi.md`. | GPU |
| `fragment_<name>` | Matching named fragment shader entry point; its complete ABI is defined by `docs/shader-abi.md`. | GPU |

`Event` is a builtin struct with at least `kind: str`, `x: float`, `y: float`,
and `key: Option<str>`. The shared API specification may add fields without
changing the invocation contract.

An adapter may accept conventional aliases, such as Ruby `draw` for `frame`,
but must lower them to the canonical names. Every alias must be listed in
`docs/adapters/<language>.md`; an undocumented alias is non-conforming.

## Values and types

The v1 value types are:

| Type | Contract |
|---|---|
| `int` | Signed 32-bit two's-complement integer. Arithmetic wraps modulo 2³². |
| `float` | Host values are IEEE-754 f64; GPU values are IEEE-754 f32. |
| `bool` | Exactly `true` or `false`; it is not interchangeable with an integer. |
| `str` | UTF-8 text. Strings are host-only. |
| `T[]` | Zero-based, ordered, homogeneous array. |
| `Map<str, T>` | String-keyed map with one homogeneous value type. |
| `Option<T>` | Either a value of `T` or the absence value. |
| struct | A statically known set of named fields. |
| `vec2`…`vec4`, `mat2`…`mat4` | Numeric graphics values. |

Mesh, node, material, and texture handles are introduced with the Tier 2 API;
they are opaque types and are never interchangeable with `int`.

Only `int` to `float` is an implicit conversion. It is permitted in assignment,
argument passing, return values, and mixed numeric arithmetic. Comparisons do
not widen: both operands must already have the same type. `float` to `int`
requires `floor`, `round`, or `trunc`. String/numeric conversion is never
implicit and must produce an E03xx diagnostic with an explicit-conversion
suggestion.

Integer operations that may overflow should produce a W03xx warning. A function
reachable from both host and GPU domains should produce a W04xx precision
warning when its result can differ because of f64/f32 evaluation.

## Expressions and statements

The Common Core admits:

- scalar, array, map, and struct literals;
- local binding, assignment, indexing, and field access;
- numeric arithmetic and typed comparison;
- boolean negation and short-circuit conjunction/disjunction;
- calls to user functions and declared builtins;
- `if`/`else`, `while`, ascending range `for`, `break`, `continue`, and
  `return`; and
- function, entry-point, constant, and struct-like class definitions.

Arguments and operands are evaluated left to right. Conditions in HIR must have
type `bool`. Equality and ordered-comparison operands must have the same type;
source code must convert one side explicitly when the types differ. Arrays and
maps keep their element/value type after initialization; reassignment cannot
change a variable's type except for the permitted `int`-to-`float` widening.

Empty aggregate literals are inferred from their use. If their type remains
unknown, the adapter must require the language-specific `@pgl` annotation.
User functions are monomorphized per concrete argument-type tuple as specified
by [ADR 0009](decisions/0009-type-inference-strategy.md).

## Fixed language differences

Adapters must make the following differences explicit while lowering:

| Construct | Ruby | PHP | Perl |
|---|---|---|---|
| Integer operands to `/` | `DivInt`; quotient rounds toward negative infinity | `DivFloat` | `DivFloat` |
| A `/` operand is `float` | `DivFloat` | `DivFloat` | `DivFloat` |
| Non-`bool` condition | Expand to “not nil and not false” | E0301 plus an explicit-comparison suggestion | E0301 plus an explicit-comparison suggestion |
| String concatenation | `+` when both operands are `str` | `.` | `.` |
| Absence value | `nil` → `Option<T>` | `null` → `Option<T>` | `undef` → `Option<T>` |
| Absence test | Lower to `NilCheck` | Lower to `NilCheck` | Lower to `NilCheck` |
| Equality | Same-type comparison | Strict equality only; `==` is E0302 with a `===` suggestion | Same-type comparison |

Ruby truthiness expansion is adapter-generated `not FalsyCheck(value)` HIR.
`FalsyCheck` evaluates its operand once and is true only for nil or false. It is
not a configurable general truthiness operation. PHP and Perl rules such as
`"0"` being false are deliberately not reproduced. Backends therefore only
receive boolean conditions.

Language-specific integer, string, or collection behavior not listed here is
not inherited automatically. If the fixed HIR cannot preserve a source
construct, the adapter must reject it rather than approximate it.

## Blocks and closures

HIR has no closure value. v1 adapters may desugar only this whitelist:

- Ruby `n.times { |i| ... }` to an integer loop over `0 <= i < n`; non-positive
  `n` executes zero iterations.
- Range `each` to an ascending range loop while preserving whether the source
  endpoint is inclusive or exclusive.
- Array `each` to an index-ascending loop that binds each element.

The block body may capture surrounding locals only when expansion yields normal
lexical variable access and the block does not escape. Storing, returning,
passing as an ordinary value, or invoking a block later is E0202 with a
loop/function rewrite suggestion.

`map`, `filter`, and any further block forms are not in v1. Adding one changes
the Common Core and requires an RFC, a FeatureTag, adapter documentation, and
conformance cases.

## Struct-like classes

A v1 class may contain:

- instance fields established by construction;
- one constructor (`initialize` in Ruby or `__construct` in PHP); and
- instance methods.

All instances of a class must have the same statically known field names and
field types. Construction lowers to a `StructDef`; each method lowers to a
statically dispatched function whose first parameter is `self`. Method values
and dynamic dispatch are not supported.

Inheritance, interfaces/traits, mixins, static members, visibility semantics,
reflection, dynamic method definition, and method-missing behavior are E0203.
The diagnostic must include a suggestion to use composition, a plain field, or
a top-level function, as applicable.

## Unsupported constructs and diagnostics

The following are outside v1:

- multiple source files and imports;
- `eval`, reflection, runtime code generation, and variable-variable access;
- escaping closures or general higher-order functions;
- source-language standard-library facilities such as file and network I/O;
- heterogeneous arrays/maps and type-changing reassignment; and
- the class features excluded above.

Parse failures use E01xx, unsupported Common Core constructs use E02xx, and type
violations use E03xx. Every diagnostic must identify its source span. Every
E02xx diagnostic must also carry a concrete replacement or rewrite suggestion.
Warnings about intentional numeric incompatibility use W03xx or W04xx.

### Runtime locations

Source spans must survive HIR analysis, LIR lowering, and code generation. The
JavaScript backend must emit Source Map v3 data and an embedded span table for
checks that report through the runtime overlay. In debug builds, array bounds,
nil access, and unset-uniform failures must identify the original source file
and one-based line. Other compiler-generated runtime failures must use the same
mapping when a source span is available. Release mode may remove the mandated
debug checks but must not replace a known source location with a generated
JavaScript location.

## Conformance

Adapters declare supported syntactic sugar through FeatureTags. Declaring a tag
opts the adapter into its corresponding cases.

- L1 rendering is the primary equivalence criterion. Randomness is seeded and
  time is mocked.
- L2 stores a separate HIR snapshot for each language and case.
- L3 compares normalized HIR only for the Neutral subset: float arithmetic,
  explicit boolean conditions, strict typed comparisons, and no
  language-specific sugar.

Division and truthiness cases that intentionally produce different HIR belong
in L1/L2 and must assert the specified difference. Unsupported constructs have
diagnostic conformance cases in addition to these three layers.

## Changing this contract

Common Core additions affect every adapter, the type system, both backends, and
conformance. They require an accepted ADR or RFC, updated language mapping
tables, and tests in every affected conformance layer.
