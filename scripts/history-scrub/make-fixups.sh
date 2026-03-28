#!/usr/bin/env bash
set -euo pipefail

if ! command -v git-absorb >/dev/null 2>&1; then
  echo "git-absorb is not installed" >&2
  exit 1
fi

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

if git diff --quiet && git diff --cached --quiet; then
  echo "working tree is already clean; no fixups to create" >&2
  exit 0
fi

git-absorb "$@"
