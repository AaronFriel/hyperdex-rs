# Workstream Plan: live-hyhac

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `/home/friel/.codex/skills/autoplan/references/PLANS.md`.

## Purpose / Big Picture

This workstream is the direct path to the user-visible objective: a live
`hyperdex-rs` cluster that can run `hyhac` without semantic drift. It should
consume the improved proof and harness signal from the other workstreams, then
use observed `hyhac` failures to choose the next compatibility step.

## Goal

Run `hyhac` against the live cluster, capture the next failing operation or
semantic mismatch, and narrow implementation work to that concrete evidence
until the suite passes.

## Acceptance Evidence

- The `hyhac` harness runs against a real `hyperdex-rs` cluster.
- The next failing operation, if any, is recorded from observed output rather
  than guessed from the code.
- Eventually, the live `hyhac` suite passes.

## Mutable Surface

- `crates/legacy-protocol/**`
- `crates/legacy-frontend/**`
- `crates/hyperdex-admin-protocol/**`
- `crates/hyperdex-client-protocol/**`
- `crates/server/**`
- `/home/friel/c/aaronfriel/hyhac/scripts/**` only when launcher or harness
  wiring must point at `hyperdex-rs`

## Dependencies / Blockers

- None. The proof and multiprocess-harness fixes that this workstream was
  waiting on are now on `main`.

## Plan Of Work

Start a live `hyperdex-rs` coordinator and daemon directly from the Rust
binary, run `hyhac` through `scripts/cabal.sh test ...` instead of the checked-in
`scripts/test-with-hyperdex.sh` wrapper, record the first failing operation or
return-code mismatch, and narrow the next compatibility change to that observed
surface.

## Progress

- [x] (2026-03-27 04:19Z) Created the workstream package and recorded its
  mutable surface and validator boundary.
- [x] (2026-03-27 04:22Z) Confirmed that `hyhac`'s checked-in launcher still
  hardwires the original C++ `hyperdex` binary, so the live probe must use a
  manual `hyperdex-rs` cluster plus the direct Cabal test command.
- [ ] Run the live `hyhac` harness against the updated `main`.
- [ ] Record the next failing operation in the workstream ledger.
- [ ] Narrow the next compatibility change to that observed failure.

## Current Hypothesis

The first live failure is likely to be admin `create space` or
`waitUntilStable`, because `hyhac` starts there before it reaches client
traffic, and the checked-in compatibility notes already identify that part of
the public surface as the first live contract.

## Next Bounded Step

Start one `hyperdex-rs` coordinator and one daemon, run the direct Cabal test
command with `HYPERDEX_COORD_HOST` and `HYPERDEX_COORD_PORT` pointed at that
cluster, capture the first failing operation, and preregister the follow-up
implementation step in this ledger.

## Surprises & Discoveries

- Observation: `scripts/test-with-hyperdex.sh` cannot drive `hyperdex-rs`
  directly because it shells through `start-hyperdex.sh`, which requires the
  original `hyperdex` and `hyperdex-show-config` executables.
  Evidence: `hyhac/scripts/test-with-hyperdex.sh` execs `start-hyperdex.sh`,
  and `hyhac/scripts/start-hyperdex.sh` exits unless those two binaries exist.

## Decision Log

- Decision: keep this workstream ready rather than blocked.
  Rationale: the root can launch it as soon as the current proof and harness
  edits are committed, and there is no need to wait for another file rewrite to
  make that possible.
  Date/Author: 2026-03-27 / root

## Outcomes & Retrospective

- Pending.
