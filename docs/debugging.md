---
title: Debugging generated programs
permalink: /debugging/
---

# Debugging generated programs

Start with a debug build and retain the full intermediate output:

```console
polygl check sketch.rb --diagnostic-format json
polygl emit sketch.rb --emit hir,lir,js,glsl,manifest --source-map inline > generated.txt
polygl build sketch.rb -o dist --source-map external --sources-content --profile
```

Do not publish a `sourcesContent` build unless revealing the complete source is
intentional.

## Read the generated JavaScript

Search `app.js` for exported `setup`, `frame`, and `on_event` functions, then
for `__pglRuntime` calls. Generated bindings are encoded to avoid reserved-word
and Unicode collisions, so use the HIR/LIR dump to identify the source symbol
instead of inferring names. Debug-only `checkedIndex`, `mapGet`, and
`requireNonNil` calls carry a source object; release builds intentionally omit
compiler-inserted array/absence checks.

Load `app.js.map` in browser DevTools to map a generated line and UTF-16 column
back to the normalized source path. If a mapping looks wrong, compare it with:

```console
node --enable-source-maps dist/app.js
```

The conformance suite uses Node's standard `SourceMap` consumer for Unicode
columns; report both generated and original positions in a bug.

## Inspect shaders

`shaders.js` is data, not executable discovery code. For each shader it contains
vertex/fragment GLSL, attributes, uniforms, source locations, and the shader ABI.
Compare declared names/types/locations with `gl.getActiveAttrib`,
`gl.getActiveUniform`, and the runtime capability report. Startup rejects
unexpected reflection, duplicate names, invalid locations, sampler overflow,
and ABI mismatch before drawing.

Driver logs currently map to the vertex or fragment definition, not an exact
source expression. Exact GLSL-line mapping requires a future backend-emitted
line table; do not guess it in the runtime.

## Runtime failures

The accessible browser overlay and `formatRuntimeError` show the source location
embedded by debug checks or shader metadata. Capture:

- the first error and its cause, not only later WebGL errors;
- `handle.state`, `handle.stats()`, and `handle.capabilities()`;
- browser/OS/renderer metadata;
- the generated manifest and relevant shader record; and
- expected, actual, and diff framebuffer images for rendering regressions.

`externalWebglPolicy: "exclusive"` assumes PolyGL owns context state. After
outside GL calls, invalidate the session cache or reproduce with the `reset`
policy. For context loss, create a new session: restoration cannot reuse old
buffers, textures, programs, or handles.
