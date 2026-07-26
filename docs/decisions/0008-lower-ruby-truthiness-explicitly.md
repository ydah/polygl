---
---

# 0008: Lower Ruby truthiness explicitly

- Status: Accepted
- Date: 2026-07-25

## Context

HIR conditions require booleans, while Ruby accepts every value and treats only
`nil` and `false` as false. Expanding a condition to two equality checks would
evaluate calls and other effectful expressions twice unless adapters introduced
temporary variables. Reusing `NilCheck` alone would incorrectly treat `false`
as true.

PHP and Perl have broader coercion rules that the Common Core deliberately
rejects, so the representation must not become a configurable cross-language
truthiness operation.

## Decision

Add `FalsyCheck(value)` to HIR with one fixed operation: evaluate `value` once
and return true exactly when the result is `nil` or `false`. Ruby conditions
lower to `not FalsyCheck(value)`. Ruby conjunction, disjunction, and negation in
condition position recursively apply this conversion while retaining
short-circuit evaluation.

PHP and Perl adapters may not use `FalsyCheck` to reproduce their source
language coercion rules.

## Consequences

Backends receive boolean conditions and can preserve Ruby side effects without
compiler-generated temporaries. HIR gains one language-motivated primitive, but
its semantics are closed and testable. Type analysis and every backend must
handle `FalsyCheck`; conformance classifies Ruby truthiness cases outside the
Neutral HIR subset.
