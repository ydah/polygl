---
---

# 0006: Make behavior the primary conformance criterion

- Status: Accepted
- Date: 2026-07-25

## Context

Adapters preserve intentional source-language semantics, so equivalent-looking
programs can legitimately emit different HIR for division and truthiness.
Requiring identical HIR across every language would contradict that rule.
Behavior-only tests, however, would make lowering regressions difficult to
localize.

## Decision

Use three layers. L1 renderer-keyed output is the primary cross-language
criterion. L2 keeps a separate HIR snapshot for each language and case. L3
compares normalized HIR only for the Neutral subset where language semantics do
not differ.

Select cases from explicit adapter FeatureTags. Keep unsupported-syntax
diagnostic tests alongside the three layers rather than treating rejection as
a rendering case.

## Consequences

Intentional HIR differences no longer create false conformance failures, while
language-specific snapshots still detect lowering drift. Neutral cases prove
that adapters avoid unnecessary transformations where their semantics agree.

The suite carries more fixtures and every case author must choose the correct
layer. L1 also requires deterministic time, randomness, renderer selection, and
pixel-baseline policy.
