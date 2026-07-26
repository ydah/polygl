# Browser runtime

`@polygl/runtime` executes a generated ES2020 module in a WebGL2 canvas. The
generated `index.html` loads the data-only `shaders.js` first, passes its bundle
to `start`, and then loads `app.js`. The runtime compiles the bundle before the
application module is evaluated, so module-level handle creation can resolve
shader names. The loader form also makes the runtime active before module-level
constants are evaluated. `start` invokes `setup` once, waits for it to finish,
and then calls `frame(dt)` from `requestAnimationFrame`. The first `dt` is zero
and later values are elapsed seconds. A second start is rejected while loading
or setup is still in progress.

The Tier 1 renderer batches `rect`, `circle`, `triangle`, and one-pixel `line`
geometry as colored triangles in one dynamic vertex buffer. `stroke` enables
outlines for later shapes, while `no_stroke` disables them; a standalone line
falls back to the fill color when no stroke is active. `background` flushes
pending geometry before clearing, and `fill` changes the color recorded for
later vertices.

`push_matrix` and `pop_matrix` preserve a checked affine transform stack.
`translate`, `rotate`, and `scale` post-multiply the current transform before
vertices enter the batch. Calling `pop_matrix` without a matching push is a
runtime error. Text is rendered with the current fill and transform on a
pointer-transparent Canvas2D overlay. `background` clears both layers, `size`
keeps them aligned, and session disposal removes the overlay.

Random values come from a session-local seeded generator. Supplying the same
`seed` option to `start` produces the same sequence. Pointer coordinates are
normalized into drawing-buffer coordinates for `mouseX` and `mouseY`; keyboard
state is exposed through `keyDown`. Pointer move/down/up and key down/up events
invoke the optional `on_event` entrypoint with the built-in
`Event { kind, x, y, key }` value.

`shaders.js` contains data-only GLSL and reflection metadata. At startup the
shader registry compiles and links every pair, resolves reflected uniform
locations, and reports driver logs at the original vertex or fragment source
location. Driver-optimized inactive uniforms have no location and are skipped.
It uploads `u_time`, `u_resolution`, and identity transform defaults after
`setup` and before each frame. User uniform values are type-checked and copied
when set; debug builds additionally reject an unset active user uniform after
`setup`, while release builds retain WebGL's zero/default value until one is
set. Registry programs are deleted with the runtime session.

`material_shader("<name>")` returns an immutable handle backed by the eager
registry. Split requires a literal name and resolves it against complete shader
pairs, so the runtime lookup is a defensive invariant check rather than normal
string-based discovery.

GPU split warnings such as W0401 and W0402 are rendered by both `check` and
`build`; successful compilation no longer discards non-fatal diagnostics.

Generated debug checks call `checkedIndex`, `checkIndex`, and `requireNonNil`
with an embedded source location. An uncaught lifecycle error stops the loop and
renders a fixed browser overlay headed by `source:line:column`. Release builds
omit the compiler-inserted checks.
