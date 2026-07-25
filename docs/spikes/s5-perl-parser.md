# S-5: Perl parser feasibility

- Date: 2026-07-25
- Time box: 0.5 day
- Result: **Go**

## Question

Can the maintained Tree-sitter Perl grammar parse the Common Core well enough
to keep Perl as the third v1 language?

## Probe

An isolated Rust probe compiled `ts-parser-perl = "1.2.1"` with
`tree-sitter = "0.26.11"` and parsed a 47-line program accepted by Perl 5.34.
The program covered comments, packages, subroutines, lexical variables,
assignment, numeric/logical/comparison expressions, conditionals, while and
both for forms, arrays, hashes, indexing, `bless`, constructors, and method
calls.

```console
cargo test --manifest-path spikes/perl-parser/Cargo.toml
```

The root was a `source_file` with no error, missing node, or recovery node.
Byte and row/column ranges were present. The binding publishes its language
function, node-type description, and editor queries, and compiles generated C
without bindgen.

The upstream grammar is MIT licensed, actively released, and reported 98.5%
clean parsing across its broad 8,334-file corpus, with the remaining gaps
concentrated in syntax outside PolyGL's Common Core.

## Findings

The package name matters: the maintained upstream package is
`ts-parser-perl`, not the unrelated crates named `tree-sitter-perl` or
`tree-sitter-perl-next`.

Version 1.2.1 has a known field-splat bug around parenthesized inline rules:
field access can return punctuation such as `(` or `,`. The concrete tree is
still correct. Adapter utilities must filter for named children and retain a
structural traversal fallback, with regression fixtures for parenthesized
declarations and expressions. Upstream PR #262 targets the correction for
1.3.0.

Tree-sitter recovery means every adapter parse must reject roots containing
`ERROR` or `MISSING`. The generated parser is also large, so release size and
incremental build cost should be monitored.

## Gate

Keep Perl as the third v1 language and pin `ts-parser-perl` 1.2.1. Upgrade only
after comparing node schemas and HIR snapshots against a released fix. Lua
remains the fallback, but switching now would add table/metatable class
conventions without solving a current blocker.

Primary references:

- <https://github.com/tree-sitter-perl/tree-sitter-perl>
- <https://github.com/tree-sitter-perl/tree-sitter-perl/tree/master/benchmark>
- <https://github.com/tree-sitter-perl/tree-sitter-perl/pull/262>
