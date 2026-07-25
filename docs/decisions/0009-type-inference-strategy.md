# 0009: Infer locally and monomorphize at call sites

- Status: Accepted
- Date: 2026-07-25

## Context

PolyGL accepts a Common Core written in dynamically typed languages, but HIR
must be fully typed before host/GPU splitting and code generation. Requiring
annotations on every function would weaken the source-language experience,
while carrying dynamic values into LIR would duplicate language semantics in
every backend. Unbounded specialization would also allow accidental code-size
explosion.

The feasibility evidence is recorded in
[S-2](../spikes/s2-type-inference.md).

## Decision

Use bidirectional local constraint inference, seeded by literals, annotations,
and builtin signatures. Type user functions independently for each concrete
argument-type tuple and cache the resulting instance. Permit only `int` to
`float` implicit widening, reject other type-changing reassignment, and limit
each source function to eight instances by default.

Keep unresolved values such as unconstrained empty arrays as type variables
until later uses have been analyzed. If they remain unresolved, require a
source-language annotation and report a positioned diagnostic.

Normalize arguments through parameter annotations before forming a
specialization key. For example, an `int` passed to an annotated `float`
parameter and a direct `float` argument select the same function instance.
Defer choosing that key and emitting the instance until all constraints in the
containing body have stabilized; a later builtin use can therefore widen an
earlier call argument without leaving a stale specialization behind.

Reject recursive specialization in v1 rather than guessing a result type or
emitting a partially typed cycle.

## Consequences

Most idiomatic Common Core code needs no annotations, and LIR remains statically
typed and backend-independent. Compilation and generated code grow with the
number of concrete call signatures, so the limit and E0310 diagnostic are part
of the public contract. Recursive algorithms must be rewritten as loops or
deferred until an explicitly typed recursive strategy is designed.
