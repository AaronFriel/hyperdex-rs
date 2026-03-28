#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
ACTIONLINT_VERSION="${ACTIONLINT_VERSION:-1.7.11}"
ZIZMOR_VERSION="${ZIZMOR_VERSION:-1.23.1}"
CACHE_ROOT="${XDG_CACHE_HOME:-$HOME/.cache}/hyperdex-rs/tools"
ACTIONLINT_ROOT="${CACHE_ROOT}/actionlint/${ACTIONLINT_VERSION}"
ACTIONLINT_BIN="${ACTIONLINT_BIN:-}"

ensure_actionlint() {
  if [[ -n "$ACTIONLINT_BIN" ]]; then
    printf '%s\n' "$ACTIONLINT_BIN"
    return 0
  fi

  if command -v actionlint >/dev/null 2>&1; then
    command -v actionlint
    return 0
  fi

  mkdir -p "$ACTIONLINT_ROOT"

  if [[ ! -x "${ACTIONLINT_ROOT}/actionlint" ]]; then
    local archive
    archive="$(mktemp -t actionlint.XXXXXX.tar.gz)"
    trap 'rm -f "$archive"' RETURN
    curl -L --fail --silent --show-error \
      "https://github.com/rhysd/actionlint/releases/download/v${ACTIONLINT_VERSION}/actionlint_${ACTIONLINT_VERSION}_linux_amd64.tar.gz" \
      -o "$archive"
    tar -xzf "$archive" -C "$ACTIONLINT_ROOT" actionlint
  fi

  printf '%s\n' "${ACTIONLINT_ROOT}/actionlint"
}

cd "$REPO_ROOT"

if [[ ! -d .github/workflows ]]; then
  echo "No .github/workflows directory found; skipping workflow audit."
  exit 0
fi

ACTIONLINT_BIN="$(ensure_actionlint)"

"$ACTIONLINT_BIN"

if ! command -v uv >/dev/null 2>&1; then
  echo "uv is required to run zizmor locally." >&2
  echo "Install uv or set up the CI environment first." >&2
  exit 1
fi

uv tool run --from "zizmor==${ZIZMOR_VERSION}" zizmor .github/workflows
