# Conformance runner

PolyGL evaluates adapters in three independent layers. A `ConformanceCase`
declares its layer and required `FeatureTag`s; the runner selects it only when
an adapter advertises every required capability.

## L1: behavioral rendering

L1 is the primary equivalence criterion. `RenderedFrame` includes the renderer
identity, dimensions, and RGBA bytes. Baselines must be renderer-keyed. The M0
mechanism performs exact comparison; later pixel tolerances must be explicit
case configuration rather than an implicit global relaxation. Tests fix random
seeds and mock time.

Smoke baselines use
`conformance/l1-render/<case>/<renderer>.rgba`: a `WIDTHxHEIGHT` header followed
by lowercase hexadecimal RGBA bytes.

## L2: language HIR snapshots

L2 detects lowering regressions without claiming cross-language identity.
Snapshots use `conformance/l2-hir-snapshots/<case>/<language>.hir`. Snapshot
names accept only lowercase ASCII letters, digits, hyphen, and underscore so a
case cannot escape the conformance root.

HIR dump snapshot tests use `insta`; the command runner also verifies committed
plain `.hir` files so `cargo xtask conformance` fails on stale fixtures.

## L3: Neutral HIR equality

L3 accepts at least two language modules, normalizes clones, and compares their
dumps. It is restricted to float arithmetic, explicit boolean conditions,
strict typed comparisons, and syntax without language-specific sugar. One
committed `neutral.hir` file records the canonical result per case.

Division/truthiness cases with intentional language differences belong to L1
and L2, not L3.

## Feature tags

`polygl-adapter-api::FeatureTag` is a closed capability vocabulary. Core/Tier1,
arrays, maps, classes, block sugars, truthiness sugar, and shaders have separate
tags. Adding a tag requires cases and adapter documentation.
