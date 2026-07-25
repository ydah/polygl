# Ruby adapter

The Ruby adapter parses UTF-8 source with Prism 1.9.0 and lowers accepted syntax
directly to source-spanned HIR. The adapter accepts one source file and does not
execute Ruby code or load gems.

## Common Core mapping

| Common Core element | Ruby syntax | HIR lowering |
|---|---|---|
| Function | `def name(args) ... end` | `Item::Function` |
| Setup entry | `def setup ... end` | `EntryPoint::Setup` |
| Frame entry | `def frame(dt)` or `def draw(dt)` | `EntryPoint::Frame` |
| Event entry | `def on_event(event)` | `EntryPoint::OnEvent` |
| Shader entry | `def vertex_name` / `def fragment_name` | named GPU entry |
| Local declaration/write | `name = value` | first write is `Let`, later writes are `Assign` |
| Conditional | `if` / `elsif` / `else` | structured `If` |
| Loop | `while condition` | structured `While` |
| Loop control | `break` / `next` | `Break` / `Continue` |
| Function result | explicit `return` or final expression | `Return` in ordinary functions |
| Call | `name(args)` | builtin ID when registered, otherwise user function |

M1 supports required positional parameters, integer and float literals, UTF-8
strings, booleans, `nil`, local variables, arithmetic, comparison, boolean
operators, parentheses, and plain calls.

Place `# @pgl name: type` immediately before the containing `def` for a
parameter, or immediately before a local variable's first assignment, when
inference needs help. Scalar, array, `Map<str, T>`, `Option<T>`, vector, matrix,
opaque graphics handle, and named struct types are accepted. Invalid, unused,
or non-adjacent directives produce E0314.

## Semantic expansion

- Ruby `/` initially lowers to `DivInt`. Type inference may rewrite it to
  `DivFloat` when either operand is inferred as float.
- Every condition lowers to `not FalsyCheck(value)`. `FalsyCheck` evaluates its
  operand once and is true only for `nil` and `false`. `&&`, `||`, and `!`
  recursively preserve short-circuit Ruby condition semantics.
- Ruby `+` initially lowers to numeric `Add`. Type inference rewrites string
  operands to `StrConcat`.
- A bare return or empty function lowers to `void`. A final side-effect call may
  remain in return position and is normalized as a `void` result by shared
  lowering; write explicit `nil` when an absent value is intended.
- Ruby `==` and `!=` map to typed HIR equality and inequality.
- `&&` and `||` remain structured short-circuit operators.

## Deliberately unsupported in M1

Top-level executable statements, optional/keyword/rest parameters, receiver
method dispatch, dynamic method definition, post-test loops, multiple return
values, interpolation, arrays, hashes, classes, and general blocks produce
E02xx diagnostics with rewrite suggestions. The M3 adapter extension adds the
specified array, hash, class, `times`, and `each` subset.

Because HIR locals are lexical, a Ruby local first assigned inside a nested
conditional or loop remains local to that HIR block. Initialize it in the
surrounding function body before the control-flow construct when it is needed
afterward.
