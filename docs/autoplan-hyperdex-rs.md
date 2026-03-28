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
- `hyhac` compatibility notes: [hyhac-compatibility-surface.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/hyhac-compatibility-surface.md)
- Paper notes: [papers-and-mvp-notes.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/papers-and-mvp-notes.md)
- Worktree inventory: [worktrees.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/worktrees.md)

## Purpose / Big Picture

`hyperdex-rs` is meant to replace HyperDex with a real Rust system, not just a
test harness that imitates parts of it. Success means a live coordinator plus
daemon cluster works as a distributed system, the public compatibility surface
is strong enough to run `hyhac`, and the proof suites are strong enough to make
live failures meaningful instead of noisy.

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

## Mutable Surface

- `/home/friel/c/aaronfriel/hyperdex-rs/**`
- `/home/friel/c/aaronfriel/hyhac/scripts/**` only when launcher or harness
  wiring must point at `hyperdex-rs`
- Active worktrees listed in the `Workstream Board`
- Watchdog check-ins for this effort must send an explicit parent-thread
  message on every run

## Iteration Unit

One bounded root coordination step means: reconcile active workstreams, make
sure substantial work is preregistered, advance each unblocked workstream by at
most one validated step, and record the root-level judgment in the root loop
ledger.

## Loop Budget

Six bounded root coordination steps before reviewing whether the workstream
split, sequencing, or validators need to change.

## Dispositions

- `advance`
- `retry`
- `reframe`
- `revert`
- `escalate`
- `stop`

## Pivot Rules

- If `hyhac` or the C client tools expose a concrete failing operation, narrow
  the next product step to that observed behavior instead of broad protocol
  speculation.
- If the fastest honest public loop is still too broad for efficient iteration,
  shorten it before launching a long product pass.
- If recent history shows mostly `docs/**` or harness-only churn without
  corresponding product changes, stop and reorient toward landing code in
  `crates/**`.
- Keep dormant workstreams parked until the active product path truly needs
  them again.

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
4. Reconcile worktree results back into `main` in bounded validated steps.
5. Drive the live `hyhac` harness until the remaining semantic gaps are closed.

## Workstream Board

| Workstream | Status | Owner | Dependencies / Blockers | Plan | Ledger | Worktree / Branch | Fastest Useful Check | Next Step | Latest Disposition |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `live-hyhac` | active | `019d31ce-097e-7e51-bc7d-03b86e2996f6` (`Descartes`) | The honest full-schema large-object boundary now passes. `roundtrip` and `conditional` are now green on the broader pooled run too. The next honest pooled failures are later `search`, `count`, and parts of the atomic surface. | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/live-hyhac/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/live-hyhac/ledger.md) | `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-get-reconfigure` on `live-hyhac-get-reconfigure` | `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reaches_daemon_after_full_profiles_setup -- --nocapture` | Land the next product fix on the later pooled failures while keeping the cleared large-object, `roundtrip`, and `conditional` paths green. | `advance` |
| `multiprocess-harness` | active | `019d31ce-0ba4-7d51-bb1a-347bd18dad3d` (`Bernoulli`) | No current blocker. This workstream is active only to isolate the full-schema `roundtrip` failure into the smallest truthful post-large-object repro that still preserves real setup. | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/multiprocess-harness/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/multiprocess-harness/ledger.md) | `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/full-schema-roundtrip-repro` on `full-schema-roundtrip-repro` | `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reaches_daemon_after_full_profiles_setup -- --nocapture` | Add a focused truthful repro for the first post-large-object pooled `ClientReconfigure` failure, or prove the broader full-schema pooled loop should stand. | `advance` |
| `simulation-proof` | parked | root | Not on the critical path while live compatibility still fails earlier. | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/simulation-proof/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/simulation-proof/ledger.md) | `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/sim-coverage` on `sim-coverage-numeric` | `cargo test -p simulation-harness` | Leave parked until a live failure needs new deterministic coverage. | `advance` |
| `coordinator-config-evidence` | parked | root | Not on the critical path. The next active question is later than the coordinator follow/bootstrap path. | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/coordinator-config-evidence/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/coordinator-config-evidence/ledger.md) | none required | `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reaches_daemon_after_full_profiles_setup -- --nocapture` | Leave parked until the product pass needs another exact source comparison. | `advance` |

## Progress

- [x] (2026-03-27 05:48Z) Landed the legacy coordinator admin protocol and
  startup path needed for the original admin tools to create spaces and wait
  for stability against `hyperdex-rs`.
- [x] (2026-03-27 21:20Z) Landed distributed-runtime fixes on `main`,
  including the daemon-path validation fixes and the multiprocess `early eof`
  fix, with green `cargo test -p server` and workspace checks.
- [x] (2026-03-27 23:05Z) Moved the focused Hyhac path beyond bootstrap and
  coordinator follow traffic so the active failure is no longer in the admin
  path.
- [x] (2026-03-27 23:58Z) Proved the old fast large-object check was invalid
  because it failed before schema creation with immediate `UnknownSpace`.
- [x] (2026-03-28 00:17Z) Replaced that invalid check with a full-schema
  baseline that creates the real 19-attribute `profiles` space, waits until
  stable, proves native C success, and proves one successful Hyhac round-trip.
- [x] (2026-03-28 00:48Z) Landed the concurrent-connection fix in the legacy
  frontend and cleared the full-schema large-object post-success blocker on
  integrated `main`.
- [x] (2026-03-28 01:02Z) Ran the broader full-schema pooled Hyhac surface on a
  live Rust cluster and found the next truthful failure: `roundtrip` is the
  first later pooled failure and returns `ClientReconfigure` after the
  large-object path already succeeds.
- [x] (2026-03-28 01:18Z) Landed a sparse-record legacy `get` fix on `main`,
  keeping the large-object guard green and moving the broader pooled live
  boundary forward: `roundtrip` and `conditional` now pass.
- [ ] Land the next product fix for the later pooled failures in `search`,
  `count`, and the remaining atomic paths.

## Current Root Focus

Drive the live compatibility path past the now-cleared large-object boundary
and keep the next iterations biased toward material code in `crates/**`. The
active problem is no longer bootstrap, schema creation, the first daemon
round-trip, the large-object post-success stall, or pooled `roundtrip`
reconfigure. The active problem is now later in the pooled surface: `search`,
`count`, and several atomic operations still diverge.

## Next Root Move

Keep the newly-cleared pooled `roundtrip` and `conditional` paths green, wait
for the active harness reduction result if it returns useful leverage, and push
the next product pass onto the later pooled `search`/`count` and atomic
failures with the same honest live setup.

## Surprises & Discoveries

- Observation: the root plan had accumulated too much low-value history, which
  made current state harder to read and encouraged planning churn instead of
  code landing.
  Evidence: the root `Progress` section had grown into a long action log while
  the durable history already existed in the root loop ledger and workstream
  ledgers.
- Observation: the honest live compatibility boundary is now materially later
  than earlier coordinator/bootstrap failures.
  Evidence: the full-schema probe on `main` creates `profiles`, waits until
  stable, proves native C success, and proves one successful Hyhac round-trip
  before the next operation hangs.
- Observation: the first post-success large-object blocker was caused by the
  legacy frontend serializing long-lived clients instead of serving them
  concurrently.
  Evidence: `3c72516` makes `LegacyFrontend::serve_forever_with` spawn one task
  per accepted connection, the focused `legacy-frontend` regression passes, and
  the honest full-schema Hyhac large-object probe now completes both pooled and
  shared writes successfully.
- Observation: the next honest live failure is no longer speculative.
  Evidence: a live full-schema `*pooled*` run after `profiles` creation and
  `wait_until_stable` shows `Can store a large object: [OK]`, then fails first
  at `roundtrip` with `ClientReconfigure`, and later pooled operations fail
  with the same return code.
- Observation: sparse legacy `get` responses were one real cause of the pooled
  `ClientReconfigure` path.
  Evidence: `b23458c` now fills legacy defaults for missing attributes in
  sparse records, `legacy_get_fills_defaults_for_sparse_record_attributes`
  passes, `cargo test -p server` and `cargo test --workspace` pass, and the
  live full-schema pooled run now reports `roundtrip: [OK]` and
  `conditional: [OK]`.
- Observation: the recent imbalance was real: too much activity was being
  expressed in ledgers and harness growth rather than in product code.
  Evidence: the recent compact diff summary was dominated by `docs/**` and one
  large test file, while the next remaining blocker still requires substantive
  `crates/**` changes.

## Decision Log

- Decision: adopt the full root-pair-plus-workstreams layout instead of trying
  to keep the old single-file structure.
  Rationale: the effort already spans multiple independently advancing threads,
  worktrees, and validators.
  Date/Author: 2026-03-27 / root
- Decision: park non-critical workstreams until the live compatibility path
  needs them again.
  Rationale: the immediate blocker is the later Hyhac failure on the honest
  full-schema baseline, and the root should not treat parked proof or
  comparison threads as active progress.
  Date/Author: 2026-03-28 / root
- Decision: measure root progress primarily by reconciled code in product
  surfaces plus validators, not by planning churn.
  Rationale: the recent doc-heavy history made the control system noisier than
  the engineering signal and slowed actual delivery.
  Date/Author: 2026-03-28 / root

## Outcomes & Retrospective

- Pending. The effort is still active, and the next required outcome is a
  substantive product fix for the remaining later Hyhac failure on the honest
  live baseline.
