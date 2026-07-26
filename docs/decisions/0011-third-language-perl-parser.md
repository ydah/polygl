---
---

# 0011: Keep Perl and use the maintained Tree-sitter grammar

- Status: Accepted
- Date: 2026-07-25

## Context

Perl is the riskiest planned adapter because its syntax is context-sensitive
and several similarly named Tree-sitter crates exist. A parser failure would
replace Perl with Lua in M6. The chosen parser must cover PolyGL's Common Core,
provide owned source ranges through Rust, have a compatible license, and remain
maintained.

The parser probe and known grammar limitation are recorded in
[S-5](../spikes/s5-perl-parser.md).

## Decision

Keep Perl as the third v1 language. Pin `ts-parser-perl` 1.2.1 from
`tree-sitter-perl/tree-sitter-perl`, with a compatible Tree-sitter runtime.
Reject recovered trees containing `ERROR` or `MISSING`.

Do not trust field access to return named nodes in version 1.2.1. The shared
Tree-sitter adapter utility will filter named children and provide structural
fallbacks, backed by regressions for parenthesized variable declarations and
expressions. Reconsider 1.3.0 only after its field fix is released and node
schema plus HIR snapshots have been compared.

## Consequences

The original three-language scope remains intact, and package/bless syntax can
map directly to the v1 struct-like class model. Generated parser C adds compile
time and binary size. The adapter must carry a temporary defensive traversal,
and dependency updates require schema review. Lua stays documented as the
fallback but is not implemented in place of a viable Perl parser.
