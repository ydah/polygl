---
---

# 0022: Combine batched WebGL with a text overlay

- Status: Accepted
- Date: 2026-07-25

## Context

Tier 1 needs filled and stroked primitives, affine transforms, and text while
retaining the single dynamic WebGL2 vertex batch established by the first
renderer. Shipping a font atlas and glyph layout engine would make text part of
the WebGL batch, but would substantially expand the runtime before typography
is a performance target. Drawing text through the WebGL canvas's 2D context is
not possible after that canvas has acquired a WebGL2 context.

## Decision

Keep primitive fills and screen-space one-pixel strokes in the WebGL2 triangle
batch. Apply a session-local affine matrix to filled vertices as they enter the
batch, with an explicit push/pop stack and a checked underflow error. Transform
stroke endpoints first and construct their one-pixel quad in screen space.

Place the WebGL canvas and a same-sized, pointer-transparent Canvas2D canvas in
a runtime-owned grid wrapper for text. Text uses the current fill color and
affine transform. Resizing keeps both drawing buffers aligned, `background`
clears both layers, and stopping the session removes the owned wrapper while
restoring the WebGL canvas to its original parent.

## Consequences

Shapes retain one draw call per flush and transformations do not add shader
state. Text has predictable browser layout and no font assets, at the cost of a
second canvas and the fact that text is always composited above WebGL geometry.
A future WebGL text renderer can replace the overlay without changing the
language API.
