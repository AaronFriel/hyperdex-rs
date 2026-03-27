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
- [x] (2026-03-27 04:33Z) Ran the bounded live `hyhac` probe against a real
  `hyperdex-rs` coordinator-plus-daemon cluster and confirmed the first block
  is the coordinator admin path.
- [x] (2026-03-27 04:33Z) Narrowed the next compatibility change to the legacy
  coordinator admin frontend, with `add_space` and `wait_until_stable` as the
  first required operations.
- [x] (2026-03-27 04:39Z) Retired the first implementation thread cleanly when
  it reported no file changes and an explicit blocker on missing verified wire
  detail for the original admin protocol.
- [x] (2026-03-27 04:41Z) Finished the read-only protocol evidence pass for the
  original HyperDex admin path and recovered the concrete control-flow facts
  needed to reopen implementation.
- [ ] Implement the verified Replicant-compatible legacy coordinator admin
  behavior in the dedicated control-plane worktree.
- [ ] Rerun the bounded live `hyhac` probe against that new admin frontend.

## Current Hypothesis

The first missing live contract is still the legacy coordinator admin frontend,
and the verified protocol facts now make the next step concrete: implement
Replicant-compatible `space_add`, `wait_until_stable`, and request-id-plus-loop
completion behavior closely enough for the original C admin client path.

## Next Bounded Step

Implement the smallest legacy coordinator admin behavior that matches the
verified Replicant control flow for `space_add`, `wait_until_stable`, and
`hyperdex_admin_loop`, then rerun the bounded live probe.

## Surprises & Discoveries

- Observation: `scripts/test-with-hyperdex.sh` cannot drive `hyperdex-rs`
  directly because it shells through `start-hyperdex.sh`, which requires the
  original `hyperdex` and `hyperdex-show-config` executables.
  Evidence: `hyhac/scripts/test-with-hyperdex.sh` execs `start-hyperdex.sh`,
  and `hyhac/scripts/start-hyperdex.sh` exits unless those two binaries exist.
- Observation: after `329a469`, the coordinator survives malformed admin
  connections, but the live probe still times out on the legacy admin path.
  Evidence: the bounded direct Cabal probe timed out after `30s`, and a direct
  `hyperdex-add-space` invocation also timed out against `127.0.0.1:1982`.
- Observation: the first implementation thread on the legacy admin frontend
  stopped without code changes because the original wire behavior is not yet
  concrete enough to implement safely.
  Evidence: the retired worker reported no touched files and named the missing
  verified wire detail as the exact blocker.
- Observation: the verified protocol pass shows that the immediate target is a
  Replicant-compatible coordinator path, not the BusyBee admin header or the
  JSON control listener.
  Evidence: the original sources route `space_add` through
  `replicant_client_call`, route `wait_until_stable` through
  `replicant_client_cond_wait`, and complete both through
  `hyperdex_admin_loop`.

## Decision Log

- Decision: keep this workstream ready rather than blocked.
  Rationale: the root can launch it as soon as the current proof and harness
  edits are committed, and there is no need to wait for another file rewrite to
  make that possible.
  Date/Author: 2026-03-27 / root

## Outcomes & Retrospective

- Pending.
