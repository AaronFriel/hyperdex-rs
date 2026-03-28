# Workstream Plan: hegel-properties

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `the installed `autoplan` skill fallback rules`.

## Purpose / Big Picture

This workstream exists to get more real proof value out of Hegel specifically.
Its job is to use property-based exploration where ordinary deterministic
simulation is too narrow: replicated logical query behavior, multi-step state
transitions, and state-space coverage beyond one manually authored sequence.

## Goal

Land a growing set of Hegel-backed properties that cover real distributed
behavior instead of leaving Hegel isolated to one or two narrow checks.

## Acceptance Evidence

- The repository gains another Hegel-backed property that exercises a real
  correctness boundary.
- The Hegel properties are clearly aimed at logical-state coverage or reusable
  invariants, not just a rerun of a deterministic Turmoil scenario.
- The relevant crate stays green after the property lands.

## Mutable Surface

- the best crate for the property under test, such as:
  - `crates/simulation-harness/**`
  - `crates/placement-core/**`
  - protocol or storage crates when the property belongs there
- workstream files for this track

## Dependencies / Blockers

- None.

## Plan Of Work

Pick one correctness boundary at a time and express it as a generated Hegel
test in the crate where that invariant naturally belongs. Favor logical query
semantics, replicated state transitions, placement invariants, and other
operation-sequence behavior that would be tedious to cover by hand.

## Progress

- [x] (2026-03-29 19:20Z) Created the workstream and promoted it into the
  active board.
- [x] (2026-03-29 20:05Z) Landed
  `hegel_distributed_runtime_preserves_logical_delete_group_search_and_count`.
- [x] (2026-03-29 20:35Z) Landed
  `hegel_distributed_runtime_preserves_mixed_mutation_query_model`.
- [x] (2026-03-29 20:55Z) Landed
  `hegel_placement_strategies_preserve_replica_invariants_and_input_order_independence`
  in `placement-core`.
- [x] (2026-03-28 19:28Z) Landed
  `hegel_memory_engine_preserves_conditional_and_delete_matching_model`
  in `engine-memory`.
- [x] (2026-03-28 23:36Z) Landed
  `hegel_admin_requests_round_trip_and_preserve_supported_coordinator_mapping`
  in `hyperdex-admin-protocol`.
- [x] (2026-03-28 23:47Z) Landed
  `hegel_in_memory_catalog_preserves_space_and_daemon_state_model` in
  `control-plane`.
- [ ] Add the next Hegel property in another high-value correctness boundary
  beyond runtime, placement, `engine-memory`, `data-model`,
  `legacy-protocol`, `hyperdex-admin-protocol`, and `control-plane`.

## Current Hypothesis

Hegel is best used here for generated logical-state checks and reusable
invariants at multiple layers: runtime query semantics, mixed routed mutation
sequences, placement invariants, storage-state models, and protocol semantics
where the crate-local invariant is more natural than a full runtime
simulation.

## Next Bounded Step

Add the next Hegel property in a different correctness boundary from the
already-landed runtime, placement, `engine-memory`, `data-model`,
`legacy-protocol`, `hyperdex-admin-protocol`, and `control-plane`
properties, ideally `transport-core` or another storage/query boundary.

## Surprises & Discoveries

- The first new pass landed as a real property instead of only a proof map:
  generated routed `Put`, `DeleteGroup`, and `Get` operations now prove that
  replicated `Search` and `Count` stay logically deduplicated from either
  runtime.
- Hegel is no longer isolated to `simulation-harness`; `placement-core` now has
  a generated invariant check for both placement strategies.
- `engine-memory` now also has a generated storage-state model, so Hegel is
  spread across runtime, placement, and storage layers.
- `hyperdex-admin-protocol` now has a generated request-semantics property, so
  Hegel is also covering a protocol compatibility boundary.
- `control-plane` now has a generated catalog-state property, so Hegel also
  covers space and daemon membership transitions without needing a full
  runtime.

## Decision Log

- Decision: give Hegel its own active workstream instead of mixing it with
  Turmoil and Madsim.
  Rationale: Hegel is about generated state-space coverage, which is a
  different job from deterministic failure and recovery simulation.
  Date/Author: 2026-03-29 / root

## Outcomes & Retrospective

- `hegel_distributed_runtime_preserves_logical_delete_group_search_and_count`
  established the first runtime-level property in this track.
- `hegel_distributed_runtime_preserves_mixed_mutation_query_model` broadened
  runtime-level mixed-operation coverage.
- `hegel_placement_strategies_preserve_replica_invariants_and_input_order_independence`
  pushed Hegel into `placement-core`.
- `hegel_memory_engine_preserves_conditional_and_delete_matching_model`
  pushed Hegel into `engine-memory` with a crate-local storage-state model.
- `hegel_admin_requests_round_trip_and_preserve_supported_coordinator_mapping`
  pushed Hegel into `hyperdex-admin-protocol` with a generated wire
  encode/decode plus coordinator-mapping invariant.
- `hegel_in_memory_catalog_preserves_space_and_daemon_state_model` pushed
  Hegel into `control-plane` with a generated model for space catalog and
  daemon layout transitions.
