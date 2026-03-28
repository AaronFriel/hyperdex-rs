#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
PREK_VERSION="${PREK_VERSION:-0.3.8}"
PREK_SHA256_X86_64_UNKNOWN_LINUX_GNU="${PREK_SHA256_X86_64_UNKNOWN_LINUX_GNU:-80ec6adb9f1883344de52cb943d371ecfd25340c4a6b5b81e2600d27e246cfa1}"
PREK_ARCHIVE_ENTRY="${PREK_ARCHIVE_ENTRY:-prek-x86_64-unknown-linux-gnu/prek}"
LOCAL_BIN="${LOCAL_BIN:-$HOME/.local/bin}"

cd "$REPO_ROOT"

if command -v prek >/dev/null 2>&1 && prek --version 2>/dev/null | grep -q "$PREK_VERSION"; then
  exit 0
fi

mkdir -p "$LOCAL_BIN"
archive="$(mktemp -t prek.XXXXXX.tar.gz)"
trap 'rm -f "$archive"' EXIT
curl -L --fail --silent --show-error \
  "https://github.com/j178/prek/releases/download/v${PREK_VERSION}/prek-x86_64-unknown-linux-gnu.tar.gz" \
  -o "$archive"
printf '%s  %s\n' "$PREK_SHA256_X86_64_UNKNOWN_LINUX_GNU" "$archive" | sha256sum --check --status
tar -xzf "$archive" -C "$LOCAL_BIN" --strip-components=1 "$PREK_ARCHIVE_ENTRY"
chmod +x "${LOCAL_BIN}/prek"
