# Workstream Plan: recovery-ordering

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `/home/friel/.codex/skills/autoplan/references/PLANS.md`.

## Purpose / Big Picture

This workstream targets failure, recovery, and operation-order guarantees. Its
job is to prove something stronger than “reads still work when a node is down”:
what happens to ordering, ownership, and mutation acceptance when a node fails,
another node takes over, and the failed node later returns.

## Goal

Land the first deterministic proof for recovery and operation ordering across a
failover or rejoin path, and fix the runtime if the proof exposes a bug.

## Acceptance Evidence

- The repository gains a proof that exercises failure, recovery, and operation
  ordering together.
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
- [ ] Land the first recovery-ordering proof.

## Current Hypothesis

The most useful first proof is likely around stale-primary or local-primary
mutation acceptance during convergence, because that sits directly on the
boundary between liveness and correctness.

## Next Bounded Step

Turn the parked ownership-convergence result into either a merged proof/fix or
the next cleaner variant of that same guarantee.

## Surprises & Discoveries

- None yet.

## Decision Log

- Decision: split recovery/ordering out as its own active effort instead of
  hiding it under generic failure testing.
  Rationale: the user wants stronger guarantees around node failure, recovery,
  and order of operations specifically.
  Date/Author: 2026-03-29 / root

## Outcomes & Retrospective

- Pending.
