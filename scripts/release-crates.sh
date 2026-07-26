#!/usr/bin/env bash
set -euo pipefail

stages=()
stages_file="${RELEASE_CRATE_STAGES_FILE:-scripts/release-crate-stages.txt}"
cargo_command="${RELEASE_CARGO_COMMAND:-cargo}"
curl_command="${RELEASE_CURL_COMMAND:-curl}"
sleep_command="${RELEASE_SLEEP_COMMAND:-sleep}"
while IFS= read -r stage; do
  [[ -n "$stage" ]] && stages+=("$stage")
done <"$stages_file"

registry_status() {
  local crate="$1"
  local version="$2"
  local status
  if ! status="$(
    "$curl_command" --silent --show-error \
      --retry 4 --retry-all-errors \
      --connect-timeout 10 --max-time 30 \
      --output /dev/null --write-out '%{http_code}' \
      --user-agent "polygl-release-workflow/${GITHUB_RUN_ID:-local}" \
      "https://crates.io/api/v1/crates/${crate}/${version}"
  )"; then
    echo "failed to query crates.io for ${crate} ${version}" >&2
    return 2
  fi
  case "$status" in
    200) return 0 ;;
    404) return 1 ;;
    *)
      echo "crates.io returned HTTP ${status} for ${crate} ${version}" >&2
      return 2
      ;;
  esac
}

wait_for_stage() {
  local version="$1"
  shift
  local attempts="${CRATES_IO_POLL_ATTEMPTS:-30}"
  local interval="${CRATES_IO_POLL_INTERVAL_SECONDS:-10}"
  for ((attempt = 1; attempt <= attempts; attempt++)); do
    local waiting=0
    for crate in "$@"; do
      local status
      if registry_status "$crate" "$version"; then
        continue
      else
        status=$?
      fi
      if ((status == 1)); then
        waiting=1
      else
        return "$status"
      fi
    done
    if ((waiting == 0)); then
      return
    fi
    if ((attempt < attempts)); then
      echo "waiting for crates.io index (${attempt}/${attempts})"
      "$sleep_command" "$interval"
    fi
  done
  echo "published stage did not become visible on crates.io" >&2
  return 1
}

package_crates() {
  local output_root="$1"
  local version="$2"
  mkdir -p "$output_root"
  for stage in "${stages[@]}"; do
    read -r -a crates <<<"$stage"
    for crate in "${crates[@]}"; do
      "$cargo_command" package --locked --offline --list -p "$crate" \
        >"${output_root}/${crate}.txt"
      if [[ "$crate" == "polygl-span" ]]; then
        "$cargo_command" package --locked --offline -p "$crate"
      else
        "$cargo_command" package \
          --locked --offline --no-verify --exclude-lockfile -p "$crate"
      fi
      cp "target/package/${crate}-${version}.crate" "$output_root/"
    done
  done
}

publish_crates() {
  local version="$1"
  for stage in "${stages[@]}"; do
    read -r -a crates <<<"$stage"
    for crate in "${crates[@]}"; do
      local status
      if registry_status "$crate" "$version"; then
        echo "${crate} ${version} is already published; skipping"
        continue
      else
        status=$?
      fi
      if ((status != 1)); then
        return "$status"
      fi
      "$cargo_command" publish --locked -p "$crate"
    done
    wait_for_stage "$version" "${crates[@]}"
  done
}

case "${1:-}" in
  package)
    package_crates \
      "${2:?output directory is required}" \
      "${3:?release version is required}"
    ;;
  publish)
    publish_crates "${2:?release version is required}"
    ;;
  *)
    echo "usage: $0 package <output-directory> <version> | publish <version>" >&2
    exit 2
    ;;
esac
