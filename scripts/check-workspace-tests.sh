#!/usr/bin/env bash
set -euo pipefail

# Keep the routine workspace job on tests that pass in a stock CI image.
# The heavier distributed proof and live process coverage run in the separate
# acceptance workflow, and simulation-harness still depends on extra Hegel
# tooling that is not yet bootstrapped here.
cargo test --workspace --exclude server --exclude simulation-harness
cargo test -p server --lib
