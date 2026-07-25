# 0019: Use a reflected fixed shader ABI

- Status: Accepted
- Date: 2026-07-25

## Context

User shaders need to interoperate with meshes, transforms, materials, and
WebGL2 without making every source-language adapter model WebGL declarations.
String-based shader and uniform lookup would defer misspellings until drawing,
while unrestricted layouts would make meshes and runtime binding
language-specific.

Ruby does not gain class/struct syntax until M3, although M2 needs an executable
shader path and a Ruby conformance example.

## Decision

Shader pairs use matching `vertex_<name>` and `fragment_<name>` entries. The
compiler validates a fixed attribute layout, a struct-based varying interface,
stage result types, and literal material names. It emits reflection metadata
for attributes and uniforms, and the runtime binds only from that metadata.
Automatic uniforms have reserved canonical names; other free variables become
per-node user uniforms checked by `shader_set`.

The complete contract is `docs/shader-abi.md`. Until source adapters can
construct varying structs, a narrow zero-varying form permits a vertex `vec4`
result and a parameterless fragment `vec4` result. This is a compatibility
bridge, not a second general ABI.

## Consequences

Meshes and shader code share one deterministic interface, missing material
names fail during compilation, and runtime uniform validation has precise type
and source information. Backends can encode identifiers without changing the
source-facing ABI.

Custom vertex layouts require a future ABI revision. The temporary
zero-varying form cannot express textured or lit materials and can be removed
only after adapter-level struct conformance fixtures have migrated.
