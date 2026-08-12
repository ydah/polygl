---
title: One program through the architecture
permalink: /architecture-tutorial/
---

# One program through the architecture

Use `examples/triangle.rb`, `.php`, and `.pl` to compare the same program. Each
defines `setup`, selects a canvas size and background, then emits one immediate
triangle. Source syntax differs; the semantics after the adapter do not.

## 1. Source and adapter

The CLI selects an adapter by explicit language or file extension and creates a
UTF-8 `SourceFile`. The adapter parser owns source-language grammar, comments,
and recovery. It either returns structured, source-spanned diagnostics or HIR.
It must expand language behavior—such as Ruby truthiness or remainder choice—
into explicit Common Core operations here.

## 2. HIR and typed HIR

HIR preserves structured functions, entries, statements, expressions, and byte
spans without preserving parser nodes. `StageValidator` checks source/node/depth
budgets. Type analysis resolves every expression, method target, field, builtin,
and specialization and returns the checked `TypedHir` wrapper. Backends cannot
receive this untyped adapter result by accident through `Compiler::compile`.

Inspect these stages with:

```console
cargo run -p polygl-cli -- emit examples/triangle.rb --emit hir,lir
```

The dump is deterministic debugging text, not a stable interchange format.

## 3. Domain-resolved LIR and split

Lowering replaces builtin IDs with runtime operations, fixes concrete types,
folds only semantics-safe expressions, and records effects. Dependency SCCs
propagate Host/GPU domains. Split then produces:

- a Host module containing `setup`, frame/event work, browser runtime calls, and
  only reachable asset references; and
- a GPU module containing validated vertex/fragment pairs and shared helpers.

A Host-only operation reached from a shader is an error with the shortest
dependency path. Effectful initializers survive Host reachability pruning.

## 4. JavaScript and GLSL

The JavaScript backend emits ES2020 plus a runtime ABI marker. Debug builds add
located bounds/absence checks and release builds omit those checks. Source Maps
map generated UTF-16 positions to normalized source names according to the
explicit privacy policy.

The GLSL backend emits ES 3.00 pairs plus reflection metadata: safe generated
names, attributes, uniforms, types, locations, sources, and shader ABI. The
triangle uses the built-in Tier 1 program, but custom shader examples take the
same reflected path.

## 5. Packaging and runtime

The CLI assembles a complete adjacent staging generation containing HTML,
embedded runtime, JavaScript, shader data, manifest, optional map, and assets.
It validates portable path collisions before activation. The manifest records
source/artifact hashes, options, compiler version, adapter/API, schema versions,
and both ABIs without a timestamp.

In the browser, `start` validates the module and shader bundle before setup,
links programs, and compares live WebGL reflection with metadata. Tier 1 geometry
enters a reusable batched buffer. Failure stops the session and cleans listeners,
frames, pending image work, and owned GL resources. Debug runtime errors retain
the original source location independently of browser DevTools.

`Compiler::standard().compile(...)` is the same orchestration facade used by the
CLI and conformance tools. Alternate consumers therefore do not need to recreate
stage ordering, budgets, validation, or adapter selection.
