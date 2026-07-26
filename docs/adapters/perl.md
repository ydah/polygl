---
---

# Perl adapter

The Perl adapter parses UTF-8 Perl 5 source with `ts-parser-perl` 1.2.1 and
lowers an intentionally static subset to source-spanned HIR. It accepts one
source file, does not execute Perl, and does not load CPAN modules.

## Common Core mapping

| Common Core element | Perl syntax | HIR lowering |
|---|---|---|
| Function | `sub name { my ($args) = @_; ... }` | `Item::Function` |
| Setup entry | `sub setup { ... }` | `EntryPoint::Setup` |
| Frame entry | `sub frame { my ($dt) = @_; ... }` | `EntryPoint::Frame` |
| Event entry | `sub on_event { my ($event) = @_; ... }` | `EntryPoint::OnEvent` |
| Shader entry | `sub vertex_name` / `sub fragment_name` | named GPU entry |
| Constant | top-level `my $NAME = value;` | `Item::Const` |
| Local declaration/write | `my $name = value` / `$name = value` | `Let` / `Assign` |
| Conditional | `if` / `elsif` / `else` | nested structured `If` |
| Conditional loop | `while ($condition)` | structured `While` |
| Ascending range loop | `for my $i ($start .. $end)` | inclusive structured `For` |
| Loop control | `last` / `next` | `Break` / `Continue` |
| Array | `my @values = (...)`, `$values[$index]` | homogeneous `Array`, `Index` |
| String-keyed map | `my %values = ("key" => value)`, `$values{"key"}` | homogeneous `Map`, `Index` |
| Struct-like class | `package`, `new` + `bless`, `$self->{field}`, instance subs | `StructDef` plus static self-first functions |
| Function result | explicit `return` or final expression | `Return` in ordinary functions |
| Call | `name($args)` | builtin ID when registered, otherwise user function |

Function parameters use one leading `my ($first, $second) = @_;` declaration.
Optional/default arguments, prototypes, signatures, slurpy parameters, caller
context, aliases, and mutation through `@_` are outside Common Core. Place
`# @pgl $name: type` immediately before the containing subroutine for a
parameter or shader uniform, or immediately before a local declaration.

## Semantic differences

- Perl `/` lowers to `DivFloat` and `%` lowers to truncating `RemTrunc`.
- Conditions remain direct HIR expressions and must infer as `bool`; Perl
  truthiness is not reproduced. Use an explicit comparison or definedness test.
- Perl `.` lowers to `StrConcat`.
- Numeric `==` / `!=` and ordered comparisons lower to typed HIR comparisons.
  String-specific comparison operators are outside the initial subset.
- `&&`, `||`, and `!` preserve short-circuit boolean evaluation.
- Array and map declarations use their sigils to disambiguate the same Perl
  list syntax. Map entries must be explicit `key => value` pairs.
- A range loop bound is evaluated once by HIR. General C-style `for` loops and
  dynamically mutating bounds are rejected.

## Struct-like packages

A class package ends at the next `package` statement. It may define one `new`
subroutine and ordinary methods. `new` must destructure `$class` first, create
one hash reference with fixed fields, and return `bless $self, $class`.
Methods destructure `$self` first. Fields use `$self->{name}`.

Inheritance, roles, symbol-table mutation, dynamic method names, indirect
objects, `AUTOLOAD`, typeglobs, and package globals produce E0203 with a
composition or top-level-function suggestion.

## Capabilities and unsupported syntax

The adapter advertises `Core`, `Tier1`, `Tier2`, `Arrays`, `Maps`, `Classes`,
and `Shaders`. General references, regex side effects, `eval`, `do`/`require`,
formats, tied variables, magic variables other than the parameter-list `@_`,
postfix control flow, `map`/`grep`, anonymous subs, and top-level executable
statements produce source-spanned E02xx diagnostics with suggestions.

HIR locals are lexical. Initialize a value in the containing subroutine before
using it after an `if`, loop, or other nested block.
