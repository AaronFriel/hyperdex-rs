# Workstream Plan: hegel-properties

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `/home/friel/.codex/skills/autoplan/references/PLANS.md`.

## Purpose / Big Picture

This workstream exists to get more real proof value out of Hegel specifically.
Its job is to use property-based exploration where ordinary deterministic
simulation is too narrow: replicated logical query behavior, multi-step state
transitions, and state-space coverage beyond one manually authored sequence.

## Goal

Land a growing set of Hegel-backed properties that cover real distributed
behavior instead of leaving Hegel isolated to one or two narrow checks.

## Acceptance Evidence

- `simulation-harness` gains another Hegel-backed property that exercises real
  distributed behavior.
- The Hegel properties are clearly aimed at logical-state coverage, not just a
  rerun of a deterministic Turmoil scenario.
- `cargo test -p simulation-harness` stays green after the property lands.

## Mutable Surface

- `crates/simulation-harness/**`
- workstream files for this track

## Dependencies / Blockers

- None.

## Plan Of Work

Pick one distributed property family at a time and express it as a generated
Hegel test over the real runtime. Favor logical query semantics, replicated
state transitions, and operation-sequence behavior that would be tedious to
cover by hand.

## Progress

- [x] (2026-03-29 19:20Z) Created the workstream and promoted it into the
  active board.
- [x] (2026-03-29 20:05Z) Landed
  `hegel_distributed_runtime_preserves_logical_delete_group_search_and_count`.
- [ ] Add the next Hegel property on a different distributed behavior family.

## Current Hypothesis

Hegel is best used here for generated logical-state checks over the real
runtime: delete-group semantics, search/count consistency, routed mutation
sequences, and eventually more transaction-like multi-step behavior.

## Next Bounded Step

Add the next Hegel property that covers a distinct distributed behavior family
from the new delete-group/search/count proof.

## Surprises & Discoveries

- The first new pass landed as a real property instead of only a proof map:
  generated routed `Put`, `DeleteGroup`, and `Get` operations now prove that
  replicated `Search` and `Count` stay logically deduplicated from either
  runtime.

## Decision Log

- Decision: give Hegel its own active workstream instead of mixing it with
  Turmoil and Madsim.
  Rationale: Hegel is about generated state-space coverage, which is a
  different job from deterministic failure and recovery simulation.
  Date/Author: 2026-03-29 / root

## Outcomes & Retrospective

- `hegel_distributed_runtime_preserves_logical_delete_group_search_and_count`
  is the first property in this dedicated track.
