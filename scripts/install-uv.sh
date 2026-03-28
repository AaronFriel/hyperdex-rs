#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
UV_VERSION="${UV_VERSION:-0.6.6}"
UV_SHA256_X86_64_UNKNOWN_LINUX_GNU="${UV_SHA256_X86_64_UNKNOWN_LINUX_GNU:-4c3426c4919d9f44633ab9884827fa1ad64ad8d993516d636eb955a3835c4a8c}"
UV_INSTALL_DIR="${UV_INSTALL_DIR:-$HOME/.local/bin}"

cd "$REPO_ROOT"

if command -v uv >/dev/null 2>&1 && uv --version 2>/dev/null | grep -q "$UV_VERSION"; then
  exit 0
fi

mkdir -p "$UV_INSTALL_DIR"
archive="$(mktemp -t uv.XXXXXX.tar.gz)"
trap 'rm -f "$archive"' EXIT
curl -L --fail --silent --show-error \
  "https://github.com/astral-sh/uv/releases/download/${UV_VERSION}/uv-x86_64-unknown-linux-gnu.tar.gz" \
  -o "$archive"
printf '%s  %s\n' "$UV_SHA256_X86_64_UNKNOWN_LINUX_GNU" "$archive" | sha256sum --check --status
tar -xzf "$archive" -C "$UV_INSTALL_DIR" uv uvx
chmod +x "${UV_INSTALL_DIR}/uv" "${UV_INSTALL_DIR}/uvx"
