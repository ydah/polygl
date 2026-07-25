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

The initial renderer batches `rect`, `circle`, and `triangle` as colored
triangles in one dynamic vertex buffer. `background` flushes pending geometry
before clearing. `fill` changes the color recorded for later vertices. Calling
`size` resizes the drawing buffer and viewport.

Random values come from a session-local seeded generator. Supplying the same
`seed` option to `start` produces the same sequence. Mouse and keyboard state is
normalized for `mouseX`, `mouseY`, `keyDown`, and the optional `on_event`
entrypoint.

`shaders.js` contains data-only GLSL and reflection metadata. At startup the
shader registry compiles and links every pair, resolves reflected uniform
locations, and reports driver logs at the original vertex or fragment source
location. It uploads `u_time`, `u_resolution`, and identity transform defaults
after `setup` and before each frame. User uniform values are type-checked when
set; debug builds additionally reject an unset reflected user uniform after
`setup`, while release builds retain WebGL's zero/default value until one is
set. Registry programs are deleted with the runtime session.

Generated debug checks call `checkedIndex`, `checkIndex`, and `requireNonNil`
with an embedded source location. An uncaught lifecycle error stops the loop and
renders a fixed browser overlay headed by `source:line:column`. Release builds
omit the compiler-inserted checks.
