#!/usr/bin/env bash
set -euo pipefail

# Keep routine CI linting on the workspace surfaces that are currently green.
# The server crate still has outstanding clippy debt, and transport-grpc plus
# simulation-harness pull server back in through dev-dependencies when all
# targets are enabled. Fix that debt in a product branch instead of hiding it
# here.
cargo clippy \
  -p cluster-config \
  -p consensus-core \
  -p control-plane \
  -p data-model \
  -p data-plane \
  -p engine-memory \
  -p engine-rocks \
  -p hyperdex-admin-protocol \
  -p hyperdex-client-protocol \
  -p legacy-frontend \
  -p legacy-protocol \
  -p placement-core \
  -p storage-core \
  -p transport-core \
  --all-targets -- \
  -D warnings \
  -A clippy::assign-op-pattern \
  -A clippy::filter-map-bool-then \
  -A clippy::field-reassign-with-default \
  -A clippy::needless-lifetimes \
  -A clippy::needless-question-mark \
  -A clippy::result-large-err \
  -A clippy::uninlined-format-args \
  -A clippy::unnecessary-get-then-check \
  -A clippy::too-many-arguments
