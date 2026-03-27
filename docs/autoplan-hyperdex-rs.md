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
| `simulation-proof` | active | None | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/simulation-proof/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/simulation-proof/ledger.md) | `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/sim-coverage` on `sim-coverage-numeric`; root checkout also has in-flight proof edits | Reconcile routed numeric-mutation Hegel coverage, then tighten the remaining schema-permissive single-node sequence test. | `advance` |
| `multiprocess-harness` | active | None | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/multiprocess-harness/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/multiprocess-harness/ledger.md) | `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/dist-multiprocess-harness`; root checkout also has in-flight harness edits | Reconcile the serial-test harness fix, then replace log-text waits and ephemeral port reuse with protocol-based readiness. | `advance` |
| `live-hyhac` | ready | Benefits from the two proof/harness fixes landing first, but not hard-blocked after that | [plan.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/live-hyhac/plan.md) | [ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/live-hyhac/ledger.md) | root checkout | Run the live `hyhac` harness against current `main`, capture the next failing admin or client path, and narrow the next compatibility step to that evidence. | `advance` |

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
- [ ] Reconcile the in-flight `simulation-proof` and `multiprocess-harness`
  edits into bounded validated commits.
- [ ] Advance `live-hyhac` by rerunning the Haskell harness against the updated
  cluster and narrowing the next compatibility gap from observed failures.

## Current Root Focus

Reconcile the current proof and multiprocess-harness edits into bounded commits
without losing the new root/workstream structure. Once those two workstreams
are landed and their ledgers record outcomes, the root should pivot directly to
the live `hyhac` run instead of adding more speculative coverage first.

## Next Root Move

Commit the root AutoPlan package, land the preregistered `simulation-proof`
result, land the preregistered `multiprocess-harness` result, then launch the
first post-restructure live `hyhac` probe and record its outcome in the
`live-hyhac` ledger.

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
