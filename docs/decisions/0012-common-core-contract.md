# 0012: Freeze the v1 Common Core contract

- Status: Accepted
- Date: 2026-07-25

## Context

PolyGL cannot reproduce the full semantics of Ruby, PHP, and Perl without
becoming three language runtimes. Adapters nevertheless need one precise
boundary for accepted programs, fixed HIR semantics, and intentional
language-specific lowering. Ambiguous behavior would otherwise leak into the
type system and backends or be approximated silently.

The design mentioned `map` as a possible M3 block expansion, while the work
plan limits M3 to `times` and `each` and places `map`/`filter` in the post-v1
backlog. The v1 boundary must resolve that conflict before adapter work begins.

DESIGN §13 reserves decision numbers 0001 through 0010. This chronological ADR
therefore follows the parser selection decision at 0011 rather than reusing one
of those identifiers.

## Decision

Adopt [Common Core v1](../common-core.md) as the normative adapter-to-HIR
contract. Adapters preserve the source language only through the documented
lowerings; all other source semantics are rejected with positioned
diagnostics. HIR conditions are boolean, numeric and equality behavior is
explicit, and only `int` to `float` widens implicitly.

Limit v1 block sugar to `times` and Range/array `each`. HIR contains no closure
value. Treat `map`, `filter`, and additional block forms as post-v1 changes
requiring an RFC, FeatureTags, adapter mapping updates, and conformance tests.
Limit classes to fixed fields, constructors, and statically dispatched instance
methods.

Use behavioral equivalence as the primary cross-language conformance rule.
Compare HIR across languages only for the Neutral subset.

## Consequences

The shared compiler stages can rely on one small, statically typed semantic
model, while adapters have explicit rules for division, truthiness,
concatenation, absence, and equality. Unsupported language features fail
locally instead of changing meaning silently.

Programs cannot freely use standard-library, metaprogramming, module, closure,
or object-system features from their source language. Every Common Core
extension carries coordinated work across all adapters and test layers. The
narrow block whitelist postpones convenient collection transforms but prevents
general closure semantics from entering v1.
