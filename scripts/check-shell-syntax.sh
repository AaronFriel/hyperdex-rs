#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"

cd "$REPO_ROOT"

bash -n \
  .agent/check.sh \
  .agent/setup.sh \
  scripts/check-actions.sh \
  scripts/check-clippy.sh \
  scripts/check-module-layout.sh \
  scripts/check-rustfmt.sh \
  scripts/check-shell-syntax.sh \
  scripts/check-workspace-tests.sh \
  scripts/check-nextest-fast.sh \
  scripts/verify-live-acceptance.sh
