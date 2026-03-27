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

- A focused coordinator-plus-daemon admin probe harness test runs quickly
  enough to act as the fast proxy loop for live compatibility work.
- `cargo test -p server --test dist_multiprocess_harness -- --nocapture`
  passes.
- `cargo test --workspace` passes after harness changes.
- The harness no longer fails from same-process port collisions when its tests
  run together.

## Mutable Surface

- `Cargo.toml`
- `crates/server/Cargo.toml`
- `crates/server/tests/**`
- `crates/server/src/main.rs` only if a readiness-protocol change becomes
  necessary after the current bounded step

## Dependencies / Blockers

- None for the current fast reproducer step.
- This workstream should avoid product-code overlap with `live-hyhac`; it owns
  the probe and harness side of the loop, not the daemon-path product fix
  itself.

## Plan Of Work

Keep the existing multiprocess harness trustworthy, then use it to shorten the
feedback loop for live compatibility work. The current bounded step is to turn
the new legacy daemon `ClientGarbage` failure into a shorter, repeatable probe
than the selected `hyhac` command, so product changes can be judged without
waiting on the whole Haskell path every time.

## Progress

- [x] (2026-03-27 04:19Z) Imported this workstream from the old root-only file
  and recorded the in-flight harness stabilization step.
- [x] (2026-03-27 04:22Z) Reconciled the `serial_test` containment fix into
  `98def36` (`Stabilize multiprocess harness concurrency`).
- [x] (2026-03-27 04:33Z) Replaced ephemeral port reuse and log-text waits
  with protocol-based readiness in `faa6cb6`
  (`Use protocol readiness in multiprocess harness`).
- [x] (2026-03-27 07:10Z) Reopened this workstream as an active parallel owner
  because the live admin path needs a faster cluster-plus-admin proxy loop than
  the current manual probe sequence.
- [x] (2026-03-27 07:20Z) Reconciled `6f061b3` (`Add legacy admin bootstrap
  probe harness`), which adds a focused process-level admin progress test to
  `dist_multiprocess_harness`.
- [x] (2026-03-27 07:35Z) Reopened this workstream immediately again because
  the blocker moved from coordinator bootstrap to the legacy daemon data path,
  and the selected `hyhac` command is now the slowest useful failing check.
- [x] (2026-03-27 07:45Z) Reconciled `0b2379d` (`Add fast hyhac ClientGarbage
  repro probes`), which reduces the first daemon-path failure to the focused
  `*Can store a large object*` `hyhac` subset.

## Current Hypothesis

The short `ClientGarbage` repro is now on `main`, so this workstream is back in
a good holding state. The next change here should come from a newly observed
cluster-validation problem or a need for an even tighter repro.

## Next Bounded Step

Hold until the product worker or another real-cluster failure needs more
harness work.

## Surprises & Discoveries

- Observation: the paused harness worker also found ephemeral port reuse inside
  one test, not just cross-test interference.
  Evidence: its reported logs showed `Address already in use (os error 98)` and
  a case where `daemon-two control_port == coordinator_port`.
- Observation: the current live compatibility path still leans on a manual
  probe loop that is slower than it needs to be.
  Evidence: the most recent legacy-admin narrowing came from an external
  captured-wire run rather than a quick repeatable test under
  `dist_multiprocess_harness`.
- Observation: the new harness confirms the current bug directly from a fast
  test.
  Evidence: the targeted test prints `advanced=false` and captures
  `first_server=ClientResponse` after the bootstrap exchange.
- Observation: once bootstrap/admin compatibility landed, the next slowest
  measurement immediately became the daemon client path through `hyhac`.
  Evidence: the selected `hyhac` command now reaches real client operations and
  fails with `Left ClientGarbage`.
- Observation: that daemon-path failure is already reproducible with a much
  smaller public subset.
  Evidence: `legacy_hyhac_large_object_probe_hits_clientgarbage_fast` reaches
  `Left ClientGarbage` in about `107ms`.

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
- `6f061b3` adds that fast live admin probe loop, and the strongest next use of
  this workstream was to shorten the new `ClientGarbage` reproduction path.
- `0b2379d` delivers that shorter repro, so this workstream can wait again
  until the next measurement bottleneck appears.
