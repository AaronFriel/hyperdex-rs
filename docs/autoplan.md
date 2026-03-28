# AutoPlan

This is the active root AutoPlan for `hyperdex-rs`.

Use this file for current priorities only. Earlier planning state is archived
under [archive/phase-1](docs/archive/phase-1).

## Companion Files

- Root ledger: [ledger.md](docs/ledger.md)
- Workstream index: [workstreams.md](docs/workstreams.md)
- Capability ladder: [capability-ladder.md](docs/capability-ladder.md)
- Future directions: [future-directions.md](docs/future-directions.md)
- Paper notes: [papers-and-mvp-notes.md](docs/papers-and-mvp-notes.md)
- Hyhac compatibility notes: [hyhac-compatibility-surface.md](docs/hyhac-compatibility-surface.md)
- Archived phase-1 package: [archive/phase-1](docs/archive/phase-1)

## Goal

Keep the now-green HyperDex replacement baseline stable while driving the next
implementation phase: stronger failure testing, panic hardening, fuzzing, and
the first bounded implementation steps for transactions and region-aware
georeplication.

## Acceptance Evidence

- `cargo test --workspace` stays green.
- The live Hyhac-facing baseline remains green.
- Active workstreams land real code in `crates/**`, `.github/**`, or other
  product surfaces instead of mainly changing planning files.
- The repository gains stronger validation, stronger failure coverage, and the
  first bounded implementation steps for transactions and georeplication.
- Entry points and important public surfaces are moving toward explicit
  no-panic contracts and away from unchecked `unwrap` / `expect` usage.

## Current Priorities

### Active

1. [failure-testing](docs/workstreams/active/failure-testing/plan.md)
2. [distributed-simulation](docs/workstreams/active/distributed-simulation/plan.md)
3. [hegel-properties](docs/workstreams/active/hegel-properties/plan.md)
4. [history-scrub](docs/workstreams/active/history-scrub/plan.md)

### Backlog

1. [warp-transactions](docs/workstreams/backlog/warp-transactions/plan.md)
2. [georeplication](docs/workstreams/backlog/georeplication/plan.md)
3. [programmability](docs/workstreams/backlog/programmability/plan.md)
4. [graph-vector-layer](docs/workstreams/backlog/graph-vector-layer/plan.md)
5. [temporal-layer](docs/workstreams/backlog/temporal-layer/plan.md)
6. [object-storage-layer](docs/workstreams/backlog/object-storage-layer/plan.md)
7. [database-corpus](docs/workstreams/backlog/database-corpus/plan.md)

### Completed Baseline

- [completed](docs/workstreams/completed)

## Current Root Focus

Keep the green HyperDex replacement baseline honest while making proof work
more deliberate: keep failure-oriented bug-finding active, treat Turmoil and
Madsim as the distributed-failure and recovery tool family, and give Hegel its
own property-testing track with broader state-space coverage than a single test
or a single crate. In parallel, remove machine-specific home-directory paths
from the repository and keep driving the remaining external-local bucket down
with a repeatable history-rewrite toolchain so the repository can be pushed
safely.

## Next Root Move

Keep reconciling real code from the three active tracks: push failure-testing
to the next ownership-convergence or mixed-mutation case, extend distributed
recovery beyond the current stale-rejoin and outage-retry proofs, and continue
spreading Hegel properties into other correctness boundaries such as protocol
or storage. At the same time, keep `history-scrub` active for the deferred
external-local bucket now that the easy repo-local bucket is at zero in the
current tree and the rewrite rehearsal toolchain is landed.
