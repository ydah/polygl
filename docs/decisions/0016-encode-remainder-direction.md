# 0016: Encode remainder direction

- Status: Accepted
- Date: 2026-07-25

## Context

The source languages do not agree on `%` for negative operands. Ruby and Perl
choose a floor-directed quotient, so the result follows the divisor's sign.
PHP and JavaScript truncate the quotient toward zero, so the result follows the
dividend's sign.

A single `Rem` operation would make either an adapter or a backend silently
inherit the wrong language behavior. The distinction cannot be recovered after
source lowering from operand and result types alone.

## Decision

Represent remainder as `RemFloor` or `RemTrunc` in HIR and as corresponding
explicit operations in LIR. Require each source adapter to select the operation
that matches its language. The JavaScript backend implements floor remainder
independently of JavaScript `%` and uses `%` only for truncating remainder.

Keep both operations numeric. Integer division by zero is a runtime error;
floating-point behavior retains the selected quotient direction.

## Consequences

Negative remainder results remain portable across source and target languages,
and conformance snapshots expose the choice as `%floor` or `%trunc`. Adapters
have one additional semantic decision, while shared type inference can treat
both operations identically.
