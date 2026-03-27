# Workstream Plan: multiprocess-harness

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `/home/friel/.codex/skills/autoplan/references/PLANS.md`.

## Purpose / Big Picture

This workstream keeps the real coordinator-plus-daemons validator reliable
enough that root-level decisions can trust it. When this harness flakes, proof
and compatibility work become noisy and the root loses a trustworthy signal.

## Goal

Make the multiprocess harness deterministic enough that workspace-wide test
runs can trust its failures and successes.

## Acceptance Evidence

- `cargo test -p server --test dist_multiprocess_harness -- --nocapture`
  passes.
- `cargo test --workspace` passes after harness changes.
- The harness no longer fails from same-process port collisions when its tests
  run together.

## Mutable Surface

- `Cargo.toml`
- `crates/server/Cargo.toml`
- `crates/server/tests/dist_multiprocess_harness.rs`
- `crates/server/src/main.rs` only if a readiness-protocol change becomes
  necessary after the current bounded step

## Dependencies / Blockers

- None for the current serialization fix.
- A follow-up protocol-based readiness change will depend on whether the
  current serial containment fix is sufficient for stable workspace runs.

## Plan Of Work

First reconcile the already-tested serial containment fix that is sitting in the
root checkout and in the paused worktree. Then replace the remaining brittle
parts one at a time: ephemeral port reuse, then log-text waits.

## Progress

- [x] (2026-03-27 04:19Z) Imported this workstream from the old root-only file
  and recorded the in-flight harness stabilization step.
- [x] (2026-03-27 04:22Z) Reconciled the `serial_test` containment fix into
  `98def36` (`Stabilize multiprocess harness concurrency`).
- [x] (2026-03-27 04:33Z) Replaced ephemeral port reuse and log-text waits
  with protocol-based readiness in `faa6cb6`
  (`Use protocol readiness in multiprocess harness`).

## Current Hypothesis

The harness is in a good holding state now. The next change here should be
driven by a newly observed cluster-validation failure, not by another
speculative cleanup pass.

## Next Bounded Step

Wait for the next real-cluster failure that requires another harness change.

## Surprises & Discoveries

- Observation: the paused harness worker also found ephemeral port reuse inside
  one test, not just cross-test interference.
  Evidence: its reported logs showed `Address already in use (os error 98)` and
  a case where `daemon-two control_port == coordinator_port`.

## Decision Log

- Decision: keep the current step limited to serializing the three
  process-spawning tests.
  Rationale: this is already implemented in the root checkout, has passing
  validator evidence, and is the smallest bounded containment step. The deeper
  readiness cleanup belongs in the next step.
  Date/Author: 2026-03-27 / root

## Outcomes & Retrospective

- `98def36` restored stable workspace runs by serializing the three
  process-spawning multiprocess tests without touching product code.
- `faa6cb6` replaced ephemeral port reuse and log-text waits with held port
  reservations plus protocol-based readiness checks.
