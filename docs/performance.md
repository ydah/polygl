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

## Dated baseline

The 2026-08-12 development baseline used Apple arm64/macOS 26.3, Rust 1.96.1,
Node 24.14.1, a release compiler, seven fresh output directories per case, and
the median wall time of a new CLI process. These fixtures name workload shapes;
`large` is the current terrain example, not a claim about production-scale input.

| Compiler case | Median | Min–max |
| --- | ---: | ---: |
| small triangle | 11.06 ms | 6.62–37.02 ms |
| medium rotating cubes | 8.14 ms | 7.07–11.26 ms |
| nominal-large terrain | 9.10 ms | 7.08–11.11 ms |
| shader-heavy plasma | 6.67 ms | 6.13–10.53 ms |
| class-heavy conformance | 9.44 ms | 6.83–10.52 ms |
| error-heavy diagnostics | 8.70 ms | 5.17–9.18 ms |

Run `node benchmarks/compiler/run.mjs` after building `target/release/polygl`.
Scheduled CI uploads the raw JSON as a comparison record; it is not a
cross-machine threshold.

The same machine's headless Chromium 143 software-WebGL record measured one
instrumented sample per isolated session:

| Runtime case | Setup / frame | Observable work |
| --- | ---: | --- |
| 10,000 immediate shapes | 25.3 / 15.7 ms | 1 draw, 60,000 vertices, 20,000 triangles, 1.44 MB upload |
| 256 retained nodes | 9.4 / <0.1 ms | 256 draws, 3,072 triangles, one 1,296-byte mesh |
| 32 decoded textures | 3.9 / <0.1 ms | 32 textures, 73 state changes |
| one reflected automatic uniform | 2.3 / <0.1 ms | one program, 1.8 ms link, one upload |

Run `pnpm --dir runtime build` followed by
`node benchmarks/runtime/run.mjs`. Software timing below timer resolution is
reported as `<0.1 ms`; the counters, not a zero duration, prove the work ran.

Current raw release budgets are 190,000 bytes for `runtime.js`, 25,000 for
`app.js`, and 30,000 for `shaders.js`. At this baseline the largest representative
outputs were 172,524, 1,458, and 2,608 bytes. `scripts/check-size-budget.sh`
enforces them. Recalibrate only with a documented feature/measurement change;
do not raise a budget merely to make CI green.

Peak RSS is not published yet: macOS `/usr/bin/time`, GNU time, and hosted-runner
accounting are not directly comparable. Add it after one normalized collector
and a stable representative runner are selected.
