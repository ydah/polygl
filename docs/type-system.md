# Type inference and specialization

`polygl-types` turns adapter-produced HIR into fully typed HIR. Successful
analysis returns `TypedModule`; failure returns positioned diagnostics and no
partially typed module.

## Public API

```rust
let typed = polygl_types::analyze(&hir)?;
let hir = typed.as_hir();
```

`analyze_with_options` accepts an `AnalyzeOptions` value when the per-function
instance limit must be changed. The default limit is eight. A zero limit is an
invalid configuration.

Every expression in a successful `TypedModule` has its `Expr::ty` slot filled.
Bindings, parameters, function results, and entry-point parameters also have
concrete type expressions. Source function templates are replaced by their
reachable specialized instances; unused templates are omitted.

Input annotations are validated before inference. Named struct types must refer
to a module definition (or the builtin `Event` type), vectors and matrices must
have dimensions from two through four, and `void` is accepted only as a
function result rather than as a value type. Functions, structs, constants, and
entry points each use a unique module-level namespace; parameter and struct
field names must be unique within their declaration.

`Event` is a reserved builtin struct. Its `kind`, `x`, `y`, and `key` fields are
typed from the canonical builtin registry and user modules cannot redefine it.

## Inference rules

Inference is local and bidirectional:

- literals and source annotations provide concrete types;
- builtin signatures constrain their arguments and results;
- later uses can constrain an earlier local binding;
- empty aggregates remain unresolved until a use or annotation fixes their
  element type;
- `nil` remains unresolved until context fixes the contained option type;
- control-flow joins must agree, except that `int` may widen to `float`;
- reassignment may preserve a type or widen `int` to `float`, but cannot change
  to an unrelated type;
- source annotations remain invariant when a use only needs an `int`-to-`float`
  coercion;
- parameters are call-by-value bindings: constraints and reassignments inside a
  function may widen an unannotated parameter without changing the caller;
- constants and values reachable through a constant assignment place are
  immutable, while a same-named local may shadow a constant;
- conditions must be `bool` after language-specific adapter expansion.

Struct fields without source annotations share inference variables across
constructor expressions, field reads, and field writes. Before typed HIR is
returned, every field must resolve to one concrete value type. Instance-method
calls use the receiver's resolved struct type to select a class-qualified
template, insert the receiver as the first `self` argument, and then use the
same call-site specialization path as ordinary functions. Typed HIR contains
only the resulting static function calls and field-complete structs.

Typed rewriting resolves source-independent operations. `Add` with string
operands becomes `StrConcat`, and `DivInt` becomes `DivFloat` when its inferred
operands require floating-point division.

A side-effect expression in return position may have type `void`; it determines
the function's `void` result without becoming a first-class value. Other value
positions reject `void`, including bindings, arguments, and aggregate members.

## Call-site specialization

Each reachable user-function call is specialized by its concrete argument-type
tuple. Parameter annotations and constraints discovered in the function body
normalize the tuple before it is used as a cache key, so an `int` passed to an
annotated or body-constrained `float` parameter shares the same instance as a
`float` argument. Generated names use stable type
suffixes such as `__pgl_4_half__int` and `__pgl_4_half__float`. Source-name and
tuple components are length-delimited so distinct source signatures cannot
produce the same generated symbol.

Recursive specialization is rejected because a recursive result cannot yet be
solved independently. The default maximum is eight instances per source
function. Add annotations or split a polymorphic function when this limit is
reached.

## Ruby annotation directives

Place an annotation immediately before a function parameter declaration or the
first assignment to a local:

```ruby
# @pgl radius: float
radius = 12
```

Supported spellings are:

- `int`, `float`, `bool`, and `str`;
- `T[]`, `Map<str, T>`, and `Option<T>`;
- `vec2` through `vec4` and `mat2` through `mat4`;
- `Mesh`, `Node`, `Material`, and `Texture`;
- a named struct beginning with an uppercase letter.

Malformed directives and directives that do not match a later declaration are
errors rather than silently ignored comments.

## Diagnostics

| Code | Meaning |
|---|---|
| E0301 | condition is not `bool` |
| E0303 | inferred and required types do not match |
| E0305 | unknown variable or function |
| E0306 | wrong argument count |
| E0310 | per-function specialization limit exceeded |
| E0311 | reassignment changes a variable type |
| E0312 | type remains unresolved or is recursive within itself |
| E0313 | recursive function specialization cannot be inferred |
| E0314 | malformed or unmatched source annotation |

The accepted strategy and its tradeoffs are recorded in
[ADR 0009](decisions/0009-type-inference-strategy.md).
