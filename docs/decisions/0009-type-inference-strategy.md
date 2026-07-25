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

## Consequences

Most idiomatic Common Core code needs no annotations, and LIR remains statically
typed and backend-independent. Compilation and generated code grow with the
number of concrete call signatures, so the limit and E0310 diagnostic are part
of the public contract. Recursive calls, control-flow joins, and annotations
need explicit production handling beyond the validating S-2 prototype.
