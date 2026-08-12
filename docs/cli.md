---
---

# Command-line interface

The CLI accepts one source file and runs the shared compiler pipeline.

```text
polygl build [source.rb|source.php|source.pl] [-o <directory>] [--config <polygl.toml>] [--debug | --release] [--source-map <none|external|inline>] [--public-dir <directory>] [--html-template <file>] [--base-url </path/>] [--hashed-filenames] [--watch]
polygl serve <source.rb|source.php|source.pl> [--port <port>] [--watch]
polygl check <source.rb|source.php|source.pl>
polygl dump-hir <source.rb|source.php|source.pl>
polygl emit <source.rb|source.php|source.pl|-> [--language <adapter-id>] [--emit <hir,lir,js,glsl,manifest>]
polygl explain <diagnostic-code>
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

Manifest schema 3 records compiler and adapter versions, versioned feature
tags, HIR/LIR/builtin schemas, runtime/shader ABI versions, normalized source
path and BLAKE3 digest, effective build options, and a sorted size/digest
inventory of every payload artifact. It also records logical app, shader,
runtime, HTML, and Source Map entrypoints so deployment tools do not guess
content-hashed filenames. It deliberately omits generation time so
identical inputs and options produce identical bytes. The manifest does not
hash itself.

The manifest is build provenance for deployment tools; browsers do not load it.
Runtime compatibility is enforced from the ABI markers embedded in `app.js` and
`shaders.js`, which the runtime validates before drawing. HIR, LIR, builtin, and
adapter schema versions describe compiler/tooling exchange contracts and are not
runtime inputs.

`build` reads `polygl.toml` from the working directory when it exists;
`--config` selects another file. CLI options override config values. Paths in
the config are resolved relative to that file. A source argument is optional
when `entry` is configured. Unknown keys and invalid enum values are rejected.
The supported keys are `language`, `entry`, `output`, `mode`, `source_map`,
`sources_content`, `public_dir`, `html_template`, `base_url`,
`hashed_filenames`, and the statically representable `[runtime]` options:

```toml
language = "ruby"
entry = "sketch.rb"
output = "dist"
mode = "release"
source_map = "none"
public_dir = "public"
html_template = "shell.html"
base_url = "/gallery/"
hashed_filenames = true

[runtime]
seed = 42
auto_resize = true
external_webgl_policy = "reset"

[runtime.resource_limits]
max_meshes = 128
max_texture_bytes = 67108864
```

A custom template must contain exactly one `<!-- polygl:app -->` marker. The
bootstrap replaces that marker without interpreting the rest of the HTML.
`base_url` begins and ends with `/` and uses only ASCII unreserved path
characters, without dot segments. `public_dir` is copied recursively; symlinks
and non-regular entries are rejected, then its paths enter the same portable
collision validation as generated and source-declared assets.

`--hashed-filenames` gives app, shader, runtime, and external Source Map files a
16-hex BLAKE3 content suffix. Their full digests remain in the manifest. The
Source Map v3 `file` member is omitted in this mode to avoid a cyclic naming
dependency; the app's `sourceMappingURL` and manifest entrypoint identify the
map. `index.html` and `polygl-manifest.json` remain stable deployment
entrypoints.

Every `texture_load` argument must be a literal relative slash-separated path.
`build` reads it relative to the source file and copies it to the same path
under the output directory. Dynamic paths, absolute paths, `.` or `..`
components, URL/drive prefixes, backslashes, and names that would overwrite a
generated artifact produce E0501. Output paths are compared after NFC and full
locale-independent Unicode case folding, and Windows-reserved names and
characters are rejected, so a build cannot contain names that collide on a
supported filesystem. A missing source asset fails before the staged generation
replaces the last successful output. Complete generations are written to an
adjacent staging directory before activation, so failed builds cannot mix old
and new files and stale files disappear on success. Replacing a non-empty output
directory requires a portable two-rename transaction: there is a brief interval
where that path is absent, and a machine crash in that interval may require the
next build to recreate it. Consumers that require uninterrupted publication
should deploy versioned directories and switch a server-owned pointer.

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

`emit` writes selected compiler stages to stdout. One selected kind is emitted
directly; multiple kinds are returned as one JSON object keyed by kind. `-`
reads UTF-8 source from stdin and requires `--language ruby|php|perl`; stdin
programs cannot reference relative assets because there is no source directory.
External Source Maps are rejected for stdout output, while inline maps are
supported. The LIR output is a developer dump for inspection, not a versioned
serialization format. `--profile` on `build` or `check` prints per-pass timings,
IR/resource counts, and generated byte sizes.

`build --watch` provides continuous packaging without an HTTP server. It keeps
the last complete output generation when compilation or config parsing fails,
prints the error, and retries on source, config, template, public-directory, or
declared-asset changes. Changes observed while a build is running are not
swallowed.

`check --diagnostic-format` supports `human`, `json`, `sarif`, and `lsp`.
`--color auto|always|never`, warning limits and warning-code allow/deny rules
apply without parsing human-readable diagnostics. `explain` reads the same
diagnostic registry. `languages --json`, `completions`, `man`, and `serve
--open` provide machine-readable discovery and shell/browser integration.

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
