# AutoPlan: hyperdex-rs distributed HyperDex replacement

This root AutoPlan is a living document. The sections `Progress`,
`Workstream Board`, `Current Root Focus`, `Next Root Move`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective`
must be kept current as work proceeds.

This repository does not contain its own `PLANS.md` or `AUTOPLANS.md`. This
document follows the fallback rules at
`/home/friel/.codex/skills/autoplan/references/PLANS.md` and
`/home/friel/.codex/skills/autoplan/references/AUTOPLANS.md`.

## Companion Files

- Root loop ledger: [loop-ledger-hyperdex-rs.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/loop-ledger-hyperdex-rs.md)
- Workstreams directory: [workstreams](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams)
- Completed workstreams directory: [completed](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/completed)
- Archived workstreams directory: [archived](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/archived)
- Paper notes: [papers-and-mvp-notes.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/papers-and-mvp-notes.md)
- `hyhac` compatibility notes: [hyhac-compatibility-surface.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/hyhac-compatibility-surface.md)
- Worktree inventory: [worktrees.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/worktrees.md)

## Purpose / Big Picture

`hyperdex-rs` is meant to replace HyperDex with a pure Rust system that keeps
the same public behavior for existing clients while also exposing a modern gRPC
interface. Success means three things are true at the same time: the cluster is
real rather than an in-process demo, the public compatibility surface is strong
enough to drive `hyhac`, and the proof harnesses give high confidence that the
cluster keeps the right behavior under routing, replication, and failure.

## Goal

Create a pure-Rust HyperDex replacement at
`/home/friel/c/aaronfriel/hyperdex-rs` that preserves separate coordinator and
daemon processes, exposes both a legacy HyperDex-compatible frontend and a
modern gRPC frontend, forms a real distributed cluster, and passes the `hyhac`
test suite against a live deployment.

## Evaluation Mode

Deterministic

## Acceptance Evidence

- `cargo test --workspace` passes in `hyperdex-rs`.
- The live coordinator-plus-daemons harness proves real cluster formation and
  real cross-daemon behavior.
- Deterministic proof suites exist for routing, replication, and failure
  handling, including `turmoil`, `madsim`, and Hegel-backed generated tests.
- A live `hyperdex-rs` cluster satisfies the `hyhac` harness without changing
  `hyhac` semantics.
- Paper notes remain explicit about what is in the minimum useful system and
  what is still outside it.

## Mutable Surface

- `/home/friel/c/aaronfriel/hyperdex-rs/**`
- `/home/friel/c/aaronfriel/hyhac/scripts/**` only when needed to point the
  Haskell harness at `hyperdex-rs`
- Active worktrees listed in the `Workstream Board`
- Watchdog check-ins for this effort must send an explicit parent-thread
  message on every run

## Iteration Unit

One bounded root coordination step means: reconcile the active workstream
state, ensure any substantial in-flight implementation is preregistered in the
correct workstream ledger, advance every unblocked workstream by at most one
validated step, and record the root-level judgment in the root loop ledger.

## Loop Budget

Six bounded root coordination steps before reviewing whether the workstream
split, sequencing, or validator set needs to change.

## Dispositions

- `advance`
- `retry`
- `reframe`
- `revert`
- `escalate`
- `stop`

## Pivot Rules

- If `hyhac` exercises a public operation that `hyperdex-rs` does not yet
  implement, narrow the next compatibility step to that observed operation
  instead of broad speculation.
- If deterministic proof work stops finding new failures while live-cluster
  compatibility still fails, shift effort toward the live `hyhac` workstream.
- If the multiprocess harness becomes the main source of false negatives again,
  stop expanding features until the harness is deterministic enough to trust.
- Keep public compatibility, distributed runtime behavior, and proof coverage
  separate in the workstream structure so one thread does not obscure the state
  of the others.
- For the active `live-hyhac` path, blocker-only outcomes are no longer
  acceptable when the missing capability is implementable from repository-local
  HyperDex sources. The next delegated steps must produce code, not only
  blocker reports.

## Stop Conditions

- The live `hyhac` suite passes against a real `hyperdex-rs` cluster.
- Or a hard blocker is proven with repository-local evidence and recorded in
  the root loop ledger and the affected workstream files.

## Milestones

1. Maintain a real coordinator-plus-daemons cluster with correct distributed
   control-plane and data-plane behavior.
2. Bring the legacy and gRPC public frontends up to the `hyhac` surface.
3. Keep deterministic proof coverage honest enough to trust the live-cluster
   failures that remain.
4. Reconcile worktree results back into `main` in bounded, validated steps.
5. Drive the live `hyhac` harness until the remaining semantic gaps are closed.

## Workstream Board

| Workstream | Status | Dependencies / Blockers | Plan | Ledger | Worktree / Branch | Next Step | Latest Disposition |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `simulation-proof` | ready | None | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/simulation-proof/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/simulation-proof/ledger.md) | `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/sim-coverage` on `sim-coverage-numeric` | Hold until the next live compatibility gap needs fresh deterministic coverage. | `advance` |
| `multiprocess-harness` | ready | None | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/multiprocess-harness/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/multiprocess-harness/ledger.md) | `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/dist-multiprocess-harness` | Hold until a new real-cluster failure requires deeper harness work. | `advance` |
| `live-hyhac` | active | The request core, service core, decoder hardening, and same-port startup are now on `main`, but the live coordinator still needs original-format config-condition payloads before the C admin client can complete a real probe. | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/live-hyhac/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/live-hyhac/ledger.md) | root checkout plus one active config-encoder worker | Finish binary config payload encoding, then rerun the admin probes. | `advance` |

## Progress

- [x] (2026-03-27 04:19Z) Read the updated `autoplan` skill, fallback
  `PLANS.md`, fallback `AUTOPLANS.md`, and layout guidance.
- [x] (2026-03-27 04:19Z) Confirmed the old root file existed at
  [autoplan-hyperdex-rs.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/autoplan-hyperdex-rs.md)
  and that the repository did not yet have a root loop ledger or workstream
  package.
- [x] (2026-03-27 04:19Z) Replaced the old single-file control document with
  the new root pair plus workstream files and preregistered the active
  in-flight work.
- [x] (2026-03-27 04:22Z) Reconciled the in-flight `simulation-proof` edit
  into `6d55620` (`Add Hegel routed numeric mutation coverage`) with green
  targeted, package, and workspace validators.
- [x] (2026-03-27 04:22Z) Reconciled the in-flight `multiprocess-harness` edit
  into `98def36` (`Stabilize multiprocess harness concurrency`) with green
  harness and workspace validators.
- [x] (2026-03-27 04:33Z) Launched the next three unblocked steps in parallel
  and reconciled all three code results onto `main`:
  `329a469`, `5cc0cf8`, and `faa6cb6`.
- [x] (2026-03-27 04:33Z) Advanced `live-hyhac` far enough to isolate the next
  compatibility gap from observed failures: the live probe reaches the
  coordinator admin path, the coordinator now survives malformed connections,
  and the remaining blocker is legacy admin protocol compatibility.
- [x] (2026-03-27 04:39Z) Retired the first legacy-admin implementation thread
  without code changes because it did not yet have enough verified wire detail
  to implement the protocol safely.
- [x] (2026-03-27 04:41Z) Finished the read-only protocol evidence pass for the
  original HyperDex admin client path and recovered the concrete control-flow
  facts needed to reopen implementation safely.
- [x] (2026-03-27 04:45Z) Retired the second implementation thread cleanly when
  it again reported no file changes and a remaining blocker on concrete
  Replicant framing.
- [x] (2026-03-27 04:46Z) Finished the narrowed evidence steps for Replicant
  framing and dynamic packet capture.
- [x] (2026-03-27 04:56Z) Retired the third implementation thread cleanly when
  it again reported no file changes and identified broad implementation design
  as the blocker.
- [x] (2026-03-27 05:00Z) Retired the split admin-codec and admin-server
  workers cleanly when they still produced no file changes in their dedicated
  worktrees.
- [x] (2026-03-27 05:00Z) Relaunched the live-compatibility implementation as
  two tighter delegated steps: a pure codec implementation and a read-only
  server map that names the exact listener and session hooks to wire once the
  codec exists.
- [x] (2026-03-27 05:04Z) Finished the server wiring map with exact
  coordinator listener, session-state, and test insertion points.
- [x] (2026-03-27 05:07Z) Reconciled the codec worker result into `489de25`
  (`Add legacy admin codec helpers`) with `cargo test -p hyperdex-admin-protocol`
  passing.
- [x] (2026-03-27 05:07Z) Launched one substantial server implementation step
  using the landed codec and the completed server map.
- [x] (2026-03-27 05:11Z) Retired that server implementation step when the
  admin-server worktree still had no diff after interruption.
- [x] (2026-03-27 05:14Z) Retired the explicit-patch-target retry when the
  admin-server worktree still had no diff after interruption.
- [x] (2026-03-27 05:14Z) Reframed the server execution shape and launched a
  forked implementation worker plus a read-only reviewer.
- [x] (2026-03-27 05:17Z) Reframed the server target itself after the review
  showed that the current coordinator transport is JSON and incompatible with
  the landed codec.
- [x] (2026-03-27 05:21Z) Retired the first corrected-transport attempt with a
  precise blocker report but no diff.
- [x] (2026-03-27 05:24Z) Retired the first service-core attempt with a more
  precise blocker report: the original packed `space_add` payload still has no
  Rust decoder.
- [x] (2026-03-27 05:24Z) Raised the execution bar for `live-hyhac`: no more
  blocker-only iterations when the protocol can be implemented from the local
  HyperDex sources.
- [x] (2026-03-27 05:27Z) Recovered the exact `space` pack/unpack shape from
  `common/hyperspace.cc` and the caller path in `admin/admin.cc`.
- [x] (2026-03-27 05:39Z) Landed `df633ac` (`Decode packed legacy admin space
  requests`), which ports the packed `hyperdex::space` decoder, maps
  Replicant admin requests into coordinator requests, and emits real Replicant
  completions for `space_add`, `space_rm`, and `wait_until_stable`.
- [x] (2026-03-27 05:48Z) Reconciled `78162d5` (`Add legacy coordinator admin
  service core`) after resolving the interrupted cherry-pick, added `f26d042`
  (`Add config-follow bootstrap helper`), and restored a green workspace with
  `cargo test -p server` and `cargo test --workspace`.
- [x] (2026-03-27 05:48Z) Relaunched two substantial follow-up steps in
  parallel: coordinator startup plus live admin probes, and selective
  decoder-hardening based on the richer admin-decoder worktree result.
- [x] (2026-03-27 05:58Z) Reconciled `007bdf1` (`Harden packed admin space
  decoding`), which ports the missing packed-space validation and richer tests
  into `hyperdex-admin-protocol` while keeping the current public entry points.
- [x] (2026-03-27 06:02Z) Reconciled `99d3922` (`Serve coordinator legacy
  admin on the public port`), which proves same-port coordinator dispatch but
  also proves the original admin tools still time out afterward.
- [ ] Rerun the bounded live `hyhac` probe after that admin frontend lands.

## Current Root Focus

Drive the remaining live coordinator compatibility gap now that the request
core, service core, decoder hardening, and same-port startup are on `main`.
The one active implementation job is binary config payload encoding. The next
integration should turn that into a real admin-tool probe result.

## Next Root Move

Reconcile the binary `config` condition payload encoder, then rerun
`hyperdex-add-space` and `hyperdex-wait-until-stable`.

## Surprises & Discoveries

- Observation: the repository already had the correct root AutoPlan path at
  [autoplan-hyperdex-rs.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/autoplan-hyperdex-rs.md),
  but it was still carrying both root-control content and detailed iteration
  history in one file.
  Evidence: the file existed and still contained the large historical loop
  table when reread at `2026-03-27 04:19Z`.
- Observation: the root checkout is not clean; it already contains one proof
  edit and one harness edit that both passed validation before the pause.
  Evidence: `git status --short` showed edits in `Cargo.toml`,
  `crates/server/Cargo.toml`, `crates/server/tests/dist_multiprocess_harness.rs`,
  and `crates/simulation-harness/src/lib.rs`.
- Observation: the repository is back to a clean code state after the two
  bounded integrations, so the next useful root action is a live compatibility
  probe rather than more local cleanup.
  Evidence: `6d55620` and `98def36` are now on `main`, and `cargo test --workspace`
  passed after both landed.
- Observation: the first live compatibility blocker is the missing legacy
  coordinator admin frontend, not simulation coverage or multiprocess startup.
  Evidence: the direct `hyhac` probe timed out against a live cluster, the
  coordinator stayed alive after `329a469`, and a direct
  `hyperdex-add-space` invocation also timed out against port `1982`.
- Observation: the first legacy-admin implementation thread stopped at the
  right boundary and made no code changes because the original admin wire
  protocol is Replicant-backed and still under-specified in the current Rust
  repo.
  Evidence: the retired worker reported no file changes and identified the
  missing verified wire detail as the exact blocker.
- Observation: the original HyperDex sources now provide enough verified detail
  to reopen the admin implementation safely.
  Evidence: the read-only protocol pass confirmed the Replicant-backed flow for
  `space_add` and `wait_until_stable`, the two-byte coordinator return-code
  mapping, and the request-id-plus-loop completion contract.
- Observation: that protocol evidence was still not specific enough for a
  worker to code the wire compatibility layer without guessing the Replicant
  transport framing.
  Evidence: the second implementation thread again reported no touched files
  and named missing concrete Replicant framing as the blocker.
- Observation: the remaining transport ambiguity is gone.
  Evidence: the delegated evidence steps recovered both the BusyBee size-header
  framing and the Replicant request and response layouts, and the dynamic
  capture confirmed that both admin tools first emit the same 25-byte
  bootstrap packet before any operation-specific traffic.
- Observation: that earlier interpretation of the first 25-byte packet was too
  coarse; it is Replicant bootstrap, not the HyperDex `config` follow itself.
  Evidence: `Replicant/common/bootstrap.cc` emits `REPLNET_BOOTSTRAP` as the
  single-byte BusyBee payload `0x1c`, and `Replicant/client/client.cc` handles
  `REPLNET_BOOTSTRAP` as the bootstrap-install path before condition-follow
  traffic proceeds.
- Observation: the packed-space and request-core gap is now closed.
  Evidence: `df633ac` adds `decode_packed_hyperdex_space`,
  `ReplicantAdminRequestMessage::into_coordinator_request`, focused protocol
  tests, and `handle_replicant_admin_request`, with `cargo test --workspace`
  passing afterward.
- Observation: even with complete framing evidence, a single worker still did
  not start code changes.
  Evidence: the third implementation thread reported no touched files and named
  broad implementation design as the blocker.
- Observation: splitting the work into "codec" and "server integration" was
  still not specific enough to force either worker into a concrete diff.
  Evidence: both dedicated worktrees stayed clean at `801d20f` until the
  workers were interrupted.
- Observation: a read-only server-mapping pass succeeded immediately once it
  was reduced to exact file, function, and test references, while the codec
  worker still did not start editing.
  Evidence: the mapping worker returned concrete insertion points across
  `crates/server/**`, while the codec worktree remained clean at `e3253b4`.
- Observation: the codec worker did produce a large useful result once it
  committed before the final status check.
  Evidence: `489de25` landed BusyBee framing helpers, Replicant admin message
  codecs, varint slice helpers, and targeted protocol tests in
  `crates/hyperdex-admin-protocol/src/lib.rs`.
- Observation: even after the codec and server map existed, the first full
  server implementation worker still produced no diff.
  Evidence: the `admin-server` worktree remained clean at `928130e` until the
  worker was interrupted.
- Observation: even an explicit-patch-target retry stayed empty.
  Evidence: the `admin-server` worktree remained clean at `ee09ee0` until the
  retry worker was interrupted.
- Observation: the execution shape is now different from the failed retries.
  Evidence: the active `hyh-014` relaunch uses a forked implementation worker
  plus a parallel read-only reviewer, and the `admin-server` worktree is
  fast-forwarded to `2641a75`.
- Observation: the bigger mismatch is transport, not only missing session
  state.
  Evidence: the reviewer showed that the coordinator still binds only the JSON
  `CoordinatorControlService`, while the landed codec expects BusyBee framing
  and Replicant-style request and completion messages.
- Observation: even on the corrected transport target, the next worker still
  returned only the same structural blocker and no patch.
  Evidence: the `admin-server` worktree remained clean at `175ed25`, and the
  worker reported that the missing piece is a new coordinator transport/service
  layer in `main.rs` and `lib.rs`.
- Observation: the service-core target exposed an even smaller concrete blocker.
  Evidence: the latest worker reported that `ReplicantAdminRequestMessage::space_add`
  carries opaque bytes and the server has no Rust decoder for the original
  packed `hyperdex::space` payload format.
- Observation: the live admin service core is now on `main`, and the current
  remaining gap moved outward to startup wiring and config payload fidelity.
  Evidence: `78162d5` adds `CoordinatorAdminLegacyService` and its focused
  tests pass inside `cargo test -p server`; the full workspace is green again
  after `f26d042`, while the active follow-up work is now coordinator startup
  plus live probes and selective decoder validation.
- Observation: the original C admin client does not accept the current JSON
  config-follow payload, and the public coordinator port still binds only the
  JSON control service on `main`.
  Evidence: the recovered HyperDex source path shows `config` follow data is a
  packed `hyperdex::configuration` binary payload, while
  `default_legacy_config_encoder` still uses `serde_json::to_vec(view)` and
  `crates/server/src/main.rs` still binds only `CoordinatorControlService` on
  the public coordinator port.
- Observation: the packed-space decoder now has the missing validation and
  richer tests on `main`.
  Evidence: `007bdf1` restores secret-attribute validation, partition-count
  validation, index validation, contextual truncation checks, and richer
  packed-space fixtures in `crates/hyperdex-admin-protocol/src/lib.rs`.
- Observation: same-port coordinator dispatch is now working, but it did not
  by itself unblock the original C admin tools.
  Evidence: `99d3922` adds public-port dispatch between JSON control traffic
  and legacy admin traffic, its tests pass, and bounded `hyperdex-add-space`
  plus `hyperdex-wait-until-stable` probes still timed out against the live
  listener afterward.

## Decision Log

- Decision: adopt the full root-pair-plus-workstreams layout instead of trying
  to keep the old single-file structure.
  Rationale: the effort already has at least three independently advancing
  threads, multiple worktrees, and in-flight implementation in more than one
  surface. The updated `autoplan` rules require that structure to be explicit.
  Date/Author: 2026-03-27 / root
- Decision: keep `simulation-proof`, `multiprocess-harness`, and `live-hyhac`
  as the three active workstreams.
  Rationale: they correspond to the three immediate truths the root must manage
  separately: proof strength, multiprocess validator reliability, and the live
  compatibility objective the user actually cares about.
  Date/Author: 2026-03-27 / root

## Outcomes & Retrospective

- Pending. This effort is still active.
