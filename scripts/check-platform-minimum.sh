#!/usr/bin/env bash
set -euo pipefail

binary="${1:?binary path is required}"
platform="${2:?platform is required}"

case "$platform" in
  linux)
    maximum="${POLYGL_MAX_GLIBC:-2.39}"
    required="$({
      readelf --version-info "$binary" \
        | sed -n 's/.*Name: GLIBC_\([0-9][0-9.]*\).*/\1/p'
    } | sort -Vu | tail -1)"
    test -n "$required"
    if [[ "$(printf '%s\n%s\n' "$maximum" "$required" | sort -V | tail -1)" != "$maximum" ]]; then
      echo "$binary requires glibc $required, newer than supported $maximum" >&2
      exit 1
    fi
    echo "$binary requires glibc $required (maximum supported requirement $maximum)"
    ;;
  macos)
    maximum="${POLYGL_MAX_MACOS:-11.0}"
    required="$(otool -l "$binary" | awk '$1 == "minos" { print $2; exit }')"
    test -n "$required"
    if [[ "$(printf '%s\n%s\n' "$maximum" "$required" | sort -V | tail -1)" != "$maximum" ]]; then
      echo "$binary requires macOS $required, newer than supported $maximum" >&2
      exit 1
    fi
    echo "$binary requires macOS $required (maximum supported requirement $maximum)"
    ;;
  windows)
    file "$binary" | grep -Eq 'PE32\+?.*Windows'
    echo "$binary is a Windows PE executable"
    ;;
  *)
    echo "unsupported platform $platform" >&2
    exit 2
    ;;
esac
