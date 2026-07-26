---
title: Performance model
description: Understand where PolyGL compilation and browser runtime performance come from.
---

# Performance model

Rust makes PolyGL's parsing, type inference, specialization, and code generation
fast and predictable. That benefit is **conversion speed**: how quickly a source
file becomes browser artifacts. PolyGL does not run user sketches as native Rust
and does not currently emit a WebAssembly host.

Runtime performance is determined by two other layers:

1. the generated ES2020 JavaScript that runs application and frame logic; and
2. the browser runtime's WebGL 2 batching, shader programs, resource uploads,
   and GPU workload.

The source language does not remain in the browser. Equivalent Ruby, PHP, and
Perl programs lower through the same HIR/LIR and use the same JavaScript, GLSL,
and batching backends. Source-language choice is therefore not a useful runtime
benchmark variable; generated work and draw shape are.

## Debug and release builds

Debug output includes source-located collection-boundary, vector/matrix, and
absence checks. Release output removes those compiler-inserted checks:

```console
polygl build sketch.rb -o dist-debug
polygl build sketch.rb -o dist-release --release
```

Use debug output while authoring. Measure release output when evaluating frame
time, but do not treat `--release` as a substitute for profiling. It does not
change scene complexity, shader cost, texture bandwidth, or browser behavior.

## Measure the right stage

- Measure `polygl build` to investigate compiler conversion time.
- Use the browser Performance panel for JavaScript frame work and event cost.
- Use WebGL/GPU tooling for shader, overdraw, mesh, and texture bottlenecks.
- Watch batch flushes when alternating drawing state; compatible Tier 1 shapes
  share batches, while state changes may split them.
- Load fixed assets during module initialization or `setup` when the first
  frame must wait for them. Loads first requested from `frame` deliberately use
  a placeholder until ready.

See the [runtime contract]({{ '/runtime/' | relative_url }}),
[JavaScript backend]({{ '/js-backend/' | relative_url }}), and
[shader ABI]({{ '/shader-abi/' | relative_url }}) for the boundaries behind
this model.
