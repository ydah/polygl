# L1: render equivalence

Baselines are keyed by renderer identity. Each Ruby and PHP L1 source is built,
executed in pinned Chromium with SwiftShader, and compared against the same
exact RGBA bytes. Run:

```console
pnpm --dir conformance/browser test
```

Set `UPDATE_BASELINES=1` only when an intentional rendering change has been
reviewed. Later milestones may add explicitly configured tolerances where
renderer upgrades make exact comparison impractical.
