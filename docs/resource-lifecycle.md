---
title: Runtime resource lifecycle
permalink: /resource-lifecycle/
---

# Runtime resource lifecycle

A runtime session owns one canvas/context, frame scheduler, listeners, text
overlay, shader registry, resource registries, and pending image work. Opaque
handles are branded to that session; forged, stale, disposed, or cross-session
handles fail before WebGL use.

## Nodes, meshes, materials, and textures

- A node holds references to its mesh and any texture-valued material uniforms.
- `node_remove` detaches the node and decrements those references.
- `mesh_dispose` and `texture_dispose` succeed only when no node/material still
  refers to the resource. A disposed handle never becomes valid again.
- Built-in materials are immutable session values. Custom shader material state
  is node-local; user uniform values are copied and dirty-tracked.
- Equal texture URLs share one session load/handle. Every image loader receives
  an `AbortSignal`; stop or disposal prevents a late decode from uploading.

Creation is checked against configured and live GL limits before allocation.
The capability report and cumulative statistics expose counts, bytes, program
work, uploads, batches, and state changes.

## Shaders and WebGL state

The session owns linked programs and deletes them at stop. Artifact metadata is
validated before linking and active reflection afterward. Programs are not
shared across sessions or contexts.

PolyGL has exclusive state ownership by default. If outside code uses the same
context, call `invalidateWebGLState` afterward or select per-frame
reset. The cache covers program, buffers, VAO, blend/depth, viewport, active
texture, binding, and blend equation; it does not promise to preserve arbitrary
caller state.

## Failure and context loss

Startup, setup, frame, listener, image, and shader failures converge on session
cleanup. Stop is idempotent. Context loss cancels frames and suspends rendering;
restoration terminates the session because all prior GPU resources are invalid.
Create a new session to reconstruct them. Application references to old handles
must not be reused.
