---
---

# 0029: Version Rust crates for coordinated publication

- Status: Accepted
- Date: 2026-07-26

## Context

PolyGL publishes a native CLI through Cargo, npm, and GitHub Releases. The Rust
workspace previously used a placeholder version and path-only dependencies.
Cargo removes path information when publishing, so a registry version
requirement is also necessary for every dependency between publishable crates.
Without those requirements, the crates cannot be packaged or installed from a
registry.

The CLI also generates standalone adapter crates. Their PolyGL dependency
versions must be compatible with the CLI that generated them, rather than a
separately maintained value.

## Decision

`workspace.package.version` in the root `Cargo.toml` is the source of truth for
the coordinated PolyGL release version. Every publishable workspace crate
inherits it. Internal path dependencies in publishable crates include the exact
same registry requirement, including development dependencies:

```toml
polygl-span = { path = "../polygl-span", version = "=0.1.0" }
```

The `polygl` executable reports this value through `polygl --version`, and
`polygl new-adapter` uses `CARGO_PKG_VERSION` for generated PolyGL dependencies.
Release tags, GitHub Release metadata, and npm package versions must match the
committed workspace version; a release must fail before publication when they
do not match.

Rust crates are published in these dependency-ordered stages:

1. `polygl-span`;
2. `polygl-hir`;
3. `polygl-adapter-api` and `polygl-adapter-treesitter-util`;
4. `polygl-builtins`;
5. `polygl-core` and `polygl-types`;
6. `polygl-lir`;
7. `polygl-adapter-perl`, `polygl-adapter-php`, and `polygl-adapter-ruby`;
8. `polygl-backend-js` and `polygl-backend-glsl`;
9. `polygl-cli`.

Crates within one stage may be published in parallel after the preceding stage
is available from the registry. Development-only dependency edges do not affect
publication order. The conformance runner, `xtask`, and spike crates remain
unpublished.

## Consequences

One version update coordinates every public Rust crate and every distribution
channel. Exact internal requirements prevent a release from resolving a mixed
set of PolyGL crate versions and make generated adapters reproducible.

Publishing requires multiple registry operations in dependency order. A
partially completed publication can be resumed at the same version for crates
that have not yet been accepted, but an accepted crate version remains
immutable. Changes intended for a later release therefore require a new
workspace version.
