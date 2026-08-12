# Contributing to PolyGL

PolyGL keeps source-language adapters, shared IR, JavaScript/GLSL backends, and
the browser runtime under one compatibility contract. A change is complete
only when every affected layer and its conformance evidence agree.

## Development environment

The repository pins Rust 1.96.1, Node.js 24.14.1, pnpm 10.33.0, and the exact
dependency graph in lockfiles. The published npm launcher additionally
supports Node.js 20 and later; CI tests both Node 20.0.0 and the pinned
development version.

```console
corepack enable
pnpm --dir runtime install --frozen-lockfile
cargo test --locked --workspace --all-features
pnpm --dir runtime test
pnpm test:npm-cli
cargo xtask conformance
```

Browser framebuffer tests require the dependencies and Chromium described in
the README. `scripts/check-reproducible-build.sh` performs the same comparison
of two clean executable builds used by CI.

## Generated files

`cargo xtask gen-runtime` regenerates the runtime operation table, runtime ABI
constant, and API reference from the canonical Rust builtin registry. Commit
all resulting changes, including the embedded runtime bundle after running
`pnpm --dir runtime build`. Use these read-only gates before submitting:

```console
cargo xtask gen-runtime --check
pnpm --dir runtime test
just licenses-check
```

Do not hand-edit files marked `@generated` or
`crates/polygl-cli/assets/runtime.js`. Third-party license output is generated
with the pinned `cargo-about` version recorded in the justfile and CI.

## Test layers

- Rust unit and integration tests cover parsing, typing, IR validation,
  splitting, code generation, CLI publication, and diagnostics.
- Runtime tests execute the bundled JavaScript against a hostile fake WebGL2
  boundary and verify lifecycle and resource cleanup.
- `conformance/cases.json` is the single inventory for neutral-HIR and browser
  cases. A new versioned `FeatureTag` must have a listed conformance case.
- Browser tests compare Ruby, PHP, and Perl framebuffers under pinned Chromium
  and SwiftShader.

Prefer the smallest regression test that fails before a fix. Run the full
workspace and runtime suites for changes crossing a compiler/runtime boundary.

## Architecture and adapters

Add an ADR under `docs/decisions/` when changing a public semantic contract,
compatibility boundary, artifact format, or lifecycle policy. Advance the
appropriate adapter API, HIR schema, builtin schema, or runtime ABI version
when old and new artifacts can no longer interoperate safely.

For a new source language, start with `polygl new-adapter`, implement the
canonical adapter API and versioned capability set, then add typed diagnostics,
neutral-HIR cases, snapshots, and browser evidence. Follow
`docs/adapter-guide.md`; do not reproduce source-language behavior outside the
documented Common Core by accident.
