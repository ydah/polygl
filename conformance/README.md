# Conformance tests

PolyGL conformance is split into the three layers defined by the design:

- `l1-render`: behavioral equivalence through deterministic rendering.
- `l2-hir-snapshots`: language-specific HIR regression snapshots.
- `l3-neutral-hir`: normalized HIR equality for the neutral subset.

`polygl-conformance` implements capability-based case selection, renderer-keyed
L1 frame comparison, language-specific L2 snapshot verification, normalized L3
equality, and GPU split/backend checks. `cargo xtask conformance` compiles all
committed Ruby and PHP render cases and checks five shared L1 baselines, ten L2
snapshots, two cross-language neutral L3 comparisons and snapshots, one
positive GLSL case, and two expected GPU diagnostics.

`pnpm --dir conformance/browser test` rebuilds every L1 case in Ruby and PHP
with the CLI and compares both real WebGL2 framebuffers against the same bytes
under pinned Chromium + SwiftShader. It
also starts the plasma case to exercise driver compilation, linking, automatic
uniform reflection, and compile-time material name resolution.
