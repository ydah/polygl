---
title: Releasing
permalink: /releasing/
---

# Releasing PolyGL

The release workflow publishes one coordinated version to crates.io, npm, and
GitHub Releases. `workspace.package.version` in `Cargo.toml` is authoritative.
A tag, every publishable Rust crate, all six npm packages, the native
executables, and the GitHub Release must use that exact version.

## One-time repository setup

Create the GitHub `release` environment in
**Settings → Environments → New environment**. Add required reviewers or a
deployment branch rule if desired. The irreversible crates.io, npm, and GitHub
Release jobs all use this environment, while validation and preflight do not.

Create a crates.io API token that can publish the PolyGL crates. Store it as the
`CARGO_REGISTRY_TOKEN` environment secret, not as a repository variable:

```console
gh secret set CARGO_REGISTRY_TOKEN --env release
```

The command reads the token without putting it in shell history. Cargo consumes
it through its standard `CARGO_REGISTRY_TOKEN` environment variable.

The npm packages use GitHub OIDC Trusted Publishing and do not use a stored npm
token. Trusted Publishing requires each package to exist first. For the first
release only, download the successful preflight artifact and use an interactive
npm account with 2FA to publish the exact verified package archives. Publish
the five native packages before the launcher:

```console
npm install --global npm@11.18.0
npm login
npm publish preflight/npm-packages/polygl-cli-darwin-arm64.tgz --access public --tag latest
npm publish preflight/npm-packages/polygl-cli-darwin-x64.tgz --access public --tag latest
npm publish preflight/npm-packages/polygl-cli-linux-arm64.tgz --access public --tag latest
npm publish preflight/npm-packages/polygl-cli-linux-x64.tgz --access public --tag latest
npm publish preflight/npm-packages/polygl-cli-win32-x64.tgz --access public --tag latest
npm publish preflight/npm-packages/polygl-cli.tgz --access public --tag latest
```

Use `--tag next` instead of `latest` for a prerelease. Do not rebuild or edit
these archives after preflight.

After all six package pages exist, configure the same trusted publisher on each
package. npm 11.18.0 supports the `npm trust` command:

```console
npm trust github @polygl/cli-darwin-arm64 --repo ydah/polygl --file release.yml --environment release --allow-publish --yes
npm trust github @polygl/cli-darwin-x64 --repo ydah/polygl --file release.yml --environment release --allow-publish --yes
npm trust github @polygl/cli-linux-arm64 --repo ydah/polygl --file release.yml --environment release --allow-publish --yes
npm trust github @polygl/cli-linux-x64 --repo ydah/polygl --file release.yml --environment release --allow-publish --yes
npm trust github @polygl/cli-win32-x64 --repo ydah/polygl --file release.yml --environment release --allow-publish --yes
npm trust github @polygl/cli --repo ydah/polygl --file release.yml --environment release --allow-publish --yes
```

These values are case-sensitive: repository `ydah/polygl`, workflow filename
`release.yml`, environment `release`, and allowed action `npm publish`. The npm
account must have 2FA enabled and write access to every package. Once OIDC is
confirmed on the next release, npm recommends disabling traditional token
publishing for each package.

## Prepare and run preflight

Update `workspace.package.version` and every exact internal dependency
requirement together. Regenerate `Cargo.lock` if needed and run the normal
quality gates plus the license freshness check:

```console
cargo check --locked --workspace
just test
just conformance
just gen-check
just licenses-check
node --test npm/cli/test/*.test.mjs
actionlint
```

Merge the release preparation commit to `main`. Run the release workflow
manually with the exact Cargo version:

```console
gh workflow run release.yml --ref main -f version=0.1.0
gh run list --workflow release.yml --limit 1
gh run watch RUN_ID --exit-status
gh run download RUN_ID --name release-preflight
```

`workflow_dispatch` is publish-free. It validates the version contract, builds
all five native targets, checks `polygl --version`, enforces macOS 11.0 and
glibc 2.39 compatibility floors, validates byte-reproducible archives and
legal files, installs each launcher/native tarball pair in an empty project,
builds and tests the complete Cargo workspace, assembles every
publishable `.crate`, runs the npm tests, and verifies the name, version, exact
file set, native executable, legal files, platform `os`/`cpu`, and launcher
`bin.polygl` mapping in every packed npm tarball. It retains checksummed
archives, npm dry-run reports, verified npm tarballs, Cargo package reports,
and Cargo archives for 14 days.

`polygl-span` has no unpublished PolyGL dependency, so preflight runs normal
Cargo package verification for it. The remaining crates have exact internal
registry dependencies. Normal `cargo package` verification removes their local
path source and must resolve those versions from crates.io, where they do not
exist before the release. Preflight therefore uses Cargo itself with
`--locked --offline --no-verify --exclude-lockfile` to assemble those `.crate`
files from dependencies already fetched by the workspace build, and records
`cargo package --list` output. The full workspace build and test validate the
same source through local path dependencies. During publication, plain
`cargo publish --locked` performs normal package verification after the
preceding dependency stage is visible on crates.io.

Inspect at least:

- `artifacts/release/SHA256SUMS` and every target archive;
- all six files under `preflight/npm-dry-run/`;
- all six tarballs under `preflight/npm-packages/`;
- all fourteen `.crate` files and their matching file-list reports under
  `preflight/cargo-packages/`.

## Publish

Only a `v<SemVer>` tag push can enter publication. The tagged commit must be
reachable from `origin/main`, and the tag version must exactly match Cargo:

```console
git switch main
git pull --ff-only
git tag -s v0.1.0 -m "v0.1.0"
git push origin v0.1.0
gh run list --workflow release.yml --limit 1
gh run watch RUN_ID --exit-status
```

After approval in the `release` environment, the workflow:

1. generates a CycloneDX 1.6 SBOM, signs its association with all five native
   archives through GitHub OIDC/Sigstore attestations, and publishes both the
   SBOM and checksums;
2. publishes Rust crates in the dependency stages from ADR 0029;
3. publishes five native npm packages, waits for them, then publishes the
   launcher with `next` for prereleases or `latest` for stable versions;
4. creates the GitHub Release only after both registries succeed, marking
   prereleases and attaching the verified archives, SBOM, subject checksums,
   and complete `SHA256SUMS`.

## Recover a partial release

Do not move, delete, or recreate the release tag, and do not reuse the version
for changed source. Inspect crates.io and npm to identify the last accepted
package, then rerun the failed jobs:

```console
gh run view RUN_ID --log-failed
gh run rerun RUN_ID --failed
gh run watch RUN_ID --exit-status
```

The registry scripts query each exact package version before publishing.
Already accepted crates and npm packages are skipped. Each dependency stage is
polled with a bounded wait before its dependants proceed. The GitHub Release
step also replaces attachments safely if a previous attempt created the
release but did not finish.

If an uploaded artifact has expired, rerun the complete tag workflow rather
than rebuilding locally. If registry contents differ from the tagged source,
stop: immutable registry versions cannot be repaired, so the source fix needs a
new coordinated version.

References:

- [npm Trusted Publishing](https://docs.npmjs.com/trusted-publishers/)
- [`npm trust`](https://docs.npmjs.com/cli/v11/commands/npm-trust/)
- [`cargo publish`](https://doc.rust-lang.org/cargo/commands/cargo-publish.html)
