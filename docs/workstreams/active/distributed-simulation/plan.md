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
- [ ] Land the next distributed recovery proof beyond the stale-rejoin,
  delete-group, mixed conditional-put, and single-key delete recovery cases.

## Current Hypothesis

The workstream now covers five concrete recovery proofs on the merged tree:
stale-rejoin ordered writes, stale-rejoin delete/rewrite visibility,
delete-group retry after outage, mixed conditional-put retry after outage, and
single-key delete retry after outage. The next high-value step is either a
three-node failover/handoff proof or a Madsim proof for one of the non-
delete-group outage recovery paths.

## Next Bounded Step

Add the next deterministic Turmoil or Madsim proof that exercises recovery with
either three-node failover/handoff or non-delete-group outage recovery under
Madsim.

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
