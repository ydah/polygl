---
---

# Browser runtime

Generated modules export a numeric runtime ABI marker, and generated shader
bundles carry their own shader ABI marker. The embedded runtime checks both
before setup, shader compilation, or rendering and rejects missing or
mismatched markers with both versions in the error. Direct object programs may
omit the marker for runtime-library use; dynamically loaded compiler output may
not. The runtime package version and ABI constant are generated from the
workspace compatibility source rather than maintained by hand.

`@polygl/runtime` executes a generated ES2020 module in a WebGL2 canvas. The
generated `index.html` loads the data-only `shaders.js` first, passes its bundle
to `start`, and then loads `app.js`. The runtime compiles the bundle before the
application module is evaluated, so module-level handle creation can resolve
shader names. The loader form also makes the runtime active before module-level
constants are evaluated. `start` invokes `setup` once, waits for it to finish,
and then calls `frame(dt)` from `requestAnimationFrame`. The first `dt` is zero
and later values are elapsed seconds, capped at 0.1 seconds by default so a
backgrounded tab cannot advance a simulation by an unbounded step. Callers may
set a different positive `maxDeltaSeconds`. A second start is rejected while
loading or setup is still in progress.

The Tier 1 renderer batches `rect`, `circle`, `triangle`, and screen-space
one-pixel `line` geometry as colored triangles in one dynamic vertex buffer.
`stroke` enables outlines for later shapes, while `no_stroke` disables them; a
standalone line falls back to the fill color when no stroke is active.
`background` flushes pending geometry before clearing, and `fill` changes the
color recorded for later vertices.

`push_matrix` and `pop_matrix` preserve a checked affine transform stack.
`translate`, `rotate`, and `scale` post-multiply the current transform before
vertices enter the batch. Calling `pop_matrix` without a matching push is a
runtime error. Text is rendered with the current fill and transform on a
pointer-transparent Canvas2D overlay. `background` clears both layers, `size`
keeps them aligned, and session disposal removes the overlay.

The Tier 2 renderer retains session-owned mesh, material, node, and texture
handles. Boxes, spheres, planes, and custom meshes all upload the shader ABI's
12-float interleaved vertex layout. Nodes hold independent position, XYZ Euler
rotation in radians, scale, and user-uniform maps. The active perspective
camera and directional light feed either the built-in Blinn-Phong material or
a reflected custom shader. Each frame clears depth, draws retained nodes, then
restores the Tier 1 attribute and depth state so immediate-mode geometry can be
used as an overlay. Passing a handle to another runtime session is an error;
stopping its owner deletes mesh buffers, textures, and linked programs.

Random values come from a session-local seeded generator. Supplying the same
`seed` option to `start` produces the same sequence. Pointer coordinates are
normalized into drawing-buffer coordinates for `mouseX` and `mouseY`; keyboard
state is exposed through `keyDown`. Pointer move/down/up and key down/up events
invoke the optional `on_event` entrypoint with the built-in
`Event { kind, x, y, key }` value. Event-driven redraws are coalesced into one
animation frame; programs that define `frame` redraw on their existing loop.

`autoResize: true` observes the canvas's CSS box and sizes its drawing buffer
using `devicePixelRatio`; it is opt-in so an explicit `size` call remains the
default contract. Tests and non-browser hosts can inject `devicePixelRatio`
and `createResizeObserver`. A WebGL context loss cancels pending frames and
reports that rendering is suspended. Restoration stops the session with a
restart-required error because WebGL invalidates every buffer, texture, and
program and silently continuing with stale handles would be incorrect.

`shaders.js` contains data-only GLSL and reflection metadata. At startup the
shader registry compiles and links every pair, resolves reflected uniform
locations, and reports driver logs at the original vertex or fragment source
location. Driver-optimized inactive uniforms have no location and are skipped.
It uploads `u_time`, `u_resolution`, and identity transform defaults after
`setup` and before each frame for the legacy global binding surface. A retained
node instead receives its actual model, view, and projection matrices at draw
time. `shader_set` type-checks and copies values into that node; debug builds
reject an unset active user uniform when the node is first drawn, while release
builds retain WebGL's zero/default value until one is set. Registry programs
are deleted with the runtime session.

`material_shader("<name>")` returns an immutable handle backed by the eager
registry. Split requires a literal name and resolves it against complete shader
pairs, so the runtime lookup is a defensive invariant check rather than normal
string-based discovery.

`texture_load` immediately creates a handle containing a 1x1 white WebGL
texture. Relative URLs are cached per session. Requests made while the
application module or `setup` is running join a startup barrier, so initial
drawing and the first `frame` wait for image decoding and upload. Requests
first made by `frame` keep the white texture and do not pause the loop; the same
handle begins sampling the decoded image after its asynchronous upload. Setup
load failures reject `start`, while later failures stop the running session and
use the normal error overlay.

Scene handles are frozen opaque facades; GPU objects and mutable node state are
kept in session-private weak maps. `node_remove` releases a node's mesh and
texture references. `mesh_dispose` and `texture_dispose` delete individual GPU
resources, reject resources that are still referenced, and permanently
invalidate their handles. Disposing a pending texture also prevents a late
image decode from uploading into a deleted WebGL texture.

GPU split warnings such as W0401 and W0402 are rendered by both `check` and
`build`; successful compilation no longer discards non-fatal diagnostics.

Generated debug checks call `checkedIndex`, `checkIndex`, and `requireNonNil`
with an embedded source location. An uncaught lifecycle error stops the loop and
renders a fixed browser overlay headed by `source:line:column`. Release builds
omit the compiler-inserted checks.
