# 0027: Use session-owned Tier 2 handles

- Status: Accepted
- Date: 2026-07-25

## Context

Tier 2 adds retained meshes, nodes, materials, textures, a camera, and a light
to the immediate Tier 1 renderer. The public API must remain identical across
source languages, while WebGL resources cannot safely outlive the runtime
session that created them. The fixed shader ABI also requires node transforms
and user uniforms to be applied at draw time rather than stored once per shader
pair.

Asset calls must synchronously return an opaque `Texture` value even though the
browser completes image loading asynchronously. Builds also need a portable
rule for locating and copying those files.

## Decision

Mesh, Node, Material, and Texture values are branded, session-owned handles.
Every operation checks that its handle belongs to the active session. Stopping
the session releases all buffers, vertex arrays, textures, and programs.

Tier 2 uses a right-handed coordinate system. Node rotation is an XYZ Euler
vector in radians, and the model transform is translation times Z, Y, and X
rotation times scale. Perspective field of view is vertical and expressed in
radians. The active camera stores perspective and look-at settings; the active
directional light stores a world-space direction and RGB intensity.

Meshes use the shader ABI's fixed 12-float interleaved vertex layout:
`position: vec3`, `normal: vec3`, `uv: vec2`, and `color: vec4`.
`mesh_from(float[], int[])` exposes that layout directly. Generated box,
sphere, and plane meshes produce the same representation.

Basic materials use the runtime's Blinn-Phong program. Shader materials share
a linked program, but each node owns its user-uniform map. `shader_set` accepts
only reflected shader value types and validates missing values when that node
is drawn. Automatic model, view, and projection matrices are uploaded for each
node.

`texture_load` accepts a literal, relative, slash-separated file path. The CLI
copies each referenced file from the source directory to the same relative path
in the build. At runtime, a path cache returns a handle backed immediately by a
1x1 white texture. Loads requested while module initialization or `setup` runs
participate in a startup barrier; the first frame waits for that barrier.
Loads first requested by `frame` keep using the white texture until the image
upload completes.

## Consequences

Source-language integers cannot be passed where handles are expected, resources
cannot leak between sessions, and a linked custom shader can render many nodes
with independent state. Mesh reflection and primitive generation agree on one
layout, at the cost of a relatively wide vertex format for simple geometry.

Builds are self-contained and deterministic about asset locations. Dynamic
asset paths are rejected because they cannot be copied ahead of time. Texture
handles stay stable while their WebGL contents change, which implements the
placeholder policy without changing generated host code.
