# Conformance tests

PolyGL conformance is split into the three layers defined by the design:

- `l1-render`: behavioral equivalence through deterministic rendering.
- `l2-hir-snapshots`: language-specific HIR regression snapshots.
- `l3-neutral-hir`: normalized HIR equality for the neutral subset.

`cases.json` is the shared declarative inventory for the Rust and browser
runners. `polygl-conformance` implements capability-based selection,
renderer-keyed L1 frame comparison, language-specific L2 snapshot verification,
normalized L3 equality, and GPU split/backend checks. `cargo xtask conformance`
checks six shared L1 baselines, 22 L2 snapshots, two cross-language neutral L3
comparisons and snapshots, one positive GLSL case, and two expected GPU
diagnostics. It also rejects an advertised feature without a case. The
`lit-cubes` L1 case exercises the Tier 2 camera, light, retained mesh, node
transform, and material path.

`pnpm --dir conformance/browser test` rebuilds every L1 case in Ruby, PHP, and Perl
with the CLI and compares both real WebGL2 framebuffers against the same bytes
under pinned Chromium + SwiftShader. It
also starts the plasma case to exercise driver compilation, linking, automatic
uniform reflection, and compile-time material name resolution.
