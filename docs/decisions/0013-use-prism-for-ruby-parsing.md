---
---

# 0013: Use Prism for Ruby parsing

- Status: Accepted
- Date: 2026-07-25

## Context

The Ruby adapter needs exact UTF-8 byte locations, recovery diagnostics, and a
maintained parser without evaluating user programs. A Ruby subprocess would add
a runtime dependency and serializing `RubyVM::AbstractSyntaxTree` would depend
on an implementation-specific API. A tree-sitter grammar is useful for editor
recovery but does not model all Ruby syntax as precisely as the reference
parser.

The parser spike verified literal values, Unicode and CRLF byte offsets,
comments, syntax diagnostics, and traversal of Common Core constructs.

## Decision

Use the vendored `ruby-prism` crate and pin version 1.9.0 exactly. Convert Prism
locations to validated `polygl-span` byte spans at the adapter boundary. Reject
syntax outside the documented Common Core instead of attempting to execute or
partially interpret Ruby.

## Consequences

The compiler has no Ruby runtime dependency and receives source locations from
Ruby's reference parser. The native parser increases build time and requires a
C toolchain. Parser upgrades are explicit compatibility changes and must rerun
adapter diagnostics and snapshot tests before the pin changes.
