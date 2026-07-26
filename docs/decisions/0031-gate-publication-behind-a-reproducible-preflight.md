---
---

# 0031: Gate publication behind a reproducible preflight

- Status: Accepted
- Date: 2026-07-26

## Context

PolyGL coordinates fourteen Rust crates, six npm packages, five native
archives, and one GitHub Release. Registry versions are immutable, and a
multi-package publish can stop after only some packages have been accepted.
The previous tag-only workflow built artifacts and immediately began
publication. It repeated release-version logic in matrix jobs, relied on a
long-lived npm token, and could create the GitHub Release before npm had
succeeded.

Maintainers also need to exercise the complete cross-platform packaging path
before creating an irreversible tag.

## Decision

The release workflow has one validation boundary and one preflight boundary.
Validation derives the candidate version from either a manual input or a
`v<SemVer>` tag. It requires exact equality with
`workspace.package.version`, verifies every publishable workspace member has
that version, and rejects a release tag whose commit is not reachable from
`origin/main`. It exposes the version, prerelease state, and npm distribution
tag to all later jobs.

Both manual and tag events build the five native targets. Each runner executes
`polygl --version` and stages one binary into both the npm bundle and a
legal-complete archive. Preflight consumes the full matrix once, validates
archive names and contents, generates checksums, prepares and tests all npm
packages, records npm pack inspections, and then opens every tarball to verify
its identity, version, exact file set, native executable, legal files, native
`os`/`cpu` constraints, and launcher `bin.polygl` mapping. It also builds and
tests the complete Cargo workspace and asks Cargo to assemble all fourteen
publishable `.crate` files with matching file-list reports.
The verified payloads are retained as one inspectable artifact. A manual
`workflow_dispatch` ends successfully here and has no path to a publishing
job.

Normal Cargo package verification resolves registry dependencies after
removing local path information. Before a coordinated release, only
`polygl-span` can pass that verification because every later stage has an
exact-version dependency on an unpublished PolyGL crate. Preflight normally
verifies `polygl-span`; for the remaining crates it uses
`cargo package --locked --offline --no-verify --exclude-lockfile` to produce
Cargo's real archive from the dependency cache populated by the workspace build
without trying to resolve a registry lockfile. The full workspace build and
test cover compilation through local path dependencies. Publication still uses
plain `cargo publish --locked`, so Cargo verifies each package after the
preceding stage becomes available from crates.io.

Only a tag push may run irreversible jobs. Those jobs share a non-cancelling
release concurrency group and the protected `release` GitHub environment.
crates.io uses `CARGO_REGISTRY_TOKEN` and the dependency stages from ADR 0029.
Each exact crate version is checked before upload and every stage is polled
with a bounded wait before dependants proceed. npm uses GitHub OIDC Trusted
Publishing with npm 11.18.0; it publishes the five platform tarballs before the
launcher, using `next` for prereleases and `latest` for stable versions. It
also checks exact versions before upload. These checks make a rerun resume a
partially accepted version without attempting to overwrite immutable entries.

Every external GitHub Action is an official `actions/*` action pinned to a full
commit SHA with its release version documented inline. The Cargo credential is
available only to the publish step. The npm job has only read-only repository
access and `id-token: write`, runs only on a GitHub-hosted runner, and exposes
OIDC only alongside SHA-pinned official actions and the repository's verified
publication script.

The GitHub Release depends on successful completion of both registries. It
publishes the checksummed archives already accepted by preflight and marks
prerelease versions explicitly. A rerun replaces release attachments instead
of failing solely because the release record already exists.

## Consequences

Maintainers can verify every build and package payload without credentials or
publication by dispatching the workflow. A release tag remains a deliberate,
irreversible operation, but environment protection adds a final approval
boundary and concurrency prevents two attempts for the same ref from
overlapping.

Publication takes longer because registry propagation is observed between
dependency stages, and first-time setup is required in crates.io, npm, and the
GitHub environment. npm Trusted Publishing can only be configured after a
package exists, so the first six npm package versions require a documented
interactive bootstrap from the retained preflight tarballs. Later npm
publishes use short-lived OIDC credentials and carry automatic provenance.

Registry acceptance remains irreversible. Idempotent reruns recover from
partial completion; they do not make it safe to alter a tagged version.
