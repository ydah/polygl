---
---

# 0026: Centralize language-neutral adapter conventions

- Status: Accepted
- Date: 2026-07-25

## Context

Implementing the PHP adapter exposed identical code in the Ruby and PHP
adapters for parsing portable `@pgl` type spellings, recognizing canonical
entry-point names, naming generated struct constructors, and recognizing
vector constructors. These are Common Core or HIR conventions rather than
source-language syntax. Keeping separate copies allows adapters to drift and
makes a third adapter repeat policy code.

Other apparently similar work remains language-specific. Comment attachment,
identifier extraction, native type hints, truthiness, operators, loop
desugaring, temporary-name hygiene, and unsupported-syntax diagnostics depend
on each parser and source language.

## Decision

Expose small pure helpers from `polygl-adapter-api` for:

- portable annotation type parsing and identifier validation;
- canonical entry-point recognition;
- the generated struct-constructor function name; and
- canonical vector-constructor recognition.

Adapters may wrap a helper to add language aliases, such as Ruby `draw` for
`frame`. They retain parser traversal, source spans, directive adjacency, and
all language semantic expansion. The shared helpers return HIR values but do
not depend on a parser or builtin registry.

## Consequences

New adapters reuse one tested definition of portable types and generated HIR
names. Ruby and PHP continue to own their syntax and diagnostics, so the shared
API does not become a lowest-common-denominator parser abstraction.

Changing an annotation spelling, canonical entry name, generated constructor
name, or vector constructor is now an adapter API change and requires
cross-language conformance review.
