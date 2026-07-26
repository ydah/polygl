---
---

# 0020: Require provably nonzero GPU integer divisors

- Status: Accepted
- Date: 2026-07-25

## Context

Common Core defines integer division and remainder by zero as runtime errors.
Host JavaScript can throw an error with the originating source span, but GLSL ES
has no exception mechanism and WebGL cannot observe a per-invocation arithmetic
fault without adding framebuffer readback and changing every shader interface.
Executing GLSL integer division or remainder by zero would instead have
undefined behavior.

## Decision

Reject GPU integer division and remainder with E0406 unless constant
propagation proves the divisor nonzero. The initial proof recognizes integer
literals, unary negation, and acyclic integer constants. Keep the Host behavior
as a source-located runtime error. Emit defensive zero checks in GLSL helpers so
malformed direct backend input still avoids undefined arithmetic, and
explicitly handle `INT_MIN / -1` to preserve Common Core's wrapping integer
model.

## Consequences

Accepted GPU programs have deterministic integer division and remainder
semantics across WebGL implementations. GPU code with a dynamic divisor must
move the checked operation to Host code or expose a compiler-visible nonzero
constant. This is intentionally more restrictive than the Host subset; a future
shader-fault transport or flow-sensitive proof can relax E0406 without changing
successful program behavior.
