# GLSL ES backend

`polygl-backend-glsl` emits shader pairs from the checked GPU module returned by
`polygl_lir::split`. Passing unsplit LIR is outside the backend contract.

`GlslBackend::generate` returns one `ShaderArtifact` per case-sensitive pair
name. Each artifact contains vertex and fragment GLSL ES 3.00 source, fixed
attribute bindings, uniform reflection, and the source span of both entries.
The current GPU LIR exposes the automatic `u_time` uniform; user-uniform
lowering is added with material binding in M2-4. Pair order is deterministic by
name.

## Emission rules

- Every source begins with `#version 300 es` and high-precision float/int
  declarations.
- User identifiers are byte-encoded behind compiler-reserved prefixes. Source
  names remain unchanged in reflection metadata.
- GPU structs are dependency-ordered, and functions receive prototypes before
  their definitions.
- GPU constants are expression macros. This permits uniform- and
  function-dependent values without generating illegal GLSL global
  initializers; GPU code is side-effect free, so expansion preserves observable
  behavior.
- Structured LIR `if`, `while`, range `for`, `break`, and `continue` remain
  structured GLSL. Range bounds are captured once, and inclusive loops avoid a
  final overflowing increment.
- Ruby/PHP-style floor integer division and remainder use generated helpers
  rather than GLSL's truncating signed integer operators. The helpers also
  preserve two's-complement wrapping for `INT_MIN / -1`. Because WebGL shaders
  cannot raise a catchable Host runtime error, split rejects an integer divisor
  unless constant propagation proves it nonzero (E0406); backend zero guards
  remain as defense in depth for malformed direct input. Float floor remainder
  uses GLSL `mod`.
- `time()` lowers to `u_time`; `floor`, `round`, and `trunc` lower to the
  corresponding GLSL operation followed by an integer conversion.
- A zero-varying vertex `vec4` result is assigned to `gl_Position`. A varying
  struct result assigns `clip_pos` to `gl_Position` and copies every field,
  including `clip_pos`, to generated `out` variables. The fragment result is
  assigned to `out_color`.

The backend still returns typed `EmitError` values for malformed direct input.
GPU subset and ABI diagnostics belong to `polygl_lir::split`, where source
spans and E0401–E0406 suggestions are available. Constant cycles are rejected
before expression macros are emitted.

Backend tests run the full Ruby → HIR → typed HIR → LIR → split → GLSL path.
When `glslangValidator` is installed, tests additionally compile the generated
vertex and fragment sources; environments without it retain deterministic
string-level coverage and later WebGL conformance coverage.
