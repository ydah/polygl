# PolyGL

PolyGL is an experimental compiler and WebGL framework for writing graphics
programs in Ruby, PHP, Perl, and other host languages. It lowers each
language's supported Common Core into shared HIR and LIR, then emits JavaScript
and GLSL ES 3.00 for WebGL 2.

Ruby and PHP Common Core source can be lowered, type-checked, emitted as ES2020
with source maps, and run through the batched WebGL2 browser runtime. Shader
and Tier 1 support are available; Tier 2 and the third-language adapter remain
in progress.

## Architecture

```text
source -> language adapter -> HIR -> analysis -> LIR -> JS / GLSL -> WebGL 2
```

- `crates/`: compiler, backends, adapters, and CLI crates.
- `runtime/`: strict TypeScript WebGL runtime.
- `conformance/`: L1 rendering, L2 snapshots, and L3 neutral-HIR tests.
- `xtask/`: repository generation and validation commands.
- `docs/`: spike reports and architectural decisions.

See the [adapter authoring guide](docs/adapter-guide.md) for the tested process
for adding another source language.

Rust improves compilation speed. Runtime performance is determined by the
generated JavaScript and the runtime's rendering batches.

## Requirements

- Rust 1.96.1 (selected automatically by `rust-toolchain.toml`)
- Node.js 24.14.1 (pinned by `.node-version`)
- pnpm 10.33.0
- [just](https://github.com/casey/just)

## Development

Install runtime dependencies and run the local gates:

```console
corepack enable
pnpm --dir runtime install --frozen-lockfile
pnpm --dir conformance/browser install --frozen-lockfile
pnpm --dir conformance/browser exec playwright install chromium
just build
just test
just conformance
just gen-check
```

The equivalent generation command is `cargo xtask gen-runtime`. Generated
files must be committed and pass `cargo xtask gen-runtime --check`.

## Compile a sketch

```console
cargo run -p polygl-cli -- build examples/triangle.rb -o dist
cargo run -p polygl-cli -- serve examples/interactive.rb --watch
cargo run -p polygl-cli -- build path/to/sketch.php -o dist-php
```

The build writes `index.html`, `app.js`, `app.js.map`, and the embedded
`runtime.js`. Serve the output directory through an HTTP server so browser ES
modules can load. Debug checks are enabled by default; pass `--release` to
remove compiler-inserted collection and nil checks. The interactive example
exercises pointer and keyboard input, events, transforms, text, collections,
blocks, and a struct-like Ruby class.

Use `polygl check source.rb` or `polygl check source.php` for diagnostics
without output, and `polygl dump-hir` to inspect typed HIR. See [the CLI
reference](docs/cli.md) for details.

## License

PolyGL is licensed under either the Apache License, Version 2.0 or the MIT
license, at your option. See `LICENSE-APACHE` and `LICENSE-MIT`.
