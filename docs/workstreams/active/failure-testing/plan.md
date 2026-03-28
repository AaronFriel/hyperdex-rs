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
- [ ] Land the first new failure-oriented deterministic proof and keep the
  workspace green.

## Current Hypothesis

The existing simulation surface is broad enough to support a more adversarial
test immediately. The best first pass is likely a failure of replica
propagation, stale config visibility, or degraded write/read ordering.

## Next Bounded Step

Choose one concrete distributed assumption to break, add the deterministic
proof in the simulation harness, and only touch runtime code if the proof
exposes a real bug.

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

- Pending.
