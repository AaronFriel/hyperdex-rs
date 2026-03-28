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
real implementation phase: repository-grade validation, stronger failure
testing, fuzzing, async cleanup, Warp-style transactions, and region-aware
georeplication.

## Acceptance Evidence

- `cargo test --workspace` stays green.
- The live Hyhac-facing baseline remains green.
- Active workstreams land real code in `crates/**`, `.github/**`, or other
  product surfaces instead of mainly changing planning files.
- The repository gains stronger validation, stronger failure coverage, and the
  first bounded implementation steps for transactions and georeplication.

## Current Priorities

### Active

1. [validation-ci](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/active/validation-ci/plan.md)
2. [failure-testing](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/active/failure-testing/plan.md)
3. [async-modernization](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/active/async-modernization/plan.md)

### Backlog

1. [fuzzing-hardening](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/backlog/fuzzing-hardening/plan.md)
2. [warp-transactions](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/backlog/warp-transactions/plan.md)
3. [georeplication](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/backlog/georeplication/plan.md)
4. [programmability](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/backlog/programmability/plan.md)
5. [graph-vector-layer](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/backlog/graph-vector-layer/plan.md)
6. [temporal-layer](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/backlog/temporal-layer/plan.md)
7. [object-storage-layer](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/backlog/object-storage-layer/plan.md)

### Completed Baseline

- [completed](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/completed)

## Current Root Focus

Launch and reconcile substantive work in the three active workstreams without
recreating the documentation churn that dominated the previous phase.

## Next Root Move

Keep the root package small and truthful, track local worktrees outside git,
and spend the next coordination passes reconciling code results rather than
renarrating the plan.
