# AutoPlan: hyperdex-rs distributed HyperDex replacement

This file is archived. It preserves the phase-1 root AutoPlan from the Hyhac
compatibility and distributed-baseline effort. The active root AutoPlan is now
[docs/autoplan.md](docs/autoplan.md).
Some internal links below still reflect the phase-1 layout and should be read
as historical context rather than the current filesystem contract.

This root AutoPlan is a living document. The sections `Progress`,
`Workstream Board`, `Current Root Focus`, `Next Root Move`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective`
must be kept current as work proceeds.

This repository does not contain its own `PLANS.md` or `AUTOPLANS.md`. This
document follows the fallback rules at
`the installed `autoplan` skill fallback rules` and
`the installed `autoplan` skill fallback rules`.

## Companion Files

- Archived phase-1 ledger: [ledger.md](docs/archive/phase-1/ledger.md)
- Current root AutoPlan: [autoplan.md](docs/autoplan.md)
- Current workstream index: [workstreams.md](docs/workstreams.md)

## Purpose / Big Picture

`hyperdex-rs` is meant to replace HyperDex with a real Rust system, not just a
test harness that imitates parts of it. Success means a live coordinator plus
daemon cluster works as a distributed system, the public compatibility surface
is strong enough to run `hyhac`, and the proof suites are strong enough to make
live failures meaningful instead of noisy.

## Goal

Create a pure-Rust HyperDex replacement at
`this repository` that preserves separate coordinator and
daemon processes, exposes both a legacy HyperDex-compatible frontend and a
modern gRPC frontend, forms a real distributed cluster, passes the `hyhac`
test suite against a live deployment, and then keeps expanding the system with
stronger validation, stronger proof coverage, and new distributed features.

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
- GitHub Actions workflows exercise the repository with the same kind of
  validation discipline used in the stronger Rust repositories this project is
  already borrowing from.
- Critical parsers, request handlers, and compatibility boundaries have focused
  fuzz targets and failure-oriented test coverage.
- The repository no longer depends on `async_trait` where native Rust async
  traits are sufficient.
- Warp-style transactions and region-aware georeplication have concrete
  implementation tracks with real code and validators.

## Mutable Surface

- `**`
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

1. Keep the completed live Hyhac compatibility baseline green and reusable.
2. Add repository-grade validation and failure-oriented proof loops that make
   future distributed changes safer and faster.
3. Remove avoidable implementation drag such as `async_trait` where modern
   Rust can replace it directly.
4. Add bounded fuzzing around the legacy protocol, admin protocol, and request
   decoding surfaces.
5. Add Warp-style transactions on top of the now-proven distributed runtime.
6. Add region-aware georeplication in the style of Consus.

## Completed Workstream Group

### Phase 1: Compatibility And Distributed Baseline

These workstreams are completed and moved under
[completed](docs/workstreams/completed):

- [live-hyhac](docs/workstreams/completed/live-hyhac/plan.md):
  the public Hyhac-facing compatibility surface is green on both single-daemon
  and two-daemon live clusters, and the reusable verifier exists.
- [simulation-proof](docs/workstreams/completed/simulation-proof/plan.md):
  deterministic degraded-read proof drift is fixed and the simulation harness
  is back on a green baseline.
- [multiprocess-harness](docs/workstreams/completed/multiprocess-harness/plan.md):
  the real-process harness is stable enough to support product work without
  pretending to be the product.
- [coordinator-config-evidence](docs/workstreams/completed/coordinator-config-evidence/plan.md):
  the read-only comparison work turned the early compatibility unknowns into
  concrete fixes and is no longer on the critical path.

## Priority Groups

### Group 1: Hardening And Execution Speed

1. [validation-ci](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/validation-ci/plan.md)
2. [failure-testing](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/failure-testing/plan.md)
3. [fuzzing-hardening](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/fuzzing-hardening/plan.md)
4. [async-modernization](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/async-modernization/plan.md)

### Group 2: Feature Expansion

1. [warp-transactions](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/warp-transactions/plan.md)
2. [georeplication](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/georeplication/plan.md)

## Workstream Board

| Workstream | Status | Owner | Dependencies / Blockers | Plan | Ledger | Worktree / Branch | Fastest Useful Check | Next Step | Latest Disposition |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `validation-ci` | active | root | No blocker. The repository has no `.github/workflows` yet, so this starts from a clear gap. | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/validation-ci/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/validation-ci/ledger.md) | worktree to be launched | `find .github/workflows -maxdepth 1 -type f | sort` | Land a first GitHub Actions set that covers formatting, clippy, workspace tests, and the reusable live acceptance verifier shape. | `advance` |
| `failure-testing` | active | root | No blocker. The runtime and proof harnesses are green enough to start adversarial testing from a real baseline. | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/failure-testing/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/failure-testing/ledger.md) | worktree to be launched | `cargo test -p simulation-harness turmoil_preserves_degraded_read_correctness_after_one_node_loss -- --nocapture` | Land the next failure-oriented Turmoil or Madsim proof that intentionally breaks a live assumption instead of only re-proving the current happy path. | `advance` |
| `async-modernization` | active | root | No blocker. `async_trait` is still present across consensus, transport, protocol, and server surfaces. | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/async-modernization/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/async-modernization/ledger.md) | worktree to be launched | `rg -n \"async_trait|\\#\\[async_trait\\]\" crates Cargo.toml` | Remove `async_trait` from at least one meaningful cross-crate surface without regressing behavior. | `advance` |
| `fuzzing-hardening` | ready | root | Depends on choosing the first high-value targets, but not on other active workstreams completing. | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/fuzzing-hardening/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/fuzzing-hardening/ledger.md) | none yet | `rg -n \"decode|encode|parse|frame|request|response\" crates` | Start with protocol and parser targets after the first CI and failure-test passes are underway. | `advance` |
| `warp-transactions` | ready | root | Depends on preserving the now-green baseline while designing and landing transaction semantics on top of it. | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/warp-transactions/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/warp-transactions/ledger.md) | none yet | `rg -n \"ConditionalPut|DeleteGroup|Search|Count|consensus|placement\" crates` | Turn the paper notes into a bounded transaction design and first implementation step over current coordinator and daemon roles. | `advance` |
| `georeplication` | ready | root | Depends on current placement and replication contracts being stable enough to extend with region-aware grouping. | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/georeplication/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/georeplication/ledger.md) | none yet | `rg -n \"NodeConfig|placement|replica|control-plane|region|cluster\" crates` | Add the first bounded design and implementation step for region-aware placement and replication. | `advance` |

## Progress

- [x] (2026-03-28 03:27Z) Completed the first phase: live Hyhac acceptance is
  green on both single-daemon and two-daemon clusters, the reusable verifier
  exists, and the deterministic proof baseline is green again.
- [x] (2026-03-28 10:00Z) Moved the completed compatibility and support
  workstreams under `docs/workstreams/completed` so the active board reflects
  the next phase instead of stale history.
- [ ] Launch the first substantial execution passes for `validation-ci`,
  `failure-testing`, and `async-modernization`.
- [ ] Land the first new post-baseline code changes from those workstreams.
- [ ] Start the first bounded implementation track for Warp-style transactions
  and region-aware georeplication once the faster validation loop is in place.

## Current Root Focus

Keep the completed compatibility baseline green while shifting real engineering
effort toward the next phase: repository-grade validation, failure-oriented
proof work, fuzzing, async cleanup, transactions, and georeplication. The
immediate job is to launch and reconcile substantive code-owning workstreams
instead of letting the control files become the main source of motion.

## Next Root Move

Arm a new watchdog against this updated root package, launch substantial forks
for `validation-ci`, `failure-testing`, and `async-modernization`, and then
wait for real code results rather than doing more planning churn.

## Surprises & Discoveries

- Observation: the repository is no longer in a “find the next Hyhac bug”
  phase; that work produced a green baseline and should now be treated as a
  completed foundation rather than the center of the plan.
  Evidence: the completed workstream group now covers single-daemon and
  two-daemon live acceptance plus a reusable verifier.
- Observation: the next productive phase should improve engineering leverage
  before it chases larger distributed features.
  Evidence: the repository still lacks GitHub Actions workflows, still carries
  `async_trait` across multiple cross-crate surfaces, and has no focused fuzz
  targets for the legacy compatibility boundary.
- Observation: it is easy for this effort to regress into control-file motion
  unless the active board names code-owning work with fast validators.
  Evidence: the recent user review correctly identified too much `docs/**`
  churn compared with product changes.

## Decision Log

- Decision: move the finished compatibility and support workstreams to the
  completed area before launching the next phase.
  Rationale: the active board should show current engineering priorities, not a
  historical success story that is already green.
  Date/Author: 2026-03-28 / root
- Decision: prioritize validation, failure-oriented testing, fuzzing, and
  async cleanup before larger feature additions.
  Rationale: the next feature phase should start from stronger feedback loops
  and lower implementation drag.
  Date/Author: 2026-03-28 / root
- Decision: keep Warp-style transactions and georeplication as explicit
  workstreams now, even if they start after the first hardening passes.
  Rationale: the user asked for them as real priorities, and they are central
  to the long-term Rust replacement rather than optional side ideas.
  Date/Author: 2026-03-28 / root

## Outcomes & Retrospective

- Phase 1 is complete and preserved under `docs/workstreams/completed`.
- The next useful retrospective will be about whether the new hardening and
  expansion workstreams are landing real code quickly enough, not about the
  already-completed Hyhac compatibility line.
