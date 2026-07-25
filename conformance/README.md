# Conformance tests

PolyGL conformance is split into the three layers defined by the design:

- `l1-render`: behavioral equivalence through deterministic rendering.
- `l2-hir-snapshots`: language-specific HIR regression snapshots.
- `l3-neutral-hir`: normalized HIR equality for the neutral subset.

`polygl-conformance` implements capability-based case selection, renderer-keyed
L1 frame comparison, language-specific L2 snapshot verification, and normalized
L3 equality. `cargo xtask conformance` compiles all committed Ruby cases and
checks five L1 baselines, five L2 snapshots, and two neutral L3 snapshots.

`pnpm --dir conformance/browser test` rebuilds those cases with the CLI and
compares real WebGL2 framebuffer bytes under pinned Chromium + SwiftShader.
