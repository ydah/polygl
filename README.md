# PolyGL

PolyGL is an experimental compiler and WebGL framework for writing graphics
programs in Ruby, PHP, Perl, and other host languages. It lowers each
language's supported Common Core into shared HIR and LIR, then emits JavaScript
and GLSL ES 3.00 for WebGL 2.

The project is at the workspace-skeleton stage. Compiler and runtime behavior
described in the design is not implemented yet.

## Architecture

```text
source -> language adapter -> HIR -> analysis -> LIR -> JS / GLSL -> WebGL 2
```

- `crates/`: compiler, backends, adapters, and CLI crates.
- `runtime/`: strict TypeScript WebGL runtime.
- `conformance/`: L1 rendering, L2 snapshots, and L3 neutral-HIR tests.
- `xtask/`: repository generation and validation commands.
- `docs/`: spike reports and architectural decisions.

Rust improves compilation speed. Runtime performance is determined by the
generated JavaScript and the runtime's rendering batches.

## Requirements

- Rust 1.96.1 (selected automatically by `rust-toolchain.toml`)
- Node.js 24 or newer
- pnpm 10.33.0
- [just](https://github.com/casey/just)

## Development

Install runtime dependencies and run the local gates:

```console
pnpm --dir runtime install --frozen-lockfile
just build
just test
just conformance
just gen-check
```

The equivalent generation command is `cargo xtask gen-runtime`. Generated
files must be committed and pass `cargo xtask gen-runtime --check`.

## License

PolyGL is licensed under either the Apache License, Version 2.0 or the MIT
license, at your option. See `LICENSE-APACHE` and `LICENSE-MIT`.
