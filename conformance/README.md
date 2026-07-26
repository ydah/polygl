# Conformance tests

PolyGL conformance is split into the three layers defined by the design:

- `l1-render`: behavioral equivalence through deterministic rendering.
- `l2-hir-snapshots`: language-specific HIR regression snapshots.
- `l3-neutral-hir`: normalized HIR equality for the neutral subset.

`polygl-conformance` implements capability-based case selection, renderer-keyed
L1 frame comparison, language-specific L2 snapshot verification, normalized L3
equality, and GPU split/backend checks. `cargo xtask conformance` compiles all
committed Ruby cases and checks five L1 baselines, five L2 snapshots, two
neutral L3 snapshots, one positive GLSL case, and two expected GPU diagnostics.

`pnpm --dir conformance/browser test` rebuilds those cases with the CLI and
compares real WebGL2 framebuffer bytes under pinned Chromium + SwiftShader. It
also starts the plasma case to exercise driver compilation, linking, automatic
uniform reflection, and compile-time material name resolution.
