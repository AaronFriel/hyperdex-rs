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
- Blocker-only outcomes are no longer acceptable here when the missing
  capability is implementable from the local HyperDex sources.

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
- [x] (2026-03-27 05:21Z) Retired the first corrected-transport attempt when
  it still produced no diff, but captured a precise blocker report.
- [x] (2026-03-27 05:24Z) Retired the first service-core attempt when it
  exposed a smaller concrete blocker: no Rust decoder for the packed
  `space_add` payload.
- [x] (2026-03-27 05:24Z) Raised the execution bar: the next delegated steps
  must implement the protocol rather than report more blockers.
- [x] (2026-03-27 05:27Z) Recovered the exact `space` pack/unpack shape from
  the original HyperDex C++ sources.
- [x] (2026-03-27 05:39Z) Landed `df633ac` (`Decode packed legacy admin space
  requests`), which ports the packed `hyperdex::space` decoder, maps Replicant
  admin requests into coordinator requests, and emits real Replicant
  completions for `space_add`, `space_rm`, and `wait_until_stable`.
- [x] (2026-03-27 05:48Z) Landed `78162d5` (`Add legacy coordinator admin
  service core`) and `f26d042` (`Add config-follow bootstrap helper`), giving
  `main` a BusyBee/Replicant session core with focused tests and a green
  workspace.
- [x] (2026-03-27 05:48Z) Relaunched the next two concrete follow-ups in
  parallel: coordinator startup plus bounded live probes, and selective
  decoder hardening from the richer admin-decoder worktree result.
- [x] (2026-03-27 05:58Z) Narrowed the remaining live admin gap further:
  same-port coordinator startup, binary `config` condition payload encoding,
  and a clean retry of decoder hardening are now active as separate bounded
  implementation jobs.
- [x] (2026-03-27 05:58Z) Reconciled `007bdf1` (`Harden packed admin space
  decoding`), which ports the missing packed-space validation and richer tests
  into the existing decoder path on `main`.
- [x] (2026-03-27 06:02Z) Reconciled `99d3922` (`Serve coordinator legacy
  admin on the public port`), which proves same-port public-port dispatch but
  still leaves the original admin tools timing out.
- [ ] Rerun the bounded live `hyhac` probe against that new admin frontend.

## Current Hypothesis

The request core, session core, packed-space decoder hardening, and same-port
startup are now on `main`. The remaining live contract is down to one
implementation job: the `config` follow payload still needs the original
binary format.

## Next Bounded Step

Reconcile the binary `config` follow payload encoder, then rerun the bounded
`hyperdex-add-space` and
`hyperdex-wait-until-stable` probes, followed by the direct `hyhac` Cabal
test if those probes advance.

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
  both start with Replicant bootstrap before reaching HyperDex-specific
  condition follows or operation traffic.
  Evidence: the dynamic-capture pass observed the same 25-byte first packet for
  both tools, and `Replicant/common/bootstrap.cc` plus
  `Replicant/client/client.cc` show that `0x1c` is the bootstrap request that
  installs the coordinator set before condition follows proceed.
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
- Observation: the first worker on the corrected transport target still did not
  patch the service core.
  Evidence: the `admin-server` worktree remained clean at `175ed25`, and the
  blocker report pointed back to the missing coordinator transport/service
  layer in `crates/server/src/lib.rs`.
- Observation: the next service-core attempt exposed one concrete missing
  capability beneath that broader blocker.
  Evidence: `ReplicantAdminRequestMessage::space_add` still carries opaque
  bytes, and the server has no decoder for the original packed
  `hyperdex::space` format.
- Observation: the exact binary format is now available from source, not just
  inferred from symptoms.
  Evidence: `HyperDex/common/hyperspace.cc` contains both `operator <<` and
  `operator >>` for `hyperdex::space`, and `HyperDex/admin/admin.cc` shows
  `space_add` packing with `msg->pack_at(0) << space`.
- Observation: the first 25-byte packet from the original admin tools is
  Replicant bootstrap, not the HyperDex `config` follow itself.
  Evidence: `Replicant/common/bootstrap.cc` packs `REPLNET_BOOTSTRAP` as a
  single-byte BusyBee payload `0x1c`, and `Replicant/client/client.cc` treats
  `REPLNET_BOOTSTRAP` as the special message that installs the coordinator set
  before any condition follows are processed.
- Observation: the service-core portion of that session layer is no longer
  hypothetical; it is on `main` and validated locally.
  Evidence: `78162d5` adds `CoordinatorAdminLegacyService`, focused server
  tests cover bootstrap, `space_add`, and `wait_until_stable`, and
  `cargo test --workspace` passed after `f26d042`.
- Observation: the `stable` condition payload is already compatible, but the
  `config` condition payload is not.
  Evidence: the recovered HyperDex source path shows `stable` completes with
  an empty `e::slice` payload, which Rust already emits, while `config`
  follow data is a packed `hyperdex::configuration` binary payload and the
  current Rust encoder still sends JSON.
- Observation: the public coordinator port handling is still wrong for a live
  C admin client.
  Evidence: `crates/server/src/main.rs` still binds only
  `CoordinatorControlService` on the coordinator port, while the legacy admin
  path exists only as `CoordinatorAdminLegacyService` in `crates/server/src/lib.rs`.
- Observation: the decoder-hardening retry succeeded once it was kept on a
  narrow write scope.
  Evidence: `007bdf1` landed on `main` from the clean retry and avoided the
  earlier unrelated edits across other crates.
- Observation: the startup path is now past the public-port binding problem.
  Evidence: `99d3922` dispatches accepted public-port connections between the
  JSON control path and the legacy admin path, and a focused test keeps a
  legacy `config_follow` connection open while a JSON `space_add` succeeds on
  the same port.
- Observation: after the startup fix, the remaining live admin timeout points
  downstream of accept/dispatch.
  Evidence: bounded `hyperdex-add-space` and `hyperdex-wait-until-stable`
  probes still timed out against the live listener after `99d3922`.

## Decision Log

- Decision: keep this workstream ready rather than blocked.
  Rationale: the root can launch it as soon as the current proof and harness
  edits are committed, and there is no need to wait for another file rewrite to
  make that possible.
  Date/Author: 2026-03-27 / root

## Outcomes & Retrospective

- Pending.
