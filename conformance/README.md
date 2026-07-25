# Conformance tests

PolyGL conformance is split into the three layers defined by the design:

- `l1-render`: behavioral equivalence through deterministic rendering.
- `l2-hir-snapshots`: language-specific HIR regression snapshots.
- `l3-neutral-hir`: normalized HIR equality for the neutral subset.

`polygl-conformance` implements capability-based case selection, renderer-keyed
L1 frame comparison, language-specific L2 snapshot verification, and normalized
L3 equality. `cargo xtask conformance` validates the layout and one smoke case
through all three layers.
