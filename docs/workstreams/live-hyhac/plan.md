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
surface. The current owner should treat this as a large fork-owned step rather
than a short patch attempt: use the fastest useful checks first, then rerun the
stronger live probe before returning control.

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
- [x] (2026-03-27 06:08Z) Reconciled `0d8d566` (`Pack legacy coordinator
  config follow payload`), which puts the original binary config payload
  format on `main`.
- [x] (2026-03-27 06:19Z) Ran the next live probe far enough to isolate the
  next concrete runtime failure: daemon startup against the public coordinator
  port exits with `Error: early eof` before any admin tool runs.
- [x] (2026-03-27 06:24Z) Rechecked that runtime failure on clean `main` and
  found it was not stable: daemon join succeeded on a free-port cluster, but
  both original admin tools still timed out afterward.
- [x] (2026-03-27 06:29Z) Captured the exact first post-join timeout surface:
  the original admin client stops after the first `config` follow completion
  and never sends `space_add` or `wait_until-stable`.
- [x] (2026-03-27 06:35Z) Narrowed that wire result one step further: the
  first Rust reply is not merely the wrong `config` completion shape, it is
  the wrong Replicant frame type for the bootstrap path.
- [x] (2026-03-27 07:10Z) Reframed the next step around larger fork ownership:
  one worker should own the bootstrap fix end to end with its own fast checks
  and live probe loop, not hand back another tiny partial result.
- [x] (2026-03-27 07:20Z) The fast proxy loop is now on `main`:
  `6f061b3` adds a focused admin bootstrap-progress harness test that reports
  `advanced=false` on current `main`.
- [x] (2026-03-27 07:35Z) Reconciled `c087f81` (`Fix legacy admin bootstrap
  and packed space decoding`), which gets the original C admin client through
  bootstrap, `wait_until_stable`, `show-config`, and `add-space`.
- [x] (2026-03-27 07:35Z) The next live blocker is now in the legacy daemon
  data path: the first pooled roundtrip and richer client operations fail with
  `Left ClientGarbage` instead of an admin/bootstrap failure.
- [x] (2026-03-27 07:45Z) The faster daemon-path repro is now on `main`:
  `0b2379d` shows `*Can store a large object*` is enough to hit
  `Left ClientGarbage`.
- [x] (2026-03-27 20:04Z) Reframed the daemon-path blocker again: the focused
  large-object failure still happens before the daemon sees `REQ_ATOMIC`, so
  the next product target is the packed coordinator config and client-side
  request-preparation contract for the full `profiles` schema.
- [x] (2026-03-27 20:16Z) Reconciled `be0cb38` (`Align legacy config and
  daemon protocol encoding`), which fixes string-slice encoding for the packed
  config path and restores the correct legacy datatype codes across the full
  `profiles` schema.
- [x] (2026-03-27 20:20Z) Reframed `hyh-035` to the exact next packed-config
  fix: replace singleton primary-subspace region bounds with the original
  contiguous `hyperdex::partition(...)` hash intervals.
- [x] (2026-03-27 20:28Z) Reconciled `1d6093c` (`Use HyperDex partition
  intervals in legacy config`), verified the focused interval test, and
  confirmed that the fast large-object public loop still reproduces
  `Left ClientGarbage`.
- [x] (2026-03-27 20:31Z) The next read-only comparison identified the next
  exact packed-config mismatch after region intervals: zero-based ID
  allocation, especially `virtual_server_id=0`, where the original
  coordinator uses nonzero IDs from a shared counter.
- [x] (2026-03-27 20:36Z) The follow-up tie-off proved that the concrete
  failing key `"large"` already routes to a non-null replica tuple on current
  `main`, so the remaining blocker is later than coordinator route selection.
- [ ] Rerun the bounded live `hyhac` probe after the next packed-config/body
  mismatch is fixed.

## Current Hypothesis

The request core, session core, packed-space decoder hardening, same-port
startup, binary config encoding, daemon join, and coordinator bootstrap/admin
compatibility are now on `main`. `1d6093c` also fixes the first concrete
packed-config mismatch by replacing singleton primary-subspace region bounds
with HyperDex partition intervals. The focused large-object failure is still
the right public loop, but it still does not point at the daemon request
decoder: the daemon never sees `REQ_ATOMIC` for this path, and the harness
still shows the first captured exchange on the coordinator connection. The
next concrete gap is therefore deeper inside the packed
`hyperdex::configuration` / `hyperdex::space` body after region interval
correction. The next proven bad field is now the ID-allocation contract:
Rust emits zero-based `space_id`, `subspace_id`, `region_id`, and especially
`virtual_server_id=0`, while the original coordinator allocates those IDs from
one shared counter seeded at `1`. But the concrete failing key `"large"`
already routes to a non-null replica tuple on current `main`, so the active
public blocker is now one step later than route selection.

## Next Bounded Step

Own the next packed `hyperdex::configuration` / `hyperdex::space` body fix for
the full `profiles` schema through the focused large-object `ClientGarbage`
repro now that region intervals are corrected. Use the ID-allocation mismatch
as a correctness fix if it is already in flight, but do not stop there: the
concrete failing key is already past route selection, so drive until the next
exact pre-daemon mismatch is exposed or fixed. Stay on the fast public loop
until that path either clears or yields that next exact coordinator-side
contract.

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
- Observation: the focused large-object failure still does not reach the daemon
  atomic handler.
  Evidence: on a manual live cluster with the full `profiles` schema,
  temporary `ReqAtomic` tracing inside `handle_legacy_request` never fired
  while the focused `hyhac` large-object selection still reproduced
  `Left ClientGarbage`.
- Observation: the first captured exchange on that failing path is still on the
  coordinator connection rather than a decodable legacy daemon frame.
  Evidence: `853e290` adds
  `legacy_hyhac_large_object_probe_reports_first_coordinator_frame_pair`, and
  that test captures partial BusyBee-style frames with `trailing_bytes=45` and
  `trailing_bytes=100` on the coordinator path.
- Observation: correcting string-slice encoding and legacy datatype codes moved
  the packed config contract forward, but did not clear the first public
  `ClientGarbage` failure.
  Evidence: `be0cb38` now preserves the expected legacy datatype codes across
  the full `profiles` schema and keeps focused config tests green, while the
  shared fast large-object repro still fails.
- Observation: the first remaining packed-config mismatch is specifically the
  primary-subspace region bounds.
  Evidence: the completed `cce-002` comparison shows Rust emits
  `lower=partition`, `upper=partition`, while the original HyperDex builder
  emits contiguous `hyperdex::partition(...)` intervals such as
  `upper=0x03ffffffffffffff` for the first primary region.
- Observation: correcting the primary-subspace region bounds is necessary but
  not sufficient for the focused large-object path.
  Evidence: `1d6093c` replaces the singleton bounds with HyperDex partition
  intervals, `legacy_partition_regions_cover_full_u64_space_for_single_dimension`
  passes, and the fast public loop still reports `Left ClientGarbage`.
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
- Observation: the current Rust coordinator answers that first bootstrap with
  the wrong Replicant message type, so the C admin client never reaches the
  later `config`-follow or admin-operation path.
  Evidence: the captured 88-byte reply decodes as
  `REPLNET_CLIENT_RESPONSE`, while `/home/friel/HyperDex/Replicant/client/client.cc`
  expects `REPLNET_BOOTSTRAP` on the first successful receive for that path.
- Observation: the previous ownership shape was too granular for the updated
  `autoplan` rules.
  Evidence: the prior step stopped after narrowing the mismatch, but no forked
  worker stayed in control long enough to both patch the bootstrap reply and
  drive the live probe loop to the next concrete result.
- Observation: the faster proxy loop now reproduces the current bug from a
  single test target on `main`.
  Evidence: `6f061b3` adds
  `legacy_admin_wait_until_stable_probe_reports_bootstrap_progress`, and it
  reports `advanced=false` with `first_server=ClientResponse`.
- Observation: coordinator bootstrap/admin success does not imply daemon-path
  compatibility yet.
  Evidence: after `c087f81`, the admin tools succeed, but the selected `hyhac`
  run now fails in pooled roundtrip and richer client operations with
  `Left ClientGarbage`.
- Observation: the first daemon-path public failure is already narrowed to a
  much smaller subset than the selected `hyhac` command.
  Evidence: `0b2379d` shows `*Can store a large object*` reproduces
  `Left ClientGarbage` in the multiprocess harness.
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
- Observation: the binary `config` follow payload path is no longer only a
  hypothesis; it is implemented on `main`.
  Evidence: `0d8d566` packs `hyperdex::configuration` bytes in
  `default_legacy_config_encoder`, and the worker reported green `server` and
  workspace tests after that change.
- Observation: the next live failure is not in `hyperdex-add-space` yet.
  Evidence: the fresh free-port probe reached a listening coordinator on
  `19830`, but the daemon exited during registration with `Error: early eof`,
  so the admin tools were never able to run in that cluster instance.
- Observation: that daemon-registration failure was not stable enough to stay
  the active blocker.
  Evidence: a fresh clean-main rerun reached a live coordinator and a live
  daemon on free ports, but both `hyperdex-add-space` and
  `hyperdex-wait-until-stable` still timed out afterward.
- Observation: the original admin client never reaches the operation-specific
  request path.
  Evidence: the captured wire shows one 25-byte Replicant bootstrap request
  and one 88-byte Rust completion response, after which the client sends no
  second request before timing out.

## Decision Log

- Decision: keep this workstream ready rather than blocked.
  Rationale: the root can launch it as soon as the current proof and harness
  edits are committed, and there is no need to wait for another file rewrite to
  make that possible.
  Date/Author: 2026-03-27 / root
- Decision: relaunch the next product-owned step from a new clean worktree.
  Rationale: the old `live-hyhac-data-plane` worktree still carries an
  unrelated uncommitted harness edit, and the next fork should own a clean
  write surface while keeping the same fast validator.
  Date/Author: 2026-03-27 / root

## Outcomes & Retrospective

- `be0cb38` and `1d6093c` advanced the packed coordinator-config contract in
  two concrete steps: first by fixing string-slice and datatype encoding, then
  by fixing primary-region interval encoding. The public `ClientGarbage`
  failure is now narrower than before, but the next product step should stay
  on the packed `configuration` / `space` body rather than returning to the
  daemon request path.
