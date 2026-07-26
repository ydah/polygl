---
---

# 0028: Publish native CLI packages from one build matrix

- Status: Accepted
- Date: 2026-07-25

## Context

PolyGL needs a low-friction installation path for users who do not have a Rust
toolchain. GitHub Releases must provide inspectable native archives, while npm
users expect a normal global package installation. Downloading an executable
from a registry lifecycle script makes installation dependent on a second
service, complicates proxies and offline caches, and hides the executable from
the package integrity boundary.

The compiler embeds its runtime, so the native executable is the complete
platform-dependent unit. Cross-compiling all targets from one host would add
linker and C-library risk, especially for macOS and Windows.

## Decision

A tag matching `v<semver>` starts one GitHub Actions matrix with native runners
for Linux x64/arm64, macOS x64/arm64, and Windows x64. Every runner builds the
locked `polygl-cli` package, executes its help command, and stages one immutable
binary. GitHub Release archives and npm platform packages consume those same
uploaded artifacts. Release archives include target triples and are covered by
one SHA-256 checksum file.

The npm launcher is `@polygl/cli`. It declares five exact-version optional
dependencies, one per supported platform:

- `@polygl/cli-linux-x64`
- `@polygl/cli-linux-arm64`
- `@polygl/cli-darwin-x64`
- `@polygl/cli-darwin-arm64`
- `@polygl/cli-win32-x64`

Each platform package constrains npm's `os` and `cpu` fields and contains only
its executable and metadata. The launcher selects a package from
`process.platform` and `process.arch`, resolves the executable through Node's
package resolver, and forwards arguments, standard streams, and the exit
status. Unsupported pairs and omitted optional dependencies produce actionable
errors. There is no install lifecycle script and no install-time network
request outside npm itself.

The tag without its `v` prefix is the sole release version. A preparation
script validates it as SemVer, updates all six manifests, verifies package-name
alignment, and copies the matrix artifacts before publication. Platform
packages are published before the launcher.

## Consequences

Users can verify a GitHub archive or rely on npm's package integrity without
running arbitrary installation code. Native runner builds reduce toolchain
variation, and both distribution channels expose byte-identical executables.
Adding a platform requires coordinated changes to the build matrix, staging
table, launcher table, optional dependencies, and tests; an alignment test
fails when those lists diverge.

The release requires npm Trusted Publisher configuration and five runner
architectures. Publication can partially complete if npm fails after some
platform packages are accepted, so releases must never reuse a version.
ADR 0031 adds a publish-free preflight, protected publication jobs, and
idempotent recovery. Platforms outside the explicit matrix must build from
source until a tested native runner and package are added.
