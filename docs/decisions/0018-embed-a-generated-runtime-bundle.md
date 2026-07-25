# 0018: Embed a generated runtime bundle

- Status: Accepted
- Date: 2026-07-25

## Context

The browser runtime is authored as several strict TypeScript modules, while a
CLI build must be self-contained and write a single `runtime.js` without
requiring Node.js on the user's machine. Invoking the TypeScript toolchain from
Cargo would also make ordinary Rust builds depend on pnpm and installed
JavaScript dependencies.

## Decision

The runtime build compiles TypeScript to ES2020 modules and concatenates those
internal modules into one generated ESM asset. The CLI embeds that committed
asset at Rust compile time and copies it into every browser build. The runtime
test command regenerates the bundle in memory and fails when the committed
asset is stale.

The bundler handles only this closed set of runtime-owned modules. It removes
their relative imports and re-exports while retaining public declaration
exports; it is not a general JavaScript bundler.

## Consequences

Installed CLI binaries need neither Node.js nor a separate runtime package.
Rust-only CI jobs remain independent of the JavaScript toolchain, while runtime
CI protects the embedded copy from drift. Adding an external runtime dependency
will require replacing this closed-set bundler with a dependency-aware bundler
or vendoring policy.
