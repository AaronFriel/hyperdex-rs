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
- [x] (2026-03-27 07:50Z) Reopened this workstream again right after landing
  the fast repro because the product worker still needs wire-level evidence for
  the first bad request/response pair on that same large-object path.
- [x] (2026-03-27 19:49Z) Retired the first wire-capture retry after the
  interrupted worker left unrelated product files dirty in the old worktree,
  then moved the same goal onto a clean replacement worktree.
- [x] (2026-03-27 19:54Z) Retired the clean replacement retry after it only
  reverified the baseline failure without producing new wire evidence, then
  reopened the same workstream with a stricter harness-only success condition.

## Current Hypothesis

The short `ClientGarbage` repro is now on `main`, but the product worker still
only has a smaller failing subset, not the first bad request/response pair. The
first retry drifted across unrelated product files, and the second retry stayed
clean but stopped at baseline verification. This workstream now needs a stricter
success condition: expose or decode the first bad daemon frame directly from the
fast repro without taking over the product fix itself.

## Next Bounded Step

Expose or decode the first bad daemon-path request/response pair around the
large-object `ClientGarbage` repro.

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
- Observation: the shorter repro still does not tell us exactly which daemon
  request/response edge the legacy client rejects.
  Evidence: the landed tests prove the public failure quickly, but they stop at
  the failing subset rather than capturing the first bad legacy data-plane
  exchange.
- Observation: the first retry for wire-level capture did not stay inside the
  harness-owned surface.
  Evidence: the interrupted `clientgarbage-probe` worktree ended with edits in
  product files outside `crates/server/tests/**`, including
  `crates/server/src/lib.rs` and several other core crates.
- Observation: the clean replacement retry avoided drift but still did not move
  beyond the already-known baseline.
  Evidence: the completed worker returned only that `main` at `ad458f1` still
  reproduces `Left ClientGarbage` and that `cargo test -p server` is green,
  with no code changes in `clientgarbage-wire`.

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
  unless the product worker benefits from wire-level evidence on that same
  path. That is the current bounded step.
- The first wire-capture retry produced no bounded harness result because the
  worktree drifted outside its owned surface. The replacement attempt is now
  explicitly tied to a fresh worktree before any further code is trusted.
- The clean replacement retry also produced no bounded harness result. The next
  attempt must return either a harness commit that exposes the first bad frame
  directly or a clean proof tied to test output that identifies the bad edge.
