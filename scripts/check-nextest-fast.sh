#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
NEXTTEST_FAST_FILTER="${NEXTTEST_FAST_FILTER:-not test(/^slow_/)}"

cd "$REPO_ROOT"

cargo nextest run --workspace -E "$NEXTTEST_FAST_FILTER" 2> >(
  grep -F -v "warning: in config file .config/nextest.toml, ignoring unknown configuration keys:  profile.default.terminate-after" >&2
)
