#!/usr/bin/env bash
set -euo pipefail

workspace="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
scratch="$(mktemp -d "${TMPDIR:-/tmp}/polygl-reproducible.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT

target_directory="$scratch/target"
build_once() {
  CARGO_TARGET_DIR="$target_directory" cargo build \
    --locked \
    --release \
    -p polygl-cli \
    --manifest-path "$workspace/Cargo.toml"
}

binary="polygl"
if [[ "${OS:-}" == "Windows_NT" ]]; then
  binary="polygl.exe"
fi

build_once
first="$scratch/first-$binary"
cp "$target_directory/release/$binary" "$first"

cargo clean \
  --manifest-path "$workspace/Cargo.toml" \
  --target-dir "$target_directory"
build_once
second="$target_directory/release/$binary"
if ! cmp -s "$first" "$second"; then
  echo "release executable differs between independent clean builds" >&2
  exit 1
fi

echo "reproducible release executable: $(sha256sum "$first" | cut -d ' ' -f 1)"
