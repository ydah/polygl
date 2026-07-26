# PolyGL

PolyGL is an experimental compiler and WebGL framework for writing graphics in
Ruby, PHP, or Perl. It lowers each language's supported Common Core into shared
typed HIR and LIR, then emits ES2020 JavaScript and GLSL ES 3.00 for WebGL 2.

```text
Ruby / PHP / Perl -> adapter -> HIR -> types -> LIR -> JS / GLSL -> WebGL 2
```

Ruby, PHP, and Perl support Tier 1 drawing and input, user shaders, and retained
Tier 2 meshes, cameras, lights, materials, and textures. PolyGL compiles a
documented portable subset; it does not embed or execute the source-language
runtime.

## Run your first sketch

Install a tagged native build from npm:

```console
npm install --global @polygl/cli
polygl languages
```

The npm launcher supports Linux x64/arm64, macOS x64/arm64, and Windows x64.
It selects a platform package through `optionalDependencies` and does not
download executables from an install script. The same binaries and SHA-256
checksums are available from
[GitHub Releases](https://github.com/ydah/polygl/releases). A Rust installation
can use `cargo install polygl-cli` once the compiler crates are published.

Create `sketch.rb`:

```ruby
def setup
  size(640, 360)
  background(0.03, 0.04, 0.08)
  fill(0.2, 0.75, 1.0)
  triangle(80.0, 300.0, 320.0, 55.0, 560.0, 300.0)
end
```

Check it, start the local development server, and open
<http://127.0.0.1:4173>:

```console
polygl check sketch.rb
polygl serve sketch.rb --watch
```

Successful saves rebuild and reload the page. A failed rebuild keeps the last
valid sketch running and shows a source-located diagnostic in the browser.

Use the language-specific guides for the equivalent first program:

- [Ruby](docs/getting-started/ruby.md)
- [PHP](docs/getting-started/php.md)
- [Perl](docs/getting-started/perl.md)

The complete documentation is published at
[ydah.github.io/polygl](https://ydah.github.io/polygl/).

## CLI

```text
polygl build <source.rb|source.php|source.pl> [-o <directory>] [--debug | --release]
polygl serve <source.rb|source.php|source.pl> [--port <port>] [--watch]
polygl check <source.rb|source.php|source.pl>
polygl dump-hir <source.rb|source.php|source.pl>
polygl languages
polygl new-adapter <language> [-o <directory>]
polygl --version
```

`build` writes a self-contained browser application with `index.html`,
`app.js`, source maps, reflected shaders, packaged texture assets, and the
embedded runtime. Debug checks are enabled by default; `--release` removes
compiler-inserted collection, vector/matrix, and absence checks. See the
[CLI reference](docs/cli.md) for exact behavior.

## Examples

The repository includes runnable 2D, interactive, shader, and 3D sketches:

```console
polygl serve examples/triangle.rb --watch
polygl serve examples/triangle.pl --watch
polygl serve examples/interactive.rb --watch
polygl serve examples/plasma.rb --watch
polygl serve examples/rotating_cubes.php --watch
polygl serve examples/rotating_cubes.pl --watch
polygl serve examples/terrain.rb --watch
```

## Performance model

Rust improves parsing, type inference, specialization, and code-generation
speed. It does not make the browser execute user code as native Rust. Runtime
performance is determined by the generated JavaScript, the WebGL 2 batching
runtime, shader work, and the browser/GPU. See the
[performance guide](docs/performance.md) for measurement boundaries.

## Development

The repository pins Rust 1.96.1, Node.js 24.14.1, and pnpm 10.33.0. Install the
runtime and browser-test dependencies, then run the local gates:

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

The main directories are:

- `crates/`: compiler, backends, adapters, and CLI
- `runtime/`: strict TypeScript WebGL runtime
- `conformance/`: L1 rendering, L2 snapshots, and L3 neutral-HIR checks
- `xtask/`: generated-file and conformance orchestration
- `docs/`: specifications, language guides, and architectural decisions
- `npm/`: platform packages and release staging

Start with the [architecture contracts](docs/common-core.md) or the
[adapter authoring guide](docs/adapter-guide.md) when contributing a language.

## License

PolyGL is licensed under either the Apache License, Version 2.0 or the MIT
license, at your option. See `LICENSE-APACHE` and `LICENSE-MIT`. Native release
archives and npm packages also include `THIRD_PARTY_LICENSES.txt` for the
complete crate dependency closure linked into the command-line executable,
including publishable PolyGL workspace crates and third-party crates. The
separate PolyGL license files remain authoritative for PolyGL itself.
