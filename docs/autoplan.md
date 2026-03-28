# AutoPlan

This is the active root AutoPlan for `hyperdex-rs`.

Use this file for current priorities only. Earlier planning state is archived
under [archive/phase-1](/home/friel/c/aaronfriel/hyperdex-rs/docs/archive/phase-1).

## Companion Files

- Root ledger: [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/ledger.md)
- Workstream index: [workstreams.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams.md)
- Capability ladder: [capability-ladder.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/capability-ladder.md)
- Future directions: [future-directions.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/future-directions.md)
- Paper notes: [papers-and-mvp-notes.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/papers-and-mvp-notes.md)
- Hyhac compatibility notes: [hyhac-compatibility-surface.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/hyhac-compatibility-surface.md)
- Archived phase-1 package: [archive/phase-1](/home/friel/c/aaronfriel/hyperdex-rs/docs/archive/phase-1)

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

1. [failure-testing](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/active/failure-testing/plan.md)
2. [distributed-simulation](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/active/distributed-simulation/plan.md)
3. [hegel-properties](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/active/hegel-properties/plan.md)

### Backlog

1. [warp-transactions](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/backlog/warp-transactions/plan.md)
2. [georeplication](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/backlog/georeplication/plan.md)
3. [programmability](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/backlog/programmability/plan.md)
4. [graph-vector-layer](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/backlog/graph-vector-layer/plan.md)
5. [temporal-layer](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/backlog/temporal-layer/plan.md)
6. [object-storage-layer](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/backlog/object-storage-layer/plan.md)
7. [database-corpus](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/backlog/database-corpus/plan.md)

### Completed Baseline

- [completed](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/completed)

## Current Root Focus

Keep the green HyperDex replacement baseline honest while making proof work
more deliberate: keep failure-oriented bug-finding active, treat Turmoil and
Madsim as the distributed-failure and recovery tool family, and give Hegel its
own property-testing track with broader state-space coverage than a single test.

## Next Root Move

Keep reconciling real code from the three active tracks: land the next Hegel
property, land the next deterministic distributed recovery proof, and move the
parked ownership-convergence fix forward on the current tree.
