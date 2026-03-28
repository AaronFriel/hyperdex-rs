# Workstream Plan: distributed-simulation

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `the installed `autoplan` skill fallback rules`.

## Purpose / Big Picture

This workstream uses Turmoil and Madsim for deterministic distributed-system
behavior: node loss, rejoin, recovery, ownership changes, and operation
ordering. Its job is to prove something stronger than “reads still work when a
node is down”: what happens to ordering, ownership, and mutation acceptance
when a node fails, another node takes over, and the failed node later returns.

## Goal

Land deterministic Turmoil or Madsim proofs for recovery and operation
ordering across failover or rejoin paths, and fix the runtime if the proof
exposes a bug.

## Acceptance Evidence

- The repository gains a deterministic Turmoil or Madsim proof that exercises
  failure, recovery, and operation ordering together.
- The proof is deterministic and tied to a real distributed guarantee.
- The runtime stays green after the proof lands, with a fix if needed.

## Mutable Surface

- `crates/simulation-harness/**`
- `crates/server/**`
- `crates/transport-core/**`
- `crates/control-plane/**` only if needed

## Dependencies / Blockers

- None.

## Plan Of Work

Start with one concrete ordering question around failover or rejoin, not a
broad suite. The first pass should either prove or disprove that mutations are
accepted in the right order when ownership changes around a node failure.

## Progress

- [x] (2026-03-29 19:20Z) Created the workstream and promoted it into the
  active board.
- [x] (2026-03-29 20:25Z) Landed the first recovery-ordering proof on the
  merged tree.
- [x] (2026-03-29 20:45Z) Landed delete-then-rewrite visibility proof after
  stale rejoin.
- [x] (2026-03-28 22:16Z) Landed single-key delete retry/rewrite recovery
  proof after replica outage.
- [x] (2026-03-28 23:45Z) Landed a Madsim stale-rejoin ordering proof so the
  scheduler now covers both outage-retry and stale-placement recovery.
- [ ] Land the next distributed recovery proof beyond the current two-node
  stale-rejoin and replica-outage families.

## Current Hypothesis

The workstream now covers six concrete recovery proofs on the merged tree:
two stale-rejoin ordering or visibility cases under Turmoil, one stale-rejoin
ordering case under Madsim, and three replica-outage retry or recovery cases.
The next high-value step is either a three-node failover or handoff proof, or
another recovery family that is not just a two-node stale rejoin or a
replica-outage retry.

## Next Bounded Step

Add the next deterministic recovery proof that exercises either three-node
failover or handoff ordering, or a recovery family outside the current two-node
stale-rejoin and replica-outage retry shapes.

## Surprises & Discoveries

- None yet.

## Decision Log

- Decision: keep Turmoil and Madsim together as one active workstream and keep
  Hegel separate.
  Rationale: Turmoil and Madsim serve the same deterministic distributed
  simulation role here, while Hegel serves generated property testing.
  Date/Author: 2026-03-29 / root

## Outcomes & Retrospective

- The first pass now proves a concrete recovery-ordering guarantee:
  a rejected stale local-primary write does not leak through recovery, and two
  later authoritative writes are observed in order on the recovered node.
- The second pass now proves a concrete visibility guarantee:
  after stale rejoin, authoritative `Put -> Delete -> Put` transitions are seen
  in order on the recovered node, with no stale resurrection across the delete.
- The latest pass now proves that a single-key delete which fails during replica
  outage rolls back cleanly, succeeds after recovery, and does not interfere
  with a later rewrite observed on both replicas.
- The latest pass also proves the stale local-primary rejoin ordering guarantee
  under Madsim: a rejected pre-recovery write does not leak through, and two
  later authoritative writes are observed in order after the recovered node
  rejoins with the converged two-node view.
