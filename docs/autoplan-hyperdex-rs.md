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

| Workstream | Status | Owner | Dependencies / Blockers | Plan | Ledger | Worktree / Branch | Fastest Useful Check | Next Step | Latest Disposition |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `simulation-proof` | ready | root | None | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/simulation-proof/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/simulation-proof/ledger.md) | `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/sim-coverage` on `sim-coverage-numeric` | `cargo test -p simulation-harness` | Hold until the next live compatibility gap needs fresh deterministic coverage. | `advance` |
| `multiprocess-harness` | ready | root | None; `69d5918` already proved the fast Hyhac failure loops only through coordinator identify/bootstrap traffic on the cleaned baseline, so this workstream can pause again until another harness change is justified. | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/multiprocess-harness/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/multiprocess-harness/ledger.md) | `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/clientgarbage-wire` on `clientgarbage-wire` | `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reports_coordinator_busybee_sequence -- --nocapture` | Hold until product or read-only comparison work needs another harness change. | `advance` |
| `live-hyhac` | active | root in `live-hyhac-large-object` | `5879fab` removed the multiprocess `early eof` failures, and the sender-id plumbing is now landed and validated, but the focused large-object path still never progresses beyond coordinator bootstrap. | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/live-hyhac/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/live-hyhac/ledger.md) | `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-large-object` on `live-hyhac-large-object` | `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reports_coordinator_busybee_sequence -- --nocapture` | Reduce the still-bootstrap-only failure to the next exact non-wire bootstrap acceptance mismatch, not the broader daemon path. | `reframe` |
| `coordinator-config-evidence` | active | next forked read-only worker | None; the exact blocker is now known to be the Replicant bootstrap sender-identity contract, so this workstream now owns the read-only implementation map for that fix. | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/coordinator-config-evidence/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/coordinator-config-evidence/ledger.md) | none required | `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reports_coordinator_busybee_sequence -- --nocapture` | Map the exact Rust patch points and proving tests for the bootstrap sender-identity fix. | `advance` |

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
- [x] (2026-03-27 06:08Z) Reconciled `0d8d566` (`Pack legacy coordinator
  config follow payload`), which replaces the JSON config-follow payload with
  the packed `hyperdex::configuration` binary layout and keeps the test suite
  green.
- [x] (2026-03-27 06:19Z) Ran the next live-cluster probe far enough to
  isolate the next concrete failure: daemon startup against the public
  coordinator port exits with `Error: early eof` before any admin tool runs.
- [x] (2026-03-27 06:24Z) Rechecked that failure on clean `main` and found it
  does not reproduce: daemon join succeeds on a free-port cluster, but
  `hyperdex-add-space` and `hyperdex-wait-until-stable` still time out.
- [x] (2026-03-27 06:29Z) Captured the first concrete post-join timeout
  surface: both original admin tools send only the 25-byte Replicant
  bootstrap, receive one 88-byte `config` follow completion, and then never
  send a second request.
- [x] (2026-03-27 06:35Z) Narrowed that timeout one step further: the first
  Rust reply is not only the wrong payload shape, it is the wrong Replicant
  frame type. The C admin client expects `REPLNET_BOOTSTRAP` there and never
  advances after seeing `REPLNET_CLIENT_RESPONSE` instead.
- [x] (2026-03-27 07:10Z) Reread the refreshed `autoplan` references and
  updated the effort to favor larger fork-owned work with explicit fast proxy
  validators instead of single short-lived implementation passes.
- [x] (2026-03-27 07:15Z) Rearmed the watchdog, launched two larger fork-owned
  workstreams in parallel, and let them run long enough to manage their own
  loops.
- [x] (2026-03-27 07:20Z) Reconciled `6f061b3` (`Add legacy admin bootstrap
  probe harness`), which puts a fast live-admin progress check on `main` while
  the product worker continues on the bootstrap fix.
- [x] (2026-03-27 07:35Z) Reconciled `c087f81` (`Fix legacy admin bootstrap
  and packed space decoding`), which gets the original C admin client past
  bootstrap and through real admin operations on `main`.
- [x] (2026-03-27 07:35Z) Verified the integrated server package is green with
  `cargo test -p server`, and moved the remaining blocker from coordinator
  bootstrap to the legacy daemon request/response path reached by `hyhac`.
- [x] (2026-03-27 07:45Z) Reconciled `0b2379d` (`Add fast hyhac ClientGarbage
  repro probes`), which reduces the first public daemon-path failure to the
  focused `*Can store a large object*` `hyhac` subset on `main`.
- [x] (2026-03-27 07:50Z) Reopened the harness workstream immediately so the
  daemon-path fix still has two active owners: one on product code, one on
  wire-level repro evidence around the new fast large-object failure.
- [x] (2026-03-27 19:49Z) Retired the first wire-capture harness retry after
  the interrupted worker left unrelated product files dirty in the old
  worktree, then preregistered a fresh harness owner on a clean replacement
  worktree.
- [x] (2026-03-27 19:52Z) Rearmed the watchdog, relaunched the replacement
  harness owner on `clientgarbage-wire`, and confirmed the short large-object
  repro plus `cargo test -p server` both still return the expected signal on
  `main`.
- [x] (2026-03-27 19:54Z) Retired the clean harness retry after it returned
  only the existing baseline, then reopened the same workstream with a stricter
  harness-only requirement to expose or decode the first bad daemon frame.
- [x] (2026-03-27 20:02Z) Reconciled `853e290` (`Capture large-object
  clientgarbage coordinator frames`), which proves the focused large-object
  path still fails on the coordinator connection before the harness sees a
  decodable legacy daemon frame.
- [x] (2026-03-27 20:04Z) Reframed the product target from the daemon atomic
  handler to the packed coordinator config and client-side request-preparation
  contract for the full `profiles` schema, based on the product worker’s
  direct live-cluster probe and the new harness capture.
- [x] (2026-03-27 20:43Z) Finished the first read-only coordinator-config
  evidence step and proved the captured coordinator frames are healthy BusyBee
  identify plus Replicant bootstrap traffic, not the active mismatch.
- [x] (2026-03-27 20:14Z) Reopened the coordinator-config evidence workstream
  for a second read-only step: direct comparison of the Rust packed-config body
  against the original HyperDex `configuration` / `space` packing rules on a
  live `profiles` config body.
- [x] (2026-03-27 20:16Z) Reconciled `be0cb38` (`Align legacy config and
  daemon protocol encoding`), which corrects string-slice encoding and legacy
  datatype codes across the full `profiles` schema but still leaves the fast
  large-object public loop failing.
- [x] (2026-03-27 20:20Z) Finished the second read-only coordinator-config
  comparison and identified the first concrete remaining mismatch: the packed
  config body is still writing singleton primary-region bounds instead of the
  original contiguous partition hash intervals.
- [x] (2026-03-27 20:24Z) Reopened the coordinator-config evidence workstream
  for one narrower read-only step: turn the original HyperDex partition logic
  into exact expected intervals and packed bytes for the live `profiles`
  primary subspace.
- [x] (2026-03-27 20:28Z) Reconciled `1d6093c` (`Use HyperDex partition
  intervals in legacy config`), verified the new focused interval test, and
  confirmed that the fast large-object public loop still reproduces
  `Left ClientGarbage`.
- [x] (2026-03-27 20:28Z) Finished `cce-003`, which produced the exact
  interval table and packed-byte fixtures for the live `profiles` primary
  subspace and confirmed that the landed encoder change matches the original
  HyperDex partition contract.
- [x] (2026-03-27 20:31Z) Finished `cce-004`, which identified the next exact
  packed-config mismatch after region intervals: zero-based ID allocation,
  with `virtual_server_id=0` as a likely route-preparation blocker before any
  `REQ_ATOMIC` can be sent.
- [x] (2026-03-27 20:36Z) Finished `cce-005`, which proved that the concrete
  failing key `"large"` already routes to a non-null replica tuple on current
  `main`, so the remaining blocker is later than route selection.
- [x] (2026-03-27 20:41Z) Finished `cce-006`, which identified the next exact
  pre-daemon contract after route selection: the client-to-daemon routing
  header and its reverse mapping plus version acceptance.
- [x] (2026-03-27 20:46Z) Finished `cce-007`, which ruled out the routing-header
  contract for the concrete failing key and moved the active diagnosis to the
  first body contract after an accepted daemon header.
- [x] (2026-03-27 20:51Z) Finished `cce-008`, which ruled out the first
  post-header body contract as well and moved the active diagnosis to the first
  daemon-side processing or response contract after a structurally valid atomic
  request.
- [x] (2026-03-27 20:56Z) Finished `cce-009`, which identified the first exact
  daemon-side divergence after a structurally valid atomic request: missing
  validation plus missing explicit `RESP_ATOMIC/NET_BADDIMSPEC` response
  semantics.
- [x] (2026-03-27 21:05Z) Reconciled `acfdcdc` (`Improve legacy daemon
  protocol handling`), verified the two new focused atomic-validation tests,
  and confirmed that the next concrete failing surface is the multiprocess
  `early eof` process-level path.
- [x] (2026-03-27 21:20Z) Reconciled `5879fab` (`Fix legacy daemon multiprocess
  EOF path`), verified that the former `early eof` multiprocess failures are
  gone on integrated `main`, and restored the focused large-object probe as the
  active public failure.
- [x] (2026-03-27 21:30Z) Reopened `multiprocess-harness` as an active third
  parallel workstream on the cleaned large-object baseline so product,
  read-only evidence, and harness-owned reproduction work can all advance at
  once.
- [x] (2026-03-27 21:31Z) Launched the new harness owner on
  `clientgarbage-wire`, so the remaining large-object failure is now being
  driven in parallel by product, read-only comparison, and harness-owned
  reproduction work.
- [x] (2026-03-27 21:34Z) Reframed the read-only comparison result: the
  cleaned baseline still fails before daemon traffic, so the next exact
  evidence target is again the remaining coordinator follow/config mismatch.
- [x] (2026-03-27 21:35Z) Passed that pre-daemon evidence into the running
  product and harness workers, closed the completed read-only worker, and
  launched a new read-only comparison on the narrower follow/config target.
- [x] (2026-03-27 21:42Z) Reconciled `69d5918` (`Capture failing hyhac
  bootstrap-only coordinator loop`), which proves the fast Hyhac failure on
  the cleaned baseline still loops only through coordinator identify/bootstrap
  traffic and does not progress to non-bootstrap coordinator or daemon
  messages.
- [x] (2026-03-27 21:46Z) Finished the read-only comparison on that same
  baseline and named the remaining exact blocker: the Replicant bootstrap
  sender-identity contract.
- [ ] Rerun the bounded live `hyhac` probe after the remaining large-object
  mismatch is fixed.

## Current Root Focus

Drive the remaining focused large-object `ClientGarbage` failure around the
bootstrap acceptance path on the coordinator connection. The sender-id plumbing
is now internally consistent across identify and bootstrap, but the live Hyhac
probe still never advances to a non-bootstrap Replicant request.

## Next Root Move

Reduce the still-bootstrap-only failure to the next exact acceptance mismatch
after the wire-visible sender id is made consistent. The next concrete step is
to compare the original Replicant client's anonymous-channel bootstrap
acceptance against the Rust coordinator's handcrafted BusyBee session behavior,
instead of widening back out to follow/config or daemon traffic.

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
- Observation: the current coordinator still answers that bootstrap with the
  wrong Replicant message type, before higher-level HyperDex condition payload
  compatibility even matters.
  Evidence: the captured 88-byte reply decodes as
  `REPLNET_CLIENT_RESPONSE`, while `Replicant/client/client.cc` expects the
  first successful reply on that path to be `REPLNET_BOOTSTRAP` carrying the
  coordinator-set install.
- Observation: the current root files were accurate about the technical blocker
  but too narrow about ownership and verification speed for the updated
  `autoplan` guidance.
  Evidence: only one workstream was marked active, the board lacked an owner
  column, and the fast proxy loop for the live admin path was not named.
- Observation: the new multiprocess harness result shortens the live admin
  measurement path materially.
  Evidence: `6f061b3` adds a targeted test that boots real Rust processes,
  drives the original admin client, and reports `advanced=false` in one fast
  command instead of a manual captured-wire sequence.
- Observation: fixing the coordinator bootstrap path moved the blocker exactly
  where the public behavior said it should move next.
  Evidence: `c087f81` makes `legacy_admin_wait_until_stable_probe_reports_bootstrap_progress`
  report `advanced=true`, direct `hyperdex-show-config`, `hyperdex-wait-until-stable`,
  and `hyperdex-add-space` succeed, and the next failing check is now `hyhac`
  client traffic returning `ClientGarbage`.
- Observation: the first daemon-path public failure is already narrower than
  the earlier selected `hyhac` command suggested.
  Evidence: `0b2379d` shows `*Can store a large object*` is enough to reproduce
  `Left ClientGarbage`, and that failure appears before later pooled failures.
- Observation: the first wire-capture harness retry drifted outside its owned
  surface and could not be reconciled safely.
  Evidence: `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/clientgarbage-probe`
  now has unrelated edits in `crates/consensus-core/src/lib.rs`,
  `crates/data-model/src/lib.rs`, `crates/engine-memory/src/lib.rs`,
  `crates/hyperdex-admin-protocol/src/lib.rs`, `crates/legacy-frontend/src/lib.rs`,
  `crates/legacy-protocol/src/lib.rs`, `crates/server/src/lib.rs`, and
  `crates/simulation-harness/src/lib.rs`.
- Observation: the short public daemon-path loop still gives fast, stable
  signal on clean `main`.
  Evidence: `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_hits_clientgarbage_fast -- --nocapture`
  completed in about `86ms` of probe time and still reported the expected
  `Left ClientGarbage` output, while `cargo test -p server` stayed green.
- Observation: a clean harness retry can still fail by stopping too early even
  when the mutable surface and validator are correct.
  Evidence: the completed `clientgarbage-wire` worker returned only baseline
  verification against `ad458f1`, with no code changes and no new wire
  evidence about the bad daemon frame.
- Observation: the focused large-object path still fails before the daemon sees
  `REQ_ATOMIC`.
  Evidence: the live product worker ran a manual cluster with the full
  `profiles` schema, reproduced the focused `hyhac` failure, and temporary
  `ReqAtomic` tracing inside `handle_legacy_request` never fired.
- Observation: the first captured exchange on that path is still on the
  coordinator connection and not yet a decodable legacy daemon frame.
  Evidence: `853e290` adds
  `legacy_hyhac_large_object_probe_reports_first_coordinator_frame_pair`, which
  captures only partial BusyBee-style frames with `trailing_bytes=45` and
  `trailing_bytes=100` on the coordinator connection.
- Observation: those captured coordinator frames are healthy BusyBee identify
  plus Replicant bootstrap traffic, not the active contract failure.
  Evidence: the completed `cce-001` step maps the 45-byte stream to BusyBee
  `IDENTIFY` plus a 5-byte `REPLNET_BOOTSTRAP` request frame and maps the
  100-byte stream to BusyBee `IDENTIFY` plus a 60-byte bootstrap response
  frame.
- Observation: correcting string-slice encoding and legacy datatype codes is
  necessary but not sufficient for the large-object path.
  Evidence: `be0cb38` is on `main`, the focused config tests pass, but the fast
  large-object repro still reports `Left ClientGarbage`.
- Observation: the first remaining packed-config mismatch is now concrete and
  local to primary-subspace region bounds.
  Evidence: the completed `cce-002` comparison shows Rust emits
  `lower=partition`, `upper=partition` for every primary region, while the
  original HyperDex builder emits contiguous `hyperdex::partition(...)` hash
  intervals such as `upper=0x03ffffffffffffff` for the first region.
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
- Observation: the binary `config` follow payload encoder is now also on
  `main`.
  Evidence: `0d8d566` replaces `default_legacy_config_encoder` with packed
  `hyperdex::configuration` encoding, and the worker reported both
  `cargo test -p server` and `cargo test --workspace` passing.
- Observation: the current live blocker appears earlier than the admin tools.
  Evidence: the fresh live probe showed the coordinator listening on a free
  public port, but the daemon exited immediately with `Error: early eof` while
  trying to register through that public coordinator port, so the admin tools
  were never reached in that run.
- Observation: that daemon-registration failure was not stable on clean
  `main`.
  Evidence: a fresh free-port probe reproduced successful daemon join through
  the public coordinator port, but both `hyperdex-add-space` and
  `hyperdex-wait-until-stable` still timed out afterward.
- Observation: the original admin client does not progress beyond the first
  `config` follow completion.
  Evidence: the captured wire shows one 25-byte Replicant bootstrap request
  followed by one 88-byte Rust response, after which neither `hyperdex-add-space`
  nor `hyperdex-wait-until-stable` sends a second request before timing out.

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
