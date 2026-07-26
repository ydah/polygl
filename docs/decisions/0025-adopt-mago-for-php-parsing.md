---
---

# 0025: Adopt Mago for PHP parsing

- Status: Accepted
- Date: 2026-07-25

## Context

The PHP adapter needs a maintained parser with typed nodes, complete byte
spans, retained comments, and reliable DocBlock association for source
annotations. `php-parser-rs` is archived and exposes only start positions.
`tree-sitter-php` is current and lightweight, but would require PolyGL to build
and maintain a typed PHP tree and DocBlock association over string node kinds.

Mago's `mago-syntax` exposes an arena-backed typed CST, half-open byte spans,
parse diagnostics, trivia, and DocBlock helpers. It is licensed MIT OR
Apache-2.0, and the dependency closure reviewed in S-4 contains only
permissive licenses. The larger dependency graph and Mago's release cadence
increase build and upgrade cost.

The newest Mago releases available at the decision date require Rust 1.97,
while PolyGL pins Rust 1.96.1. Mago 1.43.0 supports Rust 1.96 and provides the
parser capabilities needed by the adapter. Its internal dependencies use
compatible version ranges, so pinning only `mago-syntax` does not keep the
complete Mago set on one release.

## Decision

Use `mago-syntax` for PHP parsing and pin every resolved `mago-*` crate to
exactly 1.43.0. Keep Mago CST references inside a single adapter lowering call
and expose only owned PolyGL HIR. Maintain a characterization test for parse
errors, spans, trivia, and annotation DocBlock association.

Treat `tree-sitter-php` as the fallback if Mago becomes unsuitable. Upgrade the
Rust toolchain separately before evaluating Mago 1.44 or later.

## Consequences

PHP lowering can use typed, source-spanned nodes without duplicating a parser
or comment association layer. Builds gain a comparatively large parser
dependency closure, and Cargo.lock changes must be reviewed to ensure every
Mago package remains on the exact selected release.

Mago types must not cross the PHP adapter boundary. A parser upgrade requires
the characterization test and adapter conformance suite to pass. Distributed
license notices must include the permissive third-party dependency closure.
