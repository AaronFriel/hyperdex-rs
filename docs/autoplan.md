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
2. [panic-hardening](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/active/panic-hardening/plan.md)
3. [fuzzing-hardening](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/active/fuzzing-hardening/plan.md)
4. [nextest-fast-feedback](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/active/nextest-fast-feedback/plan.md)

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

Keep the green HyperDex replacement baseline honest while driving four active
product workstreams in parallel: the next adversarial distributed proof, the
next panic/no-panic and lint-ratchet pass on public boundaries, the first real
fuzz targets for the highest-risk protocol decoders, and a `cargo nextest`
fast-feedback path that keeps the core suite reliably within roughly 30
seconds.

## Next Root Move

Keep the watchdog armed, preregister the next failure-testing,
panic-hardening, fuzzing, and nextest passes, and launch those four
product-only workstreams from the updated active board.
