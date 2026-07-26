# 0024: Resolve instance methods after receiver type inference

- Status: Accepted
- Date: 2026-07-25

## Context

Common Core classes have fixed fields and statically dispatched instance
methods, but Ruby receiver syntax does not name the receiver's class. Resolving
`value.move(dx)` in the adapter would require duplicating type inference or
restricting useful receiver expressions. Carrying dynamic method lookup into
LIR and JavaScript would violate the fixed class subset and make backend
behavior language-specific.

Constructor assignments also establish the field set before call-site argument
types are known. Requiring annotations for every field would make simple Ruby
classes unnecessarily verbose and would duplicate information already present
at construction sites.

## Decision

Represent an untyped receiver call as a HIR method callee whose first argument
is the receiver. Type analysis resolves the receiver to a named struct, selects
the class-qualified method template, and rewrites the call to the ordinary
static-function specialization path. Each method template has a typed `self`
first parameter. Typed HIR and LIR therefore contain no unresolved method
dispatch.

Lower each Ruby constructor to a generated function returning a struct
expression. Unannotated fields receive one inference variable per
class-and-field pair. Constructor values, reads, and writes constrain that
shared variable; analysis must resolve every field before producing typed HIR.
Struct methods are templates only and are removed from the concrete struct
definition after reachable static instances are generated.

## Consequences

Different classes may use the same method name without sharing an
implementation, and existing monomorphization limits and diagnostics also apply
to methods. Backends remain unaware of source-language method semantics.
Method chaining whose intermediate receiver type cannot yet be determined may
require a source annotation or an intermediate local. Unused classes with
unconstrained, unannotated fields are rejected because successful typed HIR
cannot contain unresolved storage types.
