---
title: Spike policy
permalink: /spikes/
---

# Spike policy

A spike answers one bounded uncertainty; it is not production code by default.
Every `spikes/` directory must have a question, pinned dependencies, license
review, repeatable command, result, and one status:

- **active**: currently maintained and run by a named CI job;
- **accepted**: its result became production code/ADR and the spike is frozen;
- **superseded**: a later experiment/decision replaces it;
- **frozen reference**: retained for evidence but not supported or updated.

| Spike | Status | Production decision / CI |
| --- | --- | --- |
| Ruby Prism | Accepted | ADR 0013; production adapter owns current tests |
| Type inference | Accepted | ADR 0009; `polygl-types` owns current tests |
| WebGL CI stability | Active | `webgl-stability.yml` compares independent pinned SwiftShader runs |
| PHP parser | Accepted | ADR 0025; production adapter owns current tests |
| Perl parser | Accepted | ADR 0011; production adapter owns current tests |

Accepted/frozen spike lockfiles are scanned for vulnerabilities but their crates
are excluded from the workspace. Only an active spike has a compatibility
expectation and dedicated CI. Dependabot may propose updates; an update to a
frozen spike is merged only when needed to reproduce the original question.

Promotion requires moving the smallest production design into normal crates,
adding adversarial/conformance tests and documentation, recording an ADR when a
contract changes, and removing any assumption that was safe only in the spike.
