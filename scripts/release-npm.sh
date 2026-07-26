#!/usr/bin/env bash
set -euo pipefail

platforms=(
  "@polygl/cli-darwin-arm64:polygl-cli-darwin-arm64"
  "@polygl/cli-darwin-x64:polygl-cli-darwin-x64"
  "@polygl/cli-linux-arm64:polygl-cli-linux-arm64"
  "@polygl/cli-linux-x64:polygl-cli-linux-x64"
  "@polygl/cli-win32-x64:polygl-cli-win32-x64"
)

version="${1:?release version is required}"
dist_tag="${2:?npm dist-tag is required}"
packages_root="${3:?packed package directory is required}"
curl_command="${RELEASE_CURL_COMMAND:-curl}"
npm_command="${RELEASE_NPM_COMMAND:-npm}"
sleep_command="${RELEASE_SLEEP_COMMAND:-sleep}"

registry_status() {
  local package="$1"
  local encoded_package="${package/\//%2F}"
  local encoded_version="${version//+/%2B}"
  local status
  if ! status="$(
    "$curl_command" --silent --show-error \
      --retry 4 --retry-all-errors \
      --connect-timeout 10 --max-time 30 \
      --output /dev/null --write-out '%{http_code}' \
      --user-agent "polygl-release-workflow/${GITHUB_RUN_ID:-local}" \
      "https://registry.npmjs.org/${encoded_package}/${encoded_version}"
  )"; then
    echo "failed to query npm for ${package} ${version}" >&2
    return 2
  fi
  case "$status" in
    200) return 0 ;;
    404) return 1 ;;
    *)
      echo "npm returned HTTP ${status} for ${package} ${version}" >&2
      return 2
      ;;
  esac
}

publish_package() {
  local package="$1"
  local archive_stem="$2"
  local archive="${packages_root}/${archive_stem}.tgz"
  local status
  if registry_status "$package"; then
    echo "${package} ${version} is already published; skipping"
    return
  else
    status=$?
  fi
  if ((status != 1)); then
    return "$status"
  fi
  test -f "$archive"
  "$npm_command" publish "$archive" \
    --access public --provenance --tag "$dist_tag"
}

wait_for_platforms() {
  local attempts="${NPM_POLL_ATTEMPTS:-30}"
  local interval="${NPM_POLL_INTERVAL_SECONDS:-10}"
  for ((attempt = 1; attempt <= attempts; attempt++)); do
    local waiting=0
    for entry in "${platforms[@]}"; do
      local status
      if registry_status "${entry%%:*}"; then
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
      echo "waiting for npm registry (${attempt}/${attempts})"
      "$sleep_command" "$interval"
    fi
  done
  echo "native npm packages did not become visible" >&2
  return 1
}

for entry in "${platforms[@]}"; do
  publish_package "${entry%%:*}" "${entry#*:}"
done
wait_for_platforms
publish_package "@polygl/cli" "polygl-cli"
