# Workstreams

This file explains the current `docs/workstreams/` layout.

## Layout

- [active](docs/workstreams/active)
  contains the workstreams that the root AutoPlan is actively driving now.
- [backlog](docs/workstreams/backlog)
  contains workstreams that are defined clearly enough to promote next, but are
  not active right now.
- [completed](docs/workstreams/completed)
  contains the finished workstreams from the earlier compatibility and
  baseline-building phase.

Each workstream directory contains:

- `plan.md`: the current purpose, owned surface, validator, and next bounded
  step
- `ledger.md`: the write-ahead log and outcome history for substantial work

## Active

- [failure-testing](docs/workstreams/active/failure-testing/plan.md)
- [distributed-simulation](docs/workstreams/active/distributed-simulation/plan.md)
- [hegel-properties](docs/workstreams/active/hegel-properties/plan.md)
- [history-scrub](docs/workstreams/active/history-scrub/plan.md)

## Backlog

- [warp-transactions](docs/workstreams/backlog/warp-transactions/plan.md)
- [georeplication](docs/workstreams/backlog/georeplication/plan.md)
- [programmability](docs/workstreams/backlog/programmability/plan.md)
- [graph-vector-layer](docs/workstreams/backlog/graph-vector-layer/plan.md)
- [temporal-layer](docs/workstreams/backlog/temporal-layer/plan.md)
- [object-storage-layer](docs/workstreams/backlog/object-storage-layer/plan.md)
- [database-corpus](docs/workstreams/backlog/database-corpus/plan.md)

## Completed

- [live-hyhac](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/completed/live-hyhac/plan.md)
- [simulation-proof](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/completed/simulation-proof/plan.md)
- [multiprocess-harness](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/completed/multiprocess-harness/plan.md)
- [coordinator-config-evidence](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/completed/coordinator-config-evidence/plan.md)
- [validation-ci](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/completed/validation-ci/plan.md)
- [async-modernization](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/completed/async-modernization/plan.md)
- [panic-hardening](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/completed/panic-hardening/plan.md)
- [fuzzing-hardening](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/completed/fuzzing-hardening/plan.md)
- [nextest-fast-feedback](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/completed/nextest-fast-feedback/plan.md)
