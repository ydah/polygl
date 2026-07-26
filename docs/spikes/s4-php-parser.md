---
---

# S-4: PHP parser selection

- Date: 2026-07-25
- Time box: 0.5 day
- Result: **Go with Mago**

## Question

Which maintained Rust-accessible parser best supports typed PHP lowering,
complete byte spans, comments for annotations, and PolyGL's permissive license?

## Comparison

`php-parser-rs` has a convenient owned AST but was archived in 2024, records
only start positions, and does not cover current PHP syntax. It is not a viable
foundation.

`tree-sitter-php` is maintained, MIT licensed, current through PHP 8.5 syntax,
and lightweight. It provides complete ranges and comments, but PolyGL would
need to build a typed PHP AST, DocBlock association, and much more lowering
logic over string node kinds.

Mago's current `mago-syntax` provides an arena-backed typed CST, byte
half-open spans with file identifiers, a walker, all trivia, distinct DocBlock
comments, and a helper that associates a DocBlock with its node. It models PHP
7 through 8.5 constructs and is MIT OR Apache-2.0. Its cost is a substantially
larger dependency graph and an API that changes more frequently.

## Probe

The newest Mago 1.44/1.45 releases require Rust 1.97, while the pinned stable
toolchain available during the spike is Rust 1.96.1. Mago 1.43.0 declares Rust
1.96 support. Its parser, CST, span, trivia, DocBlock, pipe, and void-cast
implementations match the newer parser sources. A published-crate probe on
1.43.0 successfully associated a DocBlock with the following node.

The committed characterization probe verifies parse errors, complete node and
DocBlock spans, trivia retention, and `@pgl` DocBlock association:

```console
cargo test --manifest-path spikes/php-parser/Cargo.toml
```

Pinning only `mago-syntax` is insufficient because its internal Mago
dependencies use compatible ranges. The complete `mago-*` set must remain at
1.43.0 in `Cargo.lock`.

## Gate

Adopt `mago-syntax` and the corresponding Mago crates at exactly 1.43.0 for the
initial PHP adapter. Keep all CST references inside one adapter call and expose
only owned PolyGL HIR. Upgrade the Rust toolchain separately before evaluating
Mago 1.45 or later.

Use `tree-sitter-php` as the fallback if Mago becomes unsuitable.
Characterization tests and an exact lock mitigate Mago API churn. Third-party
license notices must cover its permissive dependency closure.

Primary references:

- <https://github.com/php-rust-tools/parser>
- <https://github.com/carthage-software/mago/tree/1.43.0/crates/syntax>
- <https://github.com/carthage-software/mago/blob/1.43.0/crates/syntax/src/comments/docblock.rs>
- <https://github.com/tree-sitter/tree-sitter-php/releases/tag/v0.24.2>
