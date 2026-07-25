# Conformance tests

PolyGL conformance is split into the three layers defined by the design:

- `l1-render`: behavioral equivalence through deterministic rendering.
- `l2-hir-snapshots`: language-specific HIR regression snapshots.
- `l3-neutral-hir`: normalized HIR equality for the neutral subset.

M0-1 reserves and validates this layout. The runners, fixtures, renderer-keyed
baselines, and `FeatureTag` selection are implemented in M0-6.
