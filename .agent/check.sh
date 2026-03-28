#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
TIMING_LOG="$(mktemp -t hyperdex-rs-agent-check.XXXX)"
trap 'rm -f "$TIMING_LOG"' EXIT

run_step() {
  local label="$1"; shift
  local start end
  start="$(date +%s)"
  echo "▶ $label"
  "$@"
  end="$(date +%s)"
  local sec=$(( end - start ))
  printf "%3ds  %s\n" "$sec" "$label" | tee -a "$TIMING_LOG"
}

run_nextest_fast() {
  cargo nextest run --workspace -E "$NEXTTEST_FAST_FILTER" 2> >(
    grep -F -v "warning: in config file .config/nextest.toml, ignoring unknown configuration keys:  profile.default.terminate-after" >&2
  )
}

cd "$REPO_ROOT"

NEXTTEST_FAST_FILTER="${NEXTTEST_FAST_FILTER:-not test(/^slow_/)}"

run_step "module layout" scripts/check-module-layout.sh
run_step "shell syntax" bash -n .agent/check.sh scripts/check-clippy.sh scripts/check-module-layout.sh scripts/check-workspace-tests.sh scripts/verify-live-acceptance.sh
run_step "rustfmt (check)" cargo fmt --all -- --check
run_step "nextest fast suite" run_nextest_fast

echo
echo "Slow tests are named with a slow_ prefix and are excluded from the fast nextest path."
echo
echo "Timing summary (seconds, highest first):"
sort -nr "$TIMING_LOG"
