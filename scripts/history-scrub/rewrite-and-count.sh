#!/usr/bin/env bash
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
branch=${1:-$(git rev-parse --abbrev-ref HEAD)}
tmpdir=$(mktemp -d "${TMPDIR:-/tmp}/hyperdex-rs-history-scrub.XXXXXX")
cleanup() {
  rm -rf "$tmpdir"
}
trap cleanup EXIT

git clone --quiet --no-local "$repo_root" "$tmpdir/repo"
cd "$tmpdir/repo"
git checkout --quiet "$branch"

if git log --format=%s | grep -q '^fixup! '; then
  GIT_SEQUENCE_EDITOR=: git rebase --autosquash --root >/dev/null
fi

"$repo_root/scripts/history-scrub/count-home-friel.sh" --tree
"$repo_root/scripts/history-scrub/count-home-friel.sh" --history
