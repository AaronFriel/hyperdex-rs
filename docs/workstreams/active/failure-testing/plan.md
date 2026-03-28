# Workstream Plan: failure-testing

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `/home/friel/.codex/skills/autoplan/references/PLANS.md`.

## Purpose / Big Picture

This workstream adds tests that intentionally break assumptions instead of only
re-proving current happy paths. Its job is to harden the rewrite by forcing
the distributed runtime through realistic failure cases in deterministic
simulation.

## Goal

Land the next meaningful Turmoil or Madsim proof that breaks a real runtime
assumption and proves the resulting behavior is correct or exposes a real bug.

## Acceptance Evidence

- `simulation-harness` gains a new deterministic failure-oriented test.
- The test exercises a broken assumption, not just the current green path.
- `cargo test -p simulation-harness` passes.
- `cargo test --workspace` passes after the work lands.

## Mutable Surface

- `crates/simulation-harness/**`
- `crates/server/**` only if the new failure-oriented proof exposes a real
  runtime bug instead of only a proof gap

## Dependencies / Blockers

- None.

## Plan Of Work

Start from one concrete distributed failure question and answer it with the
shortest honest deterministic proof. The first pass should try to break a
replication, routing, or recovery assumption that matters to the current live
design.

## Progress

- [x] (2026-03-28 10:00Z) Created the workstream and promoted it to an active
  root priority for the next phase.
- [x] (2026-03-28 18:15Z) Bound this workstream to the dedicated
  `worktrees/failure-testing` checkout for one owned fork.
- [x] (2026-03-28 23:35Z) Landed a deterministic schema-convergence proof plus
  the required distributed-read fix.
- [x] (2026-03-28 23:55Z) Landed rollback-on-failed-replication fixes for
  writes and deletes in `f4e4215` and `4a3e876`.
- [x] (2026-03-29 00:39Z) Added
  `turmoil_reverts_primary_conditional_put_when_replica_transport_fails` in
  `7f02478`.
- [x] (2026-03-29 10:00Z) Added stale-placement mutation proof and fix in
  `8da80c8`.
- [x] (2026-03-29 10:10Z) Added stale-node rejoin proof in `06370d6`.
- [x] (2026-03-29 18:05Z) Hardened distributed delete-group rollback and
  schema-gap behavior in `b6ae810`, `a4ea7d3`, `fb77107`, and `2b7d144`.
- [ ] Choose the next broken distributed assumption after schema-convergence
  delete-group hardening.

## Current Hypothesis

The next highest-value step is now a primary-handoff or config-convergence
mutation path that overlaps with changing ownership, not just stale reads.
The runtime now has rollback coverage, stale-placement write guards, rejoin
proof, and delete-group schema-gap hardening, so the next proof should stress
what happens while ownership changes under an in-flight write or delete.

## Next Bounded Step

Add the shortest honest deterministic proof for a primary-handoff or
config-convergence mutation scenario, and touch runtime code only if the proof
shows the runtime can accept or expose incorrect state during ownership change.

## Surprises & Discoveries

- Observation: both Turmoil and Madsim are already present in the repository.
  Evidence: `Cargo.toml` already includes `turmoil`, and
  `crates/simulation-harness/Cargo.toml` already includes `madsim`.

## Decision Log

- Decision: make failure-oriented proof work active before larger distributed
  features.
  Rationale: the next features will be easier to land if the runtime is already
  being tested under broken assumptions.
  Date/Author: 2026-03-28 / root

## Outcomes & Retrospective

- The first three passes found real distributed bugs:
  - schema-convergence reads could abort on a live but stale replica
  - failed replicated writes could leak local primary state
  - failed replicated deletes could remove primary state without a committed
    replication result
- The fourth pass landed as proof-only evidence:
  - `ConditionalPut` already rolls back cleanly on replica transport failure
- The fifth pass found and fixed a real distributed ownership bug:
  - primary-only internode writes were accepted without verifying that the
    receiving node still owned the key under its current placement view
- The latest round found and fixed a second family of distributed convergence
  bugs:
  - distributed `DeleteGroup` needed rollback on replica failure
  - distributed `Search` and `Count` needed to tolerate schema-gap replicas
  - distributed `DeleteGroup` needed to skip schema-gap replicas without
    silently skipping true transport failures
