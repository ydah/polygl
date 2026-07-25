# S-1: Ruby Prism source fidelity

- Date: 2026-07-25
- Time box: 0.5 day
- Result: **Go**

## Question

Can the Rust binding for Prism provide the source spans, literal distinctions,
and comments needed by the Ruby adapter and annotation directives?

## Probe

An isolated Rust probe used `ruby-prism = "=1.9.0"` with vendored libprism to
parse UTF-8 and CRLF source containing integers, floats, strings,
interpolation, symbols, booleans, nil, arrays, hashes, inline comments, and an
embedded document comment.

```console
cargo run --manifest-path spikes/ruby-prism/Cargo.toml
```

The public API exposes node `Location` byte ranges, typed literal nodes and
values, parse errors, and a flat comment list with comment type and location.
Observed examples included:

```text
IntegerNode 30..32 raw="42" value=42
FloatNode 41..47 raw="3.25e1" value=32.5
StringNode 49..59 raw="\"héllo\\n\"" unescaped="héllo\n"
InlineComment 13..29 raw="# @pgl x: float\r"
```

On a multibyte line, the byte column and Unicode scalar column intentionally
differed, confirming that PolyGL should retain byte offsets as its canonical
span representation.

## Findings

Prism satisfies the adapter requirements and is MIT licensed. Build
`SourceFile` line-start indexes once, then translate byte offsets to the column
convention needed by diagnostics or source maps. Trim CRLF when interpreting
annotation comments and associate the flat comment records with declarations
by span.

The recovered AST must not be lowered when `ParseResult::errors()` is nonempty.
Prism parse objects carry lifetimes and raw pointers, so each adapter call must
convert immediately into owned HIR rather than cache parse nodes across threads.
Ruby integers also need an explicit i32 range check at the HIR boundary.

## Gate

Use the official `ruby-prism` crate and pin version 1.9.0 for v1 development.
Keep its vendored feature so no CRuby runtime is required. Record its MIT notice
in distribution metadata. There is no reason to re-evaluate
`lib-ruby-parser`.

Primary references:

- <https://ruby.github.io/prism/rust/doc/ruby_prism/>
- <https://github.com/ruby/prism/tree/v1.9.0/rust/ruby-prism>
- <https://github.com/ruby/prism/blob/v1.9.0/LICENSE.md>
