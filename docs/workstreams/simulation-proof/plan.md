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
cheap to validate and hard to regress.

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

- None for the currently in-flight numeric-mutation proof.
- The next cleanup after that depends on deciding whether the single-node Hegel
  sequence model should be made schema-correct in one bounded step or split.

## Plan Of Work

First reconcile the already-tested routed numeric-mutation property that is
currently in the root checkout and also exists as a worktree commit. Then use
that landing point to narrow the remaining schema-permissive generated tests,
starting with the single-node sequence model that still writes and reads the
undeclared `name` attribute.

## Progress

- [x] (2026-03-27 04:19Z) Imported this workstream from the old root-only file
  and recorded its current mutable surface, validator, and next bounded step.
- [x] (2026-03-27 04:22Z) Reconciled the in-flight routed numeric-mutation
  property into `6d55620` (`Add Hegel routed numeric mutation coverage`).
- [ ] Tighten the remaining schema-permissive single-node Hegel sequence test.

## Current Hypothesis

The routed numeric-mutation path is now covered. The most obvious remaining
proof weakness is the single-node Hegel sequence model still using an undeclared
attribute, and fixing that should strengthen generated coverage without
changing distributed behavior.

## Next Bounded Step

Make the single-node Hegel sequence model schema-correct, validate
`simulation-harness` and the full workspace again, and record the outcome in
this workstream ledger.

## Surprises & Discoveries

- Observation: the per-process Hegel temporary virtualenv path is already in
  the working tree, so the current numeric-mutation step does not need to solve
  shared `/tmp` collisions before adding the property itself.
  Evidence: `ensure_hegel_server_command()` already uses
  `std::process::id()` in `crates/simulation-harness/src/lib.rs`.

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
