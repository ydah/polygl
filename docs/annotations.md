---
---

# Source annotation directives

Source annotations constrain Common Core inference without adding
language-specific types to HIR. They are intended for empty collections,
polymorphic parameters, class fields, and other declarations whose type cannot
be recovered from use.

Annotations are constraints, not casts. A value must still be assignable to the
declared type, and later assignments keep that type. Function return
annotations may also come from a source language's supported native type
hints.

## Ruby

Ruby uses a standalone comment with this exact form:

```ruby
# @pgl name: type
```

For a parameter, place one or more directives immediately before the
containing `def`. Each directive names its parameter:

```ruby
# @pgl scale: float
# @pgl values: float[]
def scaled_total(scale, values)
  total = 0.0
  values.each do |value|
    total = total + value * scale
  end
  total
end
```

For a local, place the directive immediately before its first assignment:

```ruby
def setup
  # @pgl points: float[]
  points = []
end
```

Constructor fields use the field name without `@`:

```ruby
class Dot
  def initialize
    # @pgl x: float
    @x = 0
  end
end
```

A constructor assignment such as `@x = x` also inherits a direct parameter
annotation for `x`, so a separate field directive is unnecessary.

Blank lines and adjacent `# @pgl` directives may appear between a directive
and its target. An ordinary comment or executable statement breaks adjacency.
Inline comments, `# @pgl...` text without a separating space, and directives
inside a different function are ordinary comments or unmatched directives;
they never silently constrain another declaration.

## PHP

PHP uses a DocBlock with this exact directive form:

```php
/** @pgl $name: type */
```

For parameters, place one or more DocBlocks immediately before the containing
function or method. A matching native PHP type hint consumes a redundant
directive; a conflict produces E0303 and suggests removing the directive:

```php
/** @pgl $values: float[] */
function scaled_total(float $scale, $values): float {
    // ...
}
```

For a local or constructor field, place the directive immediately before the
first assignment:

```php
function setup() {
    /** @pgl $points: float[] */
    $points = [];
}

class Dot {
    function __construct() {
        /** @pgl $x: float */
        $this->x = 0;
    }
}
```

Only whitespace may separate a PHP DocBlock from its target. The target name
always includes `$` in the directive, including constructor fields. A
constructor assignment from a typed parameter also transfers that parameter
type to the field.

## Type spellings

| Family | Spellings |
|---|---|
| Scalars | `int`, `float`, `bool`, `str` |
| Collections | `T[]`, `Map<str, T>`, `Option<T>` |
| Vectors | `vec2`, `vec3`, `vec4` |
| Matrices | `mat2`, `mat3`, `mat4` |
| Graphics handles | `Mesh`, `Node`, `Material`, `Texture` |
| User structs/classes | an identifier beginning with an uppercase letter |

Collection spellings may nest. Map keys are always `str`; other key types are
outside Common Core. Type names and generic constructors are case-sensitive.
`void` is not a value type and cannot annotate a parameter, local, or field.

Malformed syntax, an unknown type, a directive without a matching declaration,
or a non-adjacent directive produces E0314 with a placement or spelling
suggestion. A well-formed annotation that conflicts with inferred use produces
E0303.
