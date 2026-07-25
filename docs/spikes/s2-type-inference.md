# S-2: Type inference and call-site monomorphization

- Date: 2026-07-25
- Time box: 1 day
- Result: **Go**

## Question

Can the design in §5.6 type common dynamically typed source without requiring
annotations everywhere, while still rejecting type-changing reassignment and
bounding code growth?

## Probe

`spikes/type-inference` is a dependency-free Rust prototype of the risky parts:

- local inference from literals and expressions;
- the sole implicit widening, `int` to `float`;
- rejection of unrelated reassignment types;
- call-site function instances keyed by argument-type tuples;
- reuse of an existing instance and an eight-instance limit;
- empty-array inference from an annotation or later use;
- rejection of an unconstrained or heterogeneous array;
- backward constraints from builtin signatures;
- order-independent numeric array joins and recursive-cycle detection.

Run it with:

```console
cargo test --manifest-path spikes/type-inference/Cargo.toml
```

All sixteen focused cases pass.

## Findings

The strategy is feasible if production inference is constraint-based and keeps
unknown element types until later uses have been analyzed. Function bodies can
be checked in an environment created from each concrete argument tuple.
Builtin parameter types propagate into unresolved arguments. Separate
assignment compatibility from type joins so an `int` binding may widen across
control flow while a `float` never satisfies an `int` annotation. The prototype
also demonstrates that the configured instance limit can be enforced before
compiling a ninth specialization.

The prototype deliberately does not cover control-flow joins, recursive call
graphs, source annotations, structs, GPU-domain constraints, or diagnostic
spans. Those remain implementation work rather than evidence against the
strategy.

## Gate

Proceed with bidirectional local inference and call-site monomorphization.
Retain annotation mode as an error-recovery path for unresolved values, not as
the primary language mode. Production implementation must add cycle detection,
flow joins, stable E0310/E0311 diagnostics, and source-backed suggestions.
