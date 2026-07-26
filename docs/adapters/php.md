# PHP adapter

The PHP adapter parses UTF-8 PHP source with Mago 1.43.0 and lowers accepted
syntax directly to source-spanned HIR. It accepts one source file, requires a
PHP opening tag, and neither executes PHP nor loads Composer packages.

## Common Core mapping

| Common Core element | PHP syntax | HIR lowering |
|---|---|---|
| Function | `function name($args) { ... }` | `Item::Function` |
| Setup entry | `function setup() { ... }` | `EntryPoint::Setup` |
| Frame entry | `function frame($dt) { ... }` | `EntryPoint::Frame` |
| Event entry | `function on_event($event) { ... }` | `EntryPoint::OnEvent` |
| Shader entry | `function vertex_name()` / `function fragment_name()` | named GPU entry |
| Constant | `const NAME = value;` | `Item::Const` |
| Local declaration/write | `$name = $value` | first write is `Let`, later writes are `Assign` |
| Conditional | `if` / `elseif` / `else` | nested structured `If` |
| Conditional loop | `while ($condition)` | structured `While` |
| Ascending range loop | `for ($i = $start; $i < $end; $i++)` | exclusive or inclusive `For` |
| Array value loop | `foreach ($values as $value)` | evaluate-once indexed `For` |
| Loop control | `break` / `continue` | `Break` / `Continue` |
| Array | `[$a, $b]`, `$values[$index]` | homogeneous `Array`, `Index` |
| String-keyed map | `["key" => $value]` | homogeneous `Map`, `Index` |
| Struct-like class | `class`, `__construct`, `$this->field`, instance methods | `StructDef` plus static self-first functions |
| Call | `name($args)` | builtin ID when registered, otherwise user function |

Required positional value parameters are supported. PHP scalar hints `int`,
`float`, `bool`, and `string`, plus directly named class hints, become HIR type
constraints. A return hint may also be `void`. Attributes, references,
defaults, variadics, named arguments, argument unpacking, namespaces, and
imports are outside Common Core.

Place `/** @pgl $name: type */` immediately before a function or method to
annotate one of its parameters, or immediately before a local's first
assignment. Constructor field annotations use the field name with `$`, for
example `/** @pgl $x: float */` before `$this->x = 0;`. Invalid, unused, or
non-adjacent directives produce E0314. See
[Source annotation directives](../annotations.md).

## Semantic differences

- PHP `/` always lowers to `DivFloat`, including integer operands.
- PHP `%` lowers to truncating `RemTrunc`.
- Conditions remain direct HIR expressions and therefore must infer as
  `bool`. PHP truthiness is not reproduced; a non-boolean condition produces
  E0301 with an explicit-comparison suggestion.
- PHP `.` lowers to `StrConcat`.
- `===` and `!==` lower to typed equality and inequality. `==`, `!=`, and
  `<>` produce E0302 and suggest the corresponding strict operator.
- `is_null($value)`, `$value === null`, and `$value !== null` lower to
  `NilCheck` (with boolean negation for the final form).
- `&&` and `||` lower to structured short-circuit boolean operators. The
  lower-precedence `and`, `or`, and `xor` forms are rejected.
- Positional PHP arrays become Common Core arrays. Arrays whose every element
  has an explicit key become maps; the type checker requires string keys.
  Mixed keyed and positional arrays are rejected.
- Because PHP spells both empty arrays and empty maps as `[]`, an empty map
  requires an annotated local or constructor field before it is returned or
  otherwise used. A bare `[]`, including a constant value, defaults to an
  array.
- `count($array)` lowers to the shared array-length operation.

The accepted `for` shape has exactly one initialization, condition, and
increment. Its variable must be fresh, the comparison must be `<` or `<=`, and
the increment must be `$i++` or `++$i`. The end is limited to a literal or
top-level constant because HIR evaluates a range bound once, whereas PHP can
reevaluate an arbitrary expression every iteration. A value-only `foreach`
evaluates its array expression once and lowers to an index-ascending loop.
Key/value targets are not portable to the array-only loop form and are
rejected with a rewrite suggestion.

## Struct-like classes

A class may define one unmodified concrete `__construct` method and unmodified
concrete instance methods. The constructor may only assign each fixed field
once using `$this->field = value`. Construction with `new ClassName(...)`
calls a generated `ClassName::new` function, and each method receives a typed
`self` first parameter.

Inheritance, interfaces, traits, attributes, visibility or static modifiers,
declared properties, class constants, promoted properties, abstract methods,
null-safe access, static access, and dynamic property or method names produce
E0203 with a composition or top-level-function suggestion.

## Capabilities and unsupported syntax

The adapter advertises `Core`, `Tier1`, `Arrays`, `Maps`, `Classes`, and
`Shaders`. General closures, generators, `match`, `switch`, exceptions,
post-test loops, variable variables, interpolated strings, legacy `array(...)`
syntax, array unpacking, reflection, and top-level executable statements
produce source-spanned E02xx diagnostics with rewrite suggestions.

HIR locals are lexical even though PHP blocks do not introduce local-variable
scope. A variable first assigned inside `if`, `while`, `for`, or `foreach`
therefore remains inside that HIR block. Initialize it in the containing
function before the control-flow statement when it is needed afterward.
