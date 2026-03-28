# Workstream Plan: simulation-proof

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `/home/friel/.codex/skills/autoplan/references/PLANS.md`.

## Purpose / Big Picture

This workstream keeps deterministic proof coverage strong enough that live
cluster failures mean something real. Its job is not to replace the live
cluster checks; its job is to make routing, replication, and failure behavior
cheap to validate and hard to regress, and to return the workspace to green
when those proofs expose a real product gap.

## Goal

Keep `simulation-harness` representative of the distributed behavior that the
legacy and gRPC frontends actually rely on, with generated tests over routed
mutations, degraded reads, and schema-correct attribute use.

## Acceptance Evidence

- `cargo test -p simulation-harness` passes.
- `cargo test --workspace` passes after any simulation-harness change.
- Generated distributed properties cover routed put/get, delete,
  conditional-write, and numeric mutation over the declared `profile_views`
  attribute.

## Mutable Surface

- `crates/simulation-harness/src/lib.rs`
- `crates/simulation-harness/Cargo.toml`
- Supporting proof notes under `docs/` only when the root requests them

## Dependencies / Blockers

- None. The current blocker is inside this workstream’s owned surface.

## Plan Of Work

Start from the currently green deterministic baseline and only reopen this
workstream when a real live or runtime failure needs new proof coverage or a
proof correction. The last bounded step fixed a stale degraded-read assumption
and returned the workspace to green.

## Progress

- [x] (2026-03-27 04:19Z) Imported this workstream from the old root-only file
  and recorded its current mutable surface, validator, and next bounded step.
- [x] (2026-03-27 04:22Z) Reconciled the in-flight routed numeric-mutation
  property into `6d55620` (`Add Hegel routed numeric mutation coverage`).
- [x] (2026-03-27 04:33Z) Tightened the remaining schema-permissive single-node
  Hegel sequence test in `5cc0cf8` (`Fix Hegel single-node schema usage`).
- [x] (2026-03-28 03:27Z) Fixed the stale degraded-read simulation assumption,
  aligned the degraded-read proofs to a replicated schema, and returned
  `cargo test --workspace` to green on root.

## Current Hypothesis

The current proof surface is back in a good holding state. The next useful
proof step should come from a fresh live or runtime failure rather than from a
stale simulation assumption.

## Next Bounded Step

Hold this workstream parked until a new live or runtime failure needs a
deterministic proof change.

## Surprises & Discoveries

- Observation: the per-process Hegel temporary virtualenv path is already in
  the working tree, so the current numeric-mutation step does not need to solve
  shared `/tmp` collisions before adding the property itself.
  Evidence: `ensure_hegel_server_command()` already uses
  `std::process::id()` in `crates/simulation-harness/src/lib.rs`.
- Observation: the earlier degraded-read simulation failure was a stale proof
  assumption rather than a runtime regression.
  Evidence: the degraded-read tests were using a schema with `tolerate 0
  failures`, while asserting replica fallback after one node loss; after
  switching those proofs to a replicated schema, `cargo test -p
  simulation-harness` and `cargo test --workspace` both passed again.

## Decision Log

- Decision: keep numeric mutation and the remaining single-node schema cleanup
  as separate bounded steps.
  Rationale: the numeric property already has strong direct evidence, while the
  single-node sequence model needs a separate judgment about how much to change
  in one pass.
  Date/Author: 2026-03-27 / root

## Outcomes & Retrospective

- `6d55620` landed the routed numeric-mutation property cleanly with no
  additional surface beyond `crates/simulation-harness/src/lib.rs`.
- `5cc0cf8` made the single-node Hegel sequence model schema-correct without
  widening the proof surface.
