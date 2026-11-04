<div align="center">

# PolyGL

**Write graphics sketches in Ruby, PHP, or Perl. Ship a typed WebGL 2 application.**

[Features](#features) · [Quick start](#quick-start) · [CLI](#cli) · [Development](#development) · [Docs](https://ydah.github.io/polygl/)

[![CI](https://github.com/ydah/polygl/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/ydah/polygl/actions/workflows/ci.yml)
[![WebGL stability](https://github.com/ydah/polygl/actions/workflows/webgl-stability.yml/badge.svg?branch=main)](https://github.com/ydah/polygl/actions/workflows/webgl-stability.yml)
[![Dependency security](https://github.com/ydah/polygl/actions/workflows/security.yml/badge.svg?branch=main)](https://github.com/ydah/polygl/actions/workflows/security.yml)
[![Rust 1.96.1](https://img.shields.io/badge/Rust-1.96.1-orange.svg)](https://www.rust-lang.org/)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

</div>

PolyGL is an experimental compiler and browser runtime for a documented,
portable graphics subset. Each adapter feeds the same typed compiler pipeline;
the browser receives generated ES2020 JavaScript, GLSL ES 3.00, and a WebGL 2
runtime. PolyGL does **not** embed or execute a Ruby, PHP, or Perl runtime.

```text
Ruby / PHP / Perl
        │
     adapter → typed HIR → domain-resolved LIR → JS / GLSL → WebGL 2
```

> **Project status:** experimental. The [Common Core](docs/common-core.md) and
> [graphics API](docs/api.md) define the supported surface; language features
> outside those contracts are intentionally rejected or unsupported.

## Features

### One graphics model, three source languages

- Ruby (`.rb`), PHP (`.php`), and Perl (`.pl`) share one type system, IR, and
  backend contract.
- Tier 1 covers drawing, input, transforms, text, and user shaders.
- Retained Tier 2 covers meshes, cameras, lights, materials, textures, and
  resource lifecycle management.
- `polygl new-adapter` scaffolds a language adapter without copying compiler
  internals. See the [adapter guide](docs/adapter-guide.md).

### Debuggable output with explicit contracts

- Debug builds preserve source locations and insert collection, vector, matrix,
  and absence checks; release builds remove those checks.
- Generated JavaScript and shader artifacts carry versioned ABI markers and
  reflection metadata. The runtime validates them before drawing.
- Source Maps are external by default in debug mode and opt-in for source
  content, so publishing source is a deliberate deployment decision.
- `polygl-manifest.json` records compiler, adapter, schema, ABI, option, and
  artifact digests without embedding generation time.

Read the [shader ABI](docs/shader-abi.md), [debugging guide](docs/debugging.md),
and [deployment guide](docs/deployment.md) before publishing a build.

### A local feedback loop that keeps the last good build

`polygl serve` binds to loopback, serves debug artifacts with caching disabled,
and can watch source, config, templates, public assets, and declared textures.
Successful rebuilds reload connected pages; failed rebuilds preserve the last
valid generation and show escaped, source-located diagnostics in the browser.

### Browser-first runtime

The TypeScript runtime owns the WebGL 2 boundary, resource cleanup, context-loss
policy, resize/DPR handling, input, batching, state-cache invalidation, shader
reflection, and accessible error reporting. Runtime tests exercise a hostile
fake WebGL2 boundary, while browser conformance runs pinned Chromium/SwiftShader
framebuffer cases.

### Reproducible, platform-aware distribution

The npm launcher selects a native package for Linux x64/arm64, macOS x64/arm64,
or Windows x64 through `optionalDependencies`; installation does not run a
download script. Tagged GitHub Releases contain the same target-named archives
and SHA-256 checksums. See [releasing](docs/releasing.md) for the publication
preflight and license notices.

## Quick start

### Install the CLI

For a published release:

```console
npm install --global @polygl/cli
polygl languages
```

The npm launcher supports Node.js 20 or later. Release availability and checksums
are listed on [GitHub Releases](https://github.com/ydah/polygl/releases). A Rust
installation can use `cargo install polygl-cli` after the compiler crates have
been published.

### Create a sketch

Save this as `sketch.rb`:

```ruby
def setup
  size(640, 360)
  background(0.03, 0.04, 0.08)
  fill(0.2, 0.75, 1.0)
  triangle(80.0, 300.0, 320.0, 55.0, 560.0, 300.0)
end
```

Check it, then start the loopback development server:

```console
polygl check sketch.rb
polygl serve sketch.rb --watch --open
```

The server listens on <http://127.0.0.1:4173> by default. For a deployable
directory, build an explicit release artifact:

```console
polygl build sketch.rb -o dist --release --source-map none
```

Start with the language-specific guides for [Ruby](docs/getting-started/ruby.md),
[PHP](docs/getting-started/php.md), or [Perl](docs/getting-started/perl.md).

## CLI

| Command | Purpose |
| --- | --- |
| `polygl build <source>` | Package `index.html`, JavaScript, shaders, runtime, assets, and a provenance manifest. |
| `polygl serve <source>` | Run the loopback development server; add `--watch` and `--open` for a live browser loop. |
| `polygl check <source>` | Parse, type-check, lower, and validate without writing artifacts. |
| `polygl dump-hir <source>` | Print fully typed HIR for compiler and adapter debugging. |
| `polygl emit <source>` | Emit selected `hir`, `lir`, `js`, `glsl`, or `manifest` stages; `-` reads UTF-8 stdin with `--language`. |
| `polygl explain <code>` | Explain a stable diagnostic code. |
| `polygl languages [--json]` | List adapters and accepted source extensions. |
| `polygl new-adapter <language>` | Create a standalone Rust adapter scaffold. |
| `polygl completions <shell>` | Generate shell completions. |
| `polygl man` | Print the bundled man page. |

Useful build options include `--debug`, `--release`, `--source-map
<none|external|inline>`, `--sources-content`, `--public-dir`, `--html-template`,
`--base-url`, `--hashed-filenames`, `--watch`, and `--profile`. The full output
contract, config schema, asset safety rules, and diagnostic formats are in the
[CLI reference](docs/cli.md).

## Examples

The repository includes runnable sketches for 2D drawing, interaction, shaders,
textures, and retained 3D scenes:

```console
polygl serve examples/triangle.rb --watch --open
polygl serve examples/interactive.rb --watch --open
polygl serve examples/plasma.rb --watch --open
polygl serve examples/texture_lifecycle.rb --watch --open
polygl serve examples/rotating_cubes.php --watch --open
polygl serve examples/terrain.rb --watch --open
```

See the [examples guide](docs/examples.md) for the supported API and the
[performance guide](docs/performance.md) for measurement boundaries. Compiler
speed does not make browser code execute as native Rust: frame time depends on
generated JavaScript, WebGL batching, shader work, browser, and GPU.

## Architecture

PolyGL keeps language-specific behavior at the adapter boundary and makes the
rest of the pipeline language-neutral:

1. An adapter parses one source file and lowers it into the documented Common
   Core with stable spans and diagnostics.
2. The compiler validates options and budgets, infers types, produces typed HIR,
   resolves domain dependencies, and splits the program into runtime stages.
3. Backends emit JavaScript, reflected GLSL ES 3.00, Source Maps, assets, and a
   reproducibility manifest.
4. The browser runtime validates the artifact ABI and shader reflection before
   it takes ownership of a WebGL 2 context.

The [architecture tutorial](docs/architecture-tutorial.md),
[Common Core contract](docs/common-core.md), and [runtime contract](docs/runtime.md)
walk through the boundaries in detail.

## Development

The repository pins Rust 1.96.1, Node.js 24.14.1, and pnpm 10.33.0. Enable the
package-manager shim, install the runtime/browser fixtures, and run the same
gates used by CI:

```console
corepack enable
pnpm --dir runtime install --frozen-lockfile
pnpm --dir conformance/browser install --frozen-lockfile
pnpm --dir conformance/browser exec playwright install chromium

cargo fmt --all -- --check
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo xtask gen-runtime --check
cargo xtask capabilities --check
cargo xtask release-stages
cargo xtask conformance

pnpm --dir runtime test
pnpm --dir conformance/browser test
npm run test:npm-cli
```

For a change that crosses the browser boundary, also run
`pnpm --dir runtime build` and commit the generated bundle when it changes.
`CONTRIBUTING.md` documents generated files, test layers, adapter work, and the
reproducible-build check.

| Directory | Responsibility |
| --- | --- |
| `crates/` | Compiler, adapters, HIR/LIR, JavaScript/GLSL backends, and CLI. |
| `runtime/` | Strict TypeScript WebGL 2 runtime and its generated bundle. |
| `conformance/` | Neutral-HIR, snapshot, framebuffer, and browser evidence. |
| `xtask/` | Generation, capability, release-stage, and conformance orchestration. |
| `docs/` | Specifications, guides, runtime contracts, and architectural decisions. |
| `npm/` | Native npm launcher and platform package staging. |

## Portability and limits

- Output targets modern browsers with WebGL 2 and ES modules; builds must be
  served over HTTP rather than opened from `file:` URLs.
- The source language is compile-time syntax, not a browser-side runtime. Dynamic
  evaluation and semantics outside the Common Core are not portable PolyGL code.
- Shader functions and uniforms must follow the [reflected shader ABI](docs/shader-abi.md).
- Browser framebuffer evidence is pinned to Chromium/SwiftShader for stable CI;
  other browsers and real GPUs remain useful compatibility checks but are not
  silently treated as equivalent baselines.
- `polygl serve` is intentionally loopback-only. Use the [deployment guide](docs/deployment.md)
  for cache headers, CSP, base paths, MIME types, and Source Map disclosure.

## Documentation

- [Documentation site](https://ydah.github.io/polygl/)
- [Getting started](docs/getting-started/ruby.md)
- [CLI reference](docs/cli.md)
- [Graphics API](docs/api.md)
- [Conformance](docs/conformance.md)
- [Debugging](docs/debugging.md)
- [Adapter authoring](docs/adapter-guide.md)
- [Contributing](CONTRIBUTING.md)
- [Release process](docs/releasing.md)

## License

PolyGL is licensed under either the Apache License, Version 2.0 or the MIT
license, at your option. See [LICENSE-APACHE](LICENSE-APACHE) and
[LICENSE-MIT](LICENSE-MIT). Native release archives and npm packages also ship
`THIRD_PARTY_LICENSES.txt` for the complete dependency closure of the CLI.
