#!/usr/bin/env bash
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
branch=${1:-$(git rev-parse --abbrev-ref HEAD)}
tmpdir=$(mktemp -d "${TMPDIR:-/tmp}/hyperdex-rs-history-scrub.XXXXXX")
helper="$tmpdir/scrub-easy-paths.py"
cleanup() {
  rm -rf "$tmpdir"
}
trap cleanup EXIT

git clone --quiet --no-local "$repo_root" "$tmpdir/repo"
cp "$repo_root/scripts/history-scrub/scrub-easy-paths.py" "$helper"
cd "$tmpdir/repo"
git checkout --quiet "$branch"

if git log --format=%s | grep -q '^fixup! '; then
  GIT_SEQUENCE_EDITOR=: git rebase --autosquash --root >/dev/null
fi

FILTER_BRANCH_SQUELCH_WARNING=1 git filter-branch --force \
  --tree-filter "python3 '$helper' >/dev/null" \
  HEAD >/dev/null 2>&1

"$repo_root/scripts/history-scrub/count-home-friel.sh" --tree
"$repo_root/scripts/history-scrub/count-home-friel.sh" --history
