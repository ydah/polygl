# PolyGL npm distribution

`@polygl/cli` is a small JavaScript launcher. Its optional dependencies contain
the native `polygl` executable for each supported operating-system and CPU
pair. Installation never downloads an executable from a lifecycle script.

Release preparation copies binaries produced by the native-runner matrix into
the platform packages and synchronizes every package version. Publish the
platform packages before publishing `@polygl/cli`.

The committed `bin/.gitkeep` files only preserve empty staging directories.
They are removed from release packages when `scripts/prepare-release.mjs`
copies the native executables.
