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
- [x] (2026-03-27 04:45Z) Retired the second implementation thread cleanly when
  it again reported no file changes and a remaining blocker on concrete
  Replicant framing.
- [x] (2026-03-27 04:46Z) Finished the narrowed evidence steps for Replicant
  framing and dynamic packet capture.
- [x] (2026-03-27 04:56Z) Retired the third implementation thread cleanly when
  it again reported no file changes and broad implementation design as the
  blocker.
- [x] (2026-03-27 05:00Z) Retired the split admin-codec and admin-server
  workers cleanly when both dedicated worktrees still had no file changes.
- [x] (2026-03-27 05:00Z) Relaunched a pure codec worker limited to
  `hyperdex-admin-protocol` and a separate read-only server-mapping worker.
- [x] (2026-03-27 05:04Z) Finished the server map with exact file, function,
  state, and test references for the coordinator-side legacy admin path.
- [x] (2026-03-27 05:07Z) Reconciled the codec worker result into `489de25`
  (`Add legacy admin codec helpers`) with `cargo test -p hyperdex-admin-protocol`
  passing.
- [x] (2026-03-27 05:07Z) Launched one substantial server implementation step
  on top of the landed codec and the completed server map.
- [x] (2026-03-27 05:11Z) Retired that server implementation attempt when the
  admin-server worktree still had no file changes after interruption.
- [x] (2026-03-27 05:14Z) Retired the explicit-patch-target retry when the
  admin-server worktree still had no file changes after interruption.
- [x] (2026-03-27 05:14Z) Reframed the server execution shape and launched a
  forked implementation worker plus a read-only reviewer.
- [x] (2026-03-27 05:17Z) Reframed the server target after the review showed
  that the coordinator still speaks the wrong transport.
- [ ] Rerun the bounded live `hyhac` probe against that new admin frontend.

## Current Hypothesis

The first missing live contract is still the legacy coordinator admin frontend.
The newest evidence shows the immediate target is a separate BusyBee/Replicant
coordinator transport and session layer, because the current coordinator path
is still JSON control and cannot interoperate with the landed codec.

## Next Bounded Step

Implement a separate BusyBee/Replicant coordinator admin transport and session
layer: listener, frame decoding, config-follow, request-id allocation, pending
completions, `space_add`, and `wait_until_stable` loop completion.

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
- Observation: the remaining blocker is now specifically the Replicant transport
  framing, not the higher-level admin operation semantics.
  Evidence: the second implementation thread reported no touched files and
  named missing concrete Replicant framing as the only blocker.
- Observation: both admin tools send the same first 25-byte packet because they
  both start by following the coordinator `config` condition before reaching
  operation-specific traffic.
  Evidence: the dynamic-capture pass observed the same 25-byte first packet for
  both tools, and the source-level pass ties that to `maintain_coord_connection`
  issuing `replicant_client_cond_follow(..., \"hyperdex\", \"config\", ...)`.
- Observation: complete protocol evidence was still not enough for one broad
  implementation worker to start editing.
  Evidence: the third implementation thread again reported no touched files and
  named broad implementation design as the blocker.
- Observation: even after splitting writes into "codec" and "server
  integration", the workers still did not start editing.
  Evidence: both dedicated worktrees stayed clean at `801d20f` until the
  workers were interrupted.
- Observation: a read-only server-mapping pass succeeded immediately once it
  was reduced to exact insertion points and tests, while the codec retry still
  did not start editing.
  Evidence: the mapping worker returned concrete paths and functions in
  `crates/server/**`, while the codec worktree remained clean at `e3253b4`.
- Observation: the codec worker did in fact land the protocol foundation
  needed for the next server step.
  Evidence: `489de25` adds BusyBee frame helpers, Replicant request and
  response codecs, varint slice helpers, and exact protocol tests in
  `crates/hyperdex-admin-protocol/src/lib.rs`.
- Observation: the first full server implementation attempt still produced no
  code.
  Evidence: the `admin-server` worktree remained clean at `928130e` until the
  worker was interrupted.
- Observation: the explicit-patch-target retry also produced no code.
  Evidence: the `admin-server` worktree remained clean at `ee09ee0` until the
  retry worker was interrupted.
- Observation: the reviewer exposed a larger transport mismatch behind the
  empty server retries.
  Evidence: the coordinator still binds only the JSON control service, while
  the landed codec expects BusyBee framing and Replicant-style completions.

## Decision Log

- Decision: keep this workstream ready rather than blocked.
  Rationale: the root can launch it as soon as the current proof and harness
  edits are committed, and there is no need to wait for another file rewrite to
  make that possible.
  Date/Author: 2026-03-27 / root

## Outcomes & Retrospective

- Pending.
