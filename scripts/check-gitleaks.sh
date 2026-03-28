#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
LOCAL_BIN="${LOCAL_BIN:-$HOME/.local/bin}"

cd "$REPO_ROOT"

export PATH="$LOCAL_BIN:$PATH"
LOCAL_BIN="$LOCAL_BIN" scripts/install-gitleaks.sh

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT
scanroot="$tmpdir/scan"
mkdir -p "$scanroot"

if git rev-parse --git-dir >/dev/null 2>&1; then
  mapfile -d '' -t paths < <(git diff --cached --name-only --diff-filter=ACMR -z)
  if [[ "${#paths[@]}" -gt 0 ]]; then
    for path in "${paths[@]}"; do
      mkdir -p "$scanroot/$(dirname "$path")"
      git show ":$path" >"$scanroot/$path"
    done
  else
    mapfile -d '' -t paths < <(git ls-files -z)
    for path in "${paths[@]}"; do
      [[ -f "$path" ]] || continue
      mkdir -p "$scanroot/$(dirname "$path")"
      cp -p -- "$path" "$scanroot/$path"
    done
  fi
else
  mapfile -d '' -t paths < <(find . -type f -not -path './target/*' -print0)
  for path in "${paths[@]}"; do
    rel_path="${path#./}"
    mkdir -p "$scanroot/$(dirname "$rel_path")"
    cp -p -- "$path" "$scanroot/$rel_path"
  done
fi

gitleaks dir "$scanroot" --no-banner --log-level error
