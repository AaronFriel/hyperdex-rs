#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
ACTIONLINT_VERSION="${ACTIONLINT_VERSION:-1.7.11}"
ACTIONLINT_SHA256_LINUX_AMD64="${ACTIONLINT_SHA256_LINUX_AMD64:-900919a84f2229bac68ca9cd4103ea297abc35e9689ebb842c6e34a3d1b01b0a}"
ACTIONLINT_BIN="${ACTIONLINT_BIN:-}"
CACHE_ROOT="${XDG_CACHE_HOME:-$HOME/.cache}/hyperdex-rs/tools"
ACTIONLINT_ROOT="${CACHE_ROOT}/actionlint/${ACTIONLINT_VERSION}"

cd "$REPO_ROOT"

if [[ -n "$ACTIONLINT_BIN" ]]; then
  printf '%s\n' "$ACTIONLINT_BIN"
  exit 0
fi

if command -v actionlint >/dev/null 2>&1; then
  command -v actionlint
  exit 0
fi

mkdir -p "$ACTIONLINT_ROOT"

if [[ ! -x "${ACTIONLINT_ROOT}/actionlint" ]]; then
  archive="$(mktemp -t actionlint.XXXXXX.tar.gz)"
  trap 'rm -f "$archive"' EXIT
  curl -L --fail --silent --show-error \
    "https://github.com/rhysd/actionlint/releases/download/v${ACTIONLINT_VERSION}/actionlint_${ACTIONLINT_VERSION}_linux_amd64.tar.gz" \
    -o "$archive"
  printf '%s  %s\n' "$ACTIONLINT_SHA256_LINUX_AMD64" "$archive" | sha256sum --check --status
  tar -xzf "$archive" -C "$ACTIONLINT_ROOT" actionlint
fi

printf '%s\n' "${ACTIONLINT_ROOT}/actionlint"
