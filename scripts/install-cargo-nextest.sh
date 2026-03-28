#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
NEXTTEST_VERSION="${NEXTTEST_VERSION:-0.9.101}"
NEXTTEST_SHA256_X86_64_UNKNOWN_LINUX_GNU="${NEXTTEST_SHA256_X86_64_UNKNOWN_LINUX_GNU:-acf6c0afd44d2785d1b607ec47fe002fadbd60f4dfdc4ad8a5650a65a0b14b6b}"
LOCAL_BIN="${LOCAL_BIN:-$HOME/.local/bin}"

cd "$REPO_ROOT"

if command -v cargo-nextest >/dev/null 2>&1 && cargo-nextest --version 2>/dev/null | grep -q "$NEXTTEST_VERSION"; then
  exit 0
fi

mkdir -p "$LOCAL_BIN"
archive="$(mktemp -t cargo-nextest.XXXXXX.tar.gz)"
trap 'rm -f "$archive"' EXIT
curl -L --fail --silent --show-error \
  "https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-${NEXTTEST_VERSION}/cargo-nextest-${NEXTTEST_VERSION}-x86_64-unknown-linux-gnu.tar.gz" \
  -o "$archive"
printf '%s  %s\n' "$NEXTTEST_SHA256_X86_64_UNKNOWN_LINUX_GNU" "$archive" | sha256sum --check --status
tar -xzf "$archive" -C "$LOCAL_BIN" cargo-nextest
chmod +x "${LOCAL_BIN}/cargo-nextest"
