---
---

# Command-line interface

The CLI accepts one source file and runs the shared compiler pipeline.

```text
polygl build <source.rb|source.php|source.pl> [-o <directory>] [--debug | --release] [--source-map <none|external|inline>] [--sources-content]
polygl serve <source.rb|source.php|source.pl> [--port <port>] [--watch]
polygl check <source.rb|source.php|source.pl>
polygl dump-hir <source.rb|source.php|source.pl>
polygl languages
polygl new-adapter <language> [-o <directory>]
polygl --version
```

`build` defaults to `dist` and debug mode. It writes:

- `index.html`, whose module bootstrap activates the runtime before dynamically
  importing the generated program;
- `app.js` and, when external Source Maps are selected, `app.js.map`;
- `shaders.js`, containing data-only GLSL and reflection metadata;
- `runtime.js`, embedded in the CLI binary from the tested TypeScript runtime;
- `polygl-manifest.json`, a reproducible provenance record for the build.

The manifest records compiler and adapter versions, versioned feature tags,
HIR/builtin/runtime ABI versions, normalized source path and BLAKE3 digest,
effective build options, and a sorted size/digest inventory of every payload
artifact. It deliberately omits generation time so identical inputs and options
produce identical bytes. The manifest does not hash itself.

Every `texture_load` argument must be a literal relative slash-separated path.
`build` reads it relative to the source file and copies it to the same path
under the output directory. Dynamic paths, absolute paths, `.` or `..`
components, URL/drive prefixes, backslashes, and names that would overwrite a
generated artifact produce E0501. A missing source asset fails before the
staged generation replaces the last successful output. Complete generations
are published together, so failed builds cannot mix old and new files and
stale files disappear on success.

Debug output includes source-located array, vector, matrix, and nil checks.
Release output removes those checks. Debug builds default to an external Source
Map; release builds default to no Source Map. `--source-map` selects `none`,
`external`, or `inline` in either mode. `sourcesContent` is omitted unless
`--sources-content` is explicitly passed. Source names are normalized to `/`
separators and are relative to the project working directory when possible;
sources outside it use only their basename. Build output must be served over
HTTP because browsers restrict module imports from local `file:` pages.

Source Maps reveal program structure, names, and locations. Embedding
`sourcesContent` additionally publishes the full original source, including
comments and literals. Public deployments should omit maps unless debugging is
required and should enable `--sources-content` only when that disclosure is
intentional. `serve` always enables an external map with source content because
it is loopback-only development tooling.

`serve` builds debug artifacts into a private temporary generation, binds only
to `127.0.0.1` (port 4173 by default), and serves it with caching disabled.
`--watch` hashes the source at a short interval and injects a same-origin
WebSocket client. Successful rebuilds atomically switch the active generation
and reload connected pages. Failed rebuilds preserve the last good generation
and display HTML-escaped source diagnostics in a browser overlay. A page opened
during a failing build receives the same current diagnostic. Without `--watch`,
an initial compiler error exits instead of starting a server that cannot
recover.

`check` performs parsing, Common Core lowering, type inference, and
monomorphization without writing artifacts. `dump-hir` performs the same checks
and prints fully typed HIR. Adapter and type errors use stable diagnostic codes,
source excerpts, and suggestions where required. See
[Diagnostic codes](errors.md) for the complete code table.

The source extension selects the adapter: `.rb` uses Ruby, `.php` uses PHP, and
`.pl` uses Perl. All three continue through the same type, HIR/LIR, JavaScript,
GLSL, and runtime pipeline.

`languages` prints the stable adapter identifier and accepted extension for
every adapter in the executable. `new-adapter` creates a standalone Rust crate
containing a `LanguageAdapter` implementation shell and compatible PolyGL
dependencies. The language identifier must start with a lowercase ASCII letter
and contain only lowercase letters or digits. The default destination is
`polygl-adapter-<language>`; `-o` selects another directory. Existing
destinations are never overwritten.

`--version` (or `-V`) prints the executable's Cargo package version. Release
tags and npm packages use this same version.

## Installation

The `@polygl/cli` npm package uses an optional dependency selected by operating
system and CPU. It supports Linux x64/arm64, macOS x64/arm64, and Windows x64.
The package does not run a download script during installation:

```console
npm install --global @polygl/cli
polygl languages
```

The same native executables are attached to tagged GitHub Releases as
target-named archives with a `SHA256SUMS` file. A Rust installation can use
`cargo install polygl-cli` after the compiler crates have been published.
