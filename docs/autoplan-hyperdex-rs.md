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
| `live-hyhac` | active | root | The corrected full-schema baseline is on `main`, native C succeeds, and Hyhac completes one successful round-trip before the next operation stalls. | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/live-hyhac/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/live-hyhac/ledger.md) | `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-post-follow` on `live-hyhac-post-follow` | `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reaches_daemon_after_full_profiles_setup -- --nocapture` | Launch one product-owned fix pass and reconcile it against the honest live check. | `advance` |
| `multiprocess-harness` | ready | root | No current blocker. This workstream should reactivate only if the current live probe is too broad for fast product iteration. | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/multiprocess-harness/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/multiprocess-harness/ledger.md) | `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/clientgarbage-wire` on `clientgarbage-wire` | `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reaches_daemon_after_full_profiles_setup -- --nocapture` | Reactivate only if a smaller or more trustworthy public repro is needed. | `advance` |
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
- [ ] Reduce and fix the remaining later Hyhac failure after the first
  successful large-object round-trip.

## Current Root Focus

Drive the remaining live compatibility failure on the corrected full-schema
baseline and bias the next iterations toward material code in `crates/**`.
The active problem is no longer bootstrap, schema creation, or the first daemon
round-trip. The active problem is the first later operation that diverges after
Hyhac has already demonstrated one successful round-trip against a live Rust
cluster.

## Next Root Move

Launch one fork that owns the end-to-end product fix for the remaining later
Hyhac failure, and one fork that owns the smallest trustworthy repro reduction
needed to keep that product pass moving quickly. Reconcile only substantive
results back into `main`, then rerun the honest live check.

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
