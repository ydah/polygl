## What changed

<!-- Describe the user-visible outcome and why this is the smallest sound change. -->

## Evidence

<!-- List focused adversarial tests and the broader validation you ran. -->

## Cross-layer checklist

- [ ] Common Core semantics and source-language differences remain documented.
- [ ] FeatureTag/version and all affected adapters agree.
- [ ] HIR, typed HIR, LIR, split, and pass invariants agree.
- [ ] JavaScript, GLSL/reflection, runtime, and artifact ABI agree.
- [ ] Conformance manifests/snapshots/browser evidence cover the behavior.
- [ ] Generated files (`gen-runtime`, capabilities, runtime bundle) are current.
- [ ] Source Map/privacy, resource lifecycle, and deployment docs are updated.
- [ ] Compatibility/version impact is stated; an ADR exists for a changed contract.
- [ ] Failure cleanup, malformed input, deterministic order, and resource budgets
      were considered.
