---
---

# 0021: Package shaders as reflected data

- Status: Accepted
- Date: 2026-07-25

## Context

The host JavaScript source map must continue to map generated statements to the
source language, while GLSL pairs need independent source, binding metadata,
build mode, and entry spans. Concatenating GLSL into `app.js` would complicate
the source map and let application code participate in shader registration.
Compiling pairs lazily would defer driver failures until a material is first
drawn.

## Decision

Emit a data-only `shaders.js` module beside `app.js`. The generated HTML loads
the frozen shader bundle first and passes it as a runtime option. A session-owned
WebGL2 registry eagerly compiles and links every pair before evaluating
`app.js`, resolves reflected uniforms, uploads automatic values every frame, and
owns program disposal. Driver failures use the shader entry span carried in the
bundle.

Keep user-uniform validation in the registry. Values are type-checked in every
build; debug builds also require every reflected user uniform to be populated
after `setup`. Material and node APIs may later route their per-instance values
through the same registry without changing the artifact format.

## Consequences

`app.js.map` remains independent of shader packaging, and invalid GLSL fails
deterministically at startup with a source-language location. Builds gain one
small JavaScript artifact and eagerly pay compilation cost for all declared
pairs. Per-material program selection and drawing remain separate scene-graph
responsibilities.
