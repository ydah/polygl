#!/usr/bin/env bash
set -euo pipefail

workspace="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
scratch="$(mktemp -d "${TMPDIR:-/tmp}/polygl-size.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT

cargo build --locked --release --manifest-path "$workspace/Cargo.toml" -p polygl-cli
binary="$workspace/target/release/polygl"
if [[ "${OS:-}" == "Windows_NT" ]]; then
  binary="$workspace/target/release/polygl.exe"
fi

artifacts=(runtime.js app.js shaders.js)
budgets=(190000 25000 30000)
largest=(0 0 0)

for source in \
  "$workspace/examples/triangle.rb" \
  "$workspace/examples/terrain.rb" \
  "$workspace/examples/plasma.rb"
do
  name="$(basename "$source" | tr '.' '-')"
  output="$scratch/$name"
  "$binary" build "$source" --release -o "$output"
  for index in "${!artifacts[@]}"; do
    artifact="${artifacts[$index]}"
    bytes="$(wc -c <"$output/$artifact" | tr -d ' ')"
    if ((bytes > largest[$index])); then
      largest[$index]="$bytes"
    fi
  done
done

for index in "${!artifacts[@]}"; do
  artifact="${artifacts[$index]}"
  bytes="${largest[$index]}"
  budget="${budgets[$index]}"
  printf '%-12s %8d / %8d bytes\n' "$artifact" "$bytes" "$budget"
  if ((bytes > budget)); then
    echo "$artifact exceeds its release size budget" >&2
    exit 1
  fi
done
