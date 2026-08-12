# Roadmap and non-goals

PolyGL's priority is a small, explicit Common Core that produces reproducible
browser artifacts from Ruby, PHP, and Perl. Roadmap entries describe direction,
not a promise for a particular release.

## Current direction

- Stabilize the public compiler facade, diagnostic protocol, artifact manifest,
  adapter API, and runtime/shader ABI independently.
- Grow behavior-first conformance before growing source syntax.
- Improve WebGL 2 rendering only when resource ownership, reflection, cleanup,
  deterministic diagnostics, and measured performance remain observable.
- Keep release artifacts reproducible, install-tested, SBOM-described, and
  attestable across supported native platforms.

## Candidate work with prerequisites

- Multi-file compilation requires module identity, import semantics, a source
  catalog, and dependency invalidation tests before an incremental cache.
- Instancing requires an ABI for instance transforms and batch keys.
- Generated GLSL source locations require a backend-produced line-to-source map.
- A physical-GPU canary requires a trusted dedicated runner and driver-owned
  baseline policy.
- Dynamic adapter plugins require loading/isolation policy in addition to the
  existing versioned object-safe API.

The reviewed decision and re-entry condition for every candidate is recorded in
`docs/improvement-resolution.md`.

## Non-goals

- Complete compatibility with Ruby, PHP, or Perl, or executing their original
  runtimes in the browser.
- Accepting language-specific implicit behavior outside the documented Common
  Core merely because one parser can parse it.
- A WebGL 1 fallback, native graphics runtime, or current WebGPU backend.
- A production web server, general JavaScript sandbox, or security boundary
  between a generated program and its hosting origin.
- Unbounded optimization, implicit asset discovery, or silently accepting an
  incompatible compiler/runtime artifact.
