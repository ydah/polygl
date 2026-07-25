# Shader ABI

PolyGL shader entry points use a fixed WebGL2 / GLSL ES 3.00 interface. The
compiler owns every generated GLSL name; source-language adapters only produce
the canonical names and types described here.

## Shader pairs

`vertex_<name>` and `fragment_<name>` form one shader pair named `<name>`.
Names are matched case-sensitively after adapter lowering. A pair is valid only
when it contains exactly one vertex entry and exactly one fragment entry.

Host code selects a pair with `material_shader("<name>")`. The compiler resolves
the literal name before code generation. A missing pair, duplicate entry, or a
non-literal name is E0405; the runtime never searches for shader source by
string.

## Standard vertex layout

The runtime exposes four optional attributes. A vertex entry requests an
attribute by using its canonical parameter name and exact type:

| Parameter | GLSL input | Location | Type |
|---|---|---:|---|
| `position` | `a_position` | 0 | `vec3` |
| `normal` | `a_normal` | 1 | `vec3` |
| `uv` | `a_uv` | 2 | `vec2` |
| `color` | `a_color` | 3 | `vec4` |

Parameter order does not affect locations. Unknown names, repeated attributes,
and type mismatches are E0405. A mesh may omit data for an attribute that the
selected vertex entry does not request.

## Varyings and stage results

A vertex entry returns a user-defined struct. Its first field must be
`clip_pos: vec4`; the compiler assigns that field to `gl_Position`. Every
remaining field becomes a smooth varying named `v_<field>`.

The matching fragment entry takes exactly one parameter whose type is the same
struct. It returns `vec4`, which the compiler writes to `out_color`. Struct
field order and types are part of the ABI. `clip_pos` is not exposed as a
fragment input. A missing `clip_pos`, a mismatched struct, or an invalid stage
result is E0405.

The first implementation also accepts a vertex entry that returns `vec4` and a
fragment entry with no parameters. This zero-varying form exists for generated
fullscreen examples while Ruby class/struct syntax is delivered in M3. The
returned vertex value is assigned directly to `gl_Position`; the fragment
result remains `vec4`.

## Uniforms

The following uniforms are reserved and supplied by the runtime:

| Source surface | GLSL uniform | Type | Value |
|---|---|---|---|
| `time()` | `u_time` | `float` | elapsed seconds |
| `u_resolution` | `u_resolution` | `vec2` | drawing-buffer width and height |
| `u_model` | `u_model` | `mat4` | node model transform |
| `u_view` | `u_view` | `mat4` | active camera view transform |
| `u_proj` | `u_proj` | `mat4` | active camera projection transform |

Reserved uniforms are emitted only when referenced, except transform uniforms
required by a generated standard vertex path. User code cannot redeclare a
reserved name.

Other shader free variables are user uniforms. Their source name is preserved
in reflection metadata and encoded to a collision-free GLSL identifier. Valid
uniform types are `int`, `float`, `bool`, `vec2` through `vec4`, `mat2` through
`mat4`, and `Texture`. A `Texture` is emitted as `sampler2D` and read through
`sample(texture, uv)`.

`shader_set(node, "<name>", value)` stores a value on one node/material
instance. The runtime validates the reflected name and type. In debug builds,
all referenced user uniforms must be set before the first draw and after a
material replacement; otherwise startup/draw fails with the originating source
location. Release builds omit the presence check but still reject a JavaScript
value that cannot be uploaded as the reflected WebGL uniform type.

## GPU language subset

GPU code permits:

- `int`, `float`, `bool`, `vec2` through `vec4`, and `mat2` through `mat4`;
- scalar/vector/matrix arithmetic and comparisons accepted by the type checker;
- structured `if`, `while`, and range `for`, including dynamic loop
  conditions as allowed by GLSL ES 3.00;
- calls to GPU or shared functions and GPU-compatible builtins.

GPU code rejects:

| Code | Condition |
|---|---|
| E0401 | direct or indirect recursion |
| E0402 | `str`, `Option`, or another value with no GPU representation |
| E0403 | dynamic arrays or maps |
| E0404 | a Host-only builtin, function, or constant reached from GPU code |
| E0405 | invalid shader pair, stage ABI, material name, uniform, or reserved name |

Functions reached by both Host and GPU entries are emitted once per target.
Because Host `float` is f64 and GPU `float` is f32, each such shared function
produces W0401 at its declaration. A loop whose compiler-visible constant trip
count exceeds 1024 produces W0402; dynamic loops remain legal.

## Generated artifacts

Each pair produces one vertex source and one fragment source beginning with
`#version 300 es` and `precision highp float;`. Build metadata records the pair
name, sources, attribute locations, uniform names/types, and source spans. The
JavaScript module registers this metadata with the runtime; application code
does not concatenate or evaluate GLSL.

Shader compiler and linker failures are runtime errors because WebGL drivers
remain the final validator. The runtime includes the pair name, stage, driver
log, and original shader-entry source location in the error overlay.

