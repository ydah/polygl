# PolyGL npm distribution

`@polygl/cli` is a small JavaScript launcher. Its optional dependencies contain
the native `polygl` executable for each supported operating-system and CPU
pair. Installation never downloads an executable from a lifecycle script.

Release preparation copies binaries produced by the native-runner matrix into
the platform packages, synchronizes every package version, and copies
`LICENSE-MIT`, `LICENSE-APACHE`, and `THIRD_PARTY_LICENSES.txt` into all six
packages. Every package manifest explicitly includes those legal files.
Publish the platform packages before publishing `@polygl/cli`.
The release workflow uses npm OIDC Trusted Publishing and publishes only after
the five-target preflight succeeds. See the
[release guide](../docs/releasing.md) for first-package bootstrap, trusted
publisher configuration, and partial-release recovery.

The committed `bin/.gitkeep` files only preserve empty staging directories.
They are removed from release packages when `scripts/prepare-release.mjs`
copies the native executables.

The dependency notice is generated from the complete locked `polygl-cli` crate
closure for all five release targets. It includes publishable PolyGL workspace
crates as well as third-party crates; `LICENSE-MIT` and `LICENSE-APACHE` remain
authoritative for PolyGL itself. Install the pinned generator and verify the
committed output before release:

```console
cargo install --locked --version 0.9.1 --features cli cargo-about
just licenses-check
```

After a dependency or target change, run `just licenses`, review the generated
license diff, and then run the freshness check. Do not edit
`THIRD_PARTY_LICENSES.txt` by hand.
