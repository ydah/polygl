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
| Counted block | `n.times { |i| ... }` | exclusive range `For` from zero |
| Collection block | range/array `.each { |item| ... }` | range or index-ascending `For` |
| Loop control | `break` / `next` | `Break` / `Continue` |
| Array | `[a, b]`, `values[index]` | homogeneous `Array`, `Index` |
| String-keyed map | `{"key" => value}` or `{key: value}` | homogeneous `Map`, `Index` |
| Function result | explicit `return` or final expression | `Return` in ordinary functions |
| Call | `name(args)` | builtin ID when registered, otherwise user function |

The adapter supports required positional parameters, integer and float literals, UTF-8
strings, booleans, `nil`, local variables, arithmetic, comparison, boolean
operators, parentheses, plain calls, homogeneous arrays and maps, and indexed
reads and writes. Array splats and hash splats remain unsupported.

Place `# @pgl name: type` immediately before the containing `def` for a
parameter, or immediately before a local variable's first assignment, when
inference needs help. Scalar, array, `Map<str, T>`, `Option<T>`, vector, matrix,
opaque graphics handle, and named struct types are accepted. Invalid, unused,
or non-adjacent directives produce E0314.

## Semantic expansion

- Ruby `/` initially lowers to `DivInt`. Type inference may rewrite it to
  `DivFloat` when either operand is inferred as float.
- Ruby `%` lowers to `RemFloor`, preserving Ruby's divisor-signed result for
  negative operands instead of inheriting JavaScript remainder semantics.
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
- `times` and `each` blocks are syntax only: they lower to structured loops and
  never become closure values. Array receivers are evaluated once before an
  index-ascending loop.

## Deliberately unsupported

Top-level executable statements, optional/keyword/rest parameters, receiver
method dispatch, dynamic method definition, post-test loops, multiple return
values, interpolation, collection splats, and general blocks produce E02xx
diagnostics with rewrite suggestions. Blocks other than direct `times` and
`each` statements, including stored or returned blocks, produce E0202.

Because HIR locals are lexical, a Ruby local first assigned inside a nested
conditional or loop remains local to that HIR block. Initialize it in the
surrounding function body before the control-flow construct when it is needed
afterward.
