---
---

# 0030: Generate release license notices with cargo-about

- Status: Accepted
- Date: 2026-07-26

## Context

PolyGL distributes the same Rust command-line executable through GitHub
Release archives and five npm native packages. Those binary distributions must
carry PolyGL's two first-party license texts and the applicable third-party
license texts. The dependency closure varies by operating system, and
development-only dependencies are not linked into the released executable.

Maintaining a handwritten dependency list would duplicate Cargo's resolver,
miss target-specific dependencies, and drift when `Cargo.lock` changes. A
license policy also needs to reject a new dependency whose terms have not been
reviewed.

## Decision

Use `cargo-about` 0.9.1 to resolve and render third-party notices. The exact
tool version is installed with Cargo's published lock file:

```console
cargo install --locked --version 0.9.1 --features cli cargo-about
```

`about.toml` evaluates the `polygl-cli` dependency graph for the five release
target triples, excludes dependencies used only for development, and accepts
only the permissive SPDX licenses currently present in that graph. Build
dependencies remain included because their code or generated output can affect
the distributed executable.

cargo-about 0.9.1 does not provide a configuration option that excludes
publishable workspace members while retaining their dependencies. The
generated report therefore covers the complete resolved crate closure and
includes publishable PolyGL workspace crates as well as third-party crates.
Filtering workspace crate names after generation would duplicate Cargo
metadata policy and could silently become stale, so the report keeps those
entries. `LICENSE-MIT` and `LICENSE-APACHE` remain the authoritative license
texts for PolyGL itself.

`about.hbs` renders the selected crate versions and license texts into the
committed `THIRD_PARTY_LICENSES.txt`. The filename identifies its distribution
role alongside the first-party license files, while its content documents the
complete crate closure. The generation command normalizes line
endings and trailing horizontal whitespace so upstream text-file conventions
do not create platform-dependent diffs. Run `just licenses` after changing
`Cargo.lock`, release targets, or the license configuration. Review the
dependency and license diff rather than editing the generated file by hand,
then run `just licenses-check`. CI performs the same freshness comparison and
fails if the committed notice differs from cargo-about's output.

Release preparation copies `LICENSE-MIT`, `LICENSE-APACHE`, and
`THIRD_PARTY_LICENSES.txt` into the npm launcher, every native npm package, and
every GitHub Release archive. `.gitattributes` fixes all three files to LF so
archives built on Windows and Unix contain byte-identical legal texts.

## Consequences

Both distribution channels contain the same legal files, and the full
target-specific crate closure is covered by one reproducible report. The
report's workspace entries intentionally duplicate first-party identification,
while the separate PolyGL license files state the authoritative first-party
terms. A new license expression fails generation until it is deliberately
reviewed and added to the accepted set.

Generating notices requires the pinned cargo-about release and access to the
locked Cargo dependency sources. Updating cargo-about is an explicit
maintenance change: update the pinned version in commands and CI, regenerate
the report, and review any output or policy changes before merging.
