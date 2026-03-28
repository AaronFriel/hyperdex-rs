#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
ZIZMOR_VERSION="${ZIZMOR_VERSION:-1.23.1}"
ACTIONLINT_BIN="${ACTIONLINT_BIN:-}"

cd "$REPO_ROOT"

if [[ ! -d .github/workflows ]]; then
  echo "No .github/workflows directory found; skipping workflow audit."
  exit 0
fi

ACTIONLINT_BIN="$(scripts/install-actionlint.sh)"

"$ACTIONLINT_BIN"

if ! command -v uv >/dev/null 2>&1; then
  echo "uv is required to run zizmor locally." >&2
  echo "Install uv or set up the CI environment first." >&2
  exit 1
fi

uv tool run --from "zizmor==${ZIZMOR_VERSION}" zizmor .github/workflows
