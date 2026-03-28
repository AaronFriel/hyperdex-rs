#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/verify-live-acceptance.sh [--quick|--ci|--full]

Runs the live acceptance checks for hyperdex-rs.

Modes:
  --quick  Run the single-daemon live Hyhac acceptance proof.
  --ci     Run bounded in-repo distributed acceptance checks that do not rely
           on the sibling `hyhac` checkout.
  --full   Run the single-daemon live Hyhac acceptance proof plus
           representative distributed multiprocess checks.

Default:
  --full
EOF
}

mode="full"

for arg in "$@"; do
  case "$arg" in
    --quick)
      mode="quick"
      ;;
    --ci)
      mode="ci"
      ;;
    --full)
      mode="full"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $arg" >&2
      usage >&2
      exit 2
      ;;
  esac
done

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
cd "$repo_root"

cargo_cmd="${CARGO:-cargo}"

run_test() {
  local test_name="$1"
  echo "==> $test_name"
  "$cargo_cmd" test -p server --test dist_multiprocess_harness "$test_name" -- --nocapture
}

if [[ "$mode" == "quick" || "$mode" == "full" ]]; then
  run_test "legacy_hyhac_split_acceptance_suite_passes_live_cluster"
fi

if [[ "$mode" == "ci" || "$mode" == "full" ]]; then
  run_test "coordinator_space_add_reaches_multiple_daemon_processes"
  run_test "legacy_atomic_routes_numeric_update_to_remote_primary_process"
  run_test "degraded_search_and_count_survive_one_daemon_process_shutdown"
fi
