# Browser runtime

`@polygl/runtime` executes a generated ES2020 module in a WebGL2 canvas. Start a
generated module with `start(() => import("./app.js"))`: the loader form makes
the runtime active before module-level constants are evaluated. `start` invokes
`setup` once, waits for it to finish, and then calls `frame(dt)` from
`requestAnimationFrame`. The first `dt` is zero and later values are elapsed
seconds. A second start is rejected while loading or setup is still in progress.

The initial renderer batches `rect`, `circle`, and `triangle` as colored
triangles in one dynamic vertex buffer. `background` flushes pending geometry
before clearing. `fill` changes the color recorded for later vertices. Calling
`size` resizes the drawing buffer and viewport.

Random values come from a session-local seeded generator. Supplying the same
`seed` option to `start` produces the same sequence. Mouse and keyboard state is
normalized for `mouseX`, `mouseY`, `keyDown`, and the optional `on_event`
entrypoint.

Generated debug checks call `checkedIndex`, `checkIndex`, and `requireNonNil`
with an embedded source location. An uncaught lifecycle error stops the loop and
renders a fixed browser overlay headed by `source:line:column`. Release builds
omit the compiler-inserted checks.
