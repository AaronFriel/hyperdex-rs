#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
GITLEAKS_VERSION="${GITLEAKS_VERSION:-8.30.1}"
GITLEAKS_SHA256_LINUX_X64="${GITLEAKS_SHA256_LINUX_X64:-551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb}"
LOCAL_BIN="${LOCAL_BIN:-$HOME/.local/bin}"

cd "$REPO_ROOT"

if command -v gitleaks >/dev/null 2>&1 && gitleaks version 2>/dev/null | grep -q "$GITLEAKS_VERSION"; then
  exit 0
fi

mkdir -p "$LOCAL_BIN"
archive="$(mktemp -t gitleaks.XXXXXX.tar.gz)"
trap 'rm -f "$archive"' EXIT
curl -L --fail --silent --show-error \
  "https://github.com/gitleaks/gitleaks/releases/download/v${GITLEAKS_VERSION}/gitleaks_${GITLEAKS_VERSION}_linux_x64.tar.gz" \
  -o "$archive"
printf '%s  %s\n' "$GITLEAKS_SHA256_LINUX_X64" "$archive" | sha256sum --check --status
tar -xzf "$archive" -C "$LOCAL_BIN" gitleaks
chmod +x "${LOCAL_BIN}/gitleaks"
