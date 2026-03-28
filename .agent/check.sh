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

cd "$REPO_ROOT"

run_step "module layout" scripts/check-module-layout.sh
run_step "workflow audit" scripts/check-actions.sh
run_step "shell syntax" scripts/check-shell-syntax.sh
run_step "gitleaks" scripts/check-gitleaks.sh
run_step "rustfmt (check)" scripts/check-rustfmt.sh
run_step "nextest fast suite" scripts/check-nextest-fast.sh

echo
echo "Slow tests are named with a slow_ prefix and are excluded from the fast nextest path."
echo
echo "Timing summary (seconds, highest first):"
sort -nr "$TIMING_LOG"
