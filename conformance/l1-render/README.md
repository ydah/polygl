# L1: render equivalence

Baselines are keyed by renderer identity. M1 builds each source in
`conformance/cases`, executes it in pinned Chromium with SwiftShader, reads the
WebGL framebuffer, and compares exact RGBA bytes. Run:

```console
pnpm --dir conformance/browser test
```

Set `UPDATE_BASELINES=1` only when an intentional rendering change has been
reviewed. Later milestones may add explicitly configured tolerances where
renderer upgrades make exact comparison impractical.
