# Workstream Plan: distributed-simulation

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `/home/friel/.codex/skills/autoplan/references/PLANS.md`.

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
- [ ] Land the next distributed recovery proof beyond stale rejoin ordering.

## Current Hypothesis

The first stale-rejoin ordering proof is now in place. The next high-value
step is another deterministic recovery scenario that couples node return or
failover with a different operation family or with multi-node disagreement.

## Next Bounded Step

Add the next deterministic Turmoil or Madsim proof that extends recovery
coverage beyond ordered writes after stale rejoin.

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
