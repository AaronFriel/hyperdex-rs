# Fail Closed on Divergent Replica Get Responses

This ExecPlan is a living document. The sections `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work proceeds.

This repository does not contain a checked-in `PLANS.md`, so this document follows the fallback rules from the Codex `execplan` skill reference.

## Purpose / Big Picture

After this change, a distributed `Get` request will stop returning an arbitrary replica's answer when two reachable replicas disagree about the same key. Instead, the runtime will reject the read so split-brain state surfaces as an error that operators can investigate. The change is visible through a deterministic simulation that first creates replicated data, then forces a single replica to drift, and finally proves that `Get` fails closed instead of returning stale or inconsistent data.

## Progress

- [x] (2026-03-28 23:33Z) Synced `worktrees/active-failure-testing` to `origin/main` by repointing the stale branch directly at the current mainline tip after a replay attempt exposed a large obsolete branch-only history stack.
- [x] (2026-03-28 23:33Z) Narrowed the next missing failure case to distributed `Get`, which still returns the first reachable replica response without checking whether another reachable replica disagrees.
- [x] (2026-03-28 23:44Z) Added deterministic simulation proofs for two divergent `Get` shapes in `crates/simulation-harness/src/tests/mod.rs`: different attribute values and record-versus-tombstone presence.
- [x] (2026-03-28 23:47Z) Updated `crates/server/src/lib.rs` so distributed `Get` compares every reachable replica answer, skips only unavailable replicas, and rejects disagreement instead of returning the first answer.
- [x] (2026-03-28 23:50Z) Verified the focused simulations pass after the runtime fix: `cargo test -p simulation-harness tests::turmoil_rejects_divergent_replica_get_results -- --exact --nocapture` and `cargo test -p simulation-harness tests::turmoil_rejects_divergent_replica_get_presence -- --exact --nocapture`.
- [x] (2026-03-28 23:53Z) Ran the narrow affected runtime validator with `cargo test -p server --lib`; all 64 `server` library tests passed.
- [x] (2026-03-28 23:56Z) Finished the required broad validator with `cargo test -p simulation-harness`; all 29 tests passed.
- [ ] Investigate whether the unrelated `server` multiprocess hyhac failures (`slow_legacy_hyhac_pooled_probe_turns_green_after_map_atomic_compatibility` and `slow_legacy_hyhac_split_acceptance_suite_passes_live_cluster`) are pre-existing flake or a separate regression outside this `Get` change.

## Surprises & Discoveries

- Observation: rebasing the existing `active-failure-testing` branch tried to replay 233 stale commits, mostly old planning history, instead of a small local workstream delta.
  Evidence: `git rev-list --left-right --count origin/main...active-failure-testing` returned `0 235` during the failed rebase attempt.

- Observation: the focused proof failed before the runtime change exactly because `Get` returned success from the first replica instead of checking the other reachable replica.
  Evidence: `cargo test -p simulation-harness tests::turmoil_rejects_divergent_replica_get_results -- --exact --nocapture` failed with `expected divergent replica get to fail closed from the secondary`.

- Observation: `cargo test -p server` failed only in two slow multiprocess hyhac cases after the regular `server` unit suite had already passed.
  Evidence: `slow_legacy_hyhac_pooled_probe_turns_green_after_map_atomic_compatibility` reported `pooled hyhac probe did not preserve map int-int union success`, and `slow_legacy_hyhac_split_acceptance_suite_passes_live_cluster` reported pooled acceptance exit status `9` with empty stderr.

## Decision Log

- Decision: Target distributed `Get` next instead of extending the already-covered write, delete-group, search, or count cases.
  Rationale: `crates/server/src/lib.rs` currently returns the first replica answer in `execute_get_with_replica_fallback`, so split-brain state can escape to clients through another operation family that is not yet hardened.
  Date/Author: 2026-03-28 / Codex

- Decision: Realign the worktree branch directly to `origin/main` instead of replaying the stale local branch stack.
  Rationale: the branch was carrying a long obsolete history unrelated to the current workstream, and the user explicitly required syncing to current `main` before coding.
  Date/Author: 2026-03-28 / Codex

## Outcomes & Retrospective

Distributed `Get` now fails closed when reachable replicas disagree on the same key, which closes another split-brain read path beyond the already-covered search, count, and write rollback cases. The two new deterministic simulations prove both value mismatch and record-versus-tombstone disagreement. The focused and broad validators needed for this workstream passed: the targeted new simulations, `cargo test -p server --lib`, and `cargo test -p simulation-harness`.

One gap remains outside the scope of this change: `cargo test -p server` still hit two slow hyhac failures in `tests/dist_multiprocess_harness.rs`. Those failures did not mention the new `Get` disagreement path and need separate investigation if the workstream later requires a clean full `server` package run.

## Context and Orientation

`crates/server/src/lib.rs` contains the distributed runtime logic. The function `execute_get_with_replica_fallback` currently walks the placement decision and returns as soon as one replica answers, so it never notices when another reachable replica has a different value or a tombstone for the same key.

`crates/simulation-harness/src/tests/mod.rs` contains deterministic runtime proofs that exercise failure handling in a two-node replicated cluster. The same file already proves fail-closed behavior for divergent search and count results, stale local-primary writes, and transport rollback cases, so it is the right place to add the next split-brain proof for `Get`.

In this repository, a "replica" is another runtime that stores the same logical record because the cluster is configured with `replicas: 2`. A "split-brain" read in this plan means that two reachable replicas return different logical answers for the same key, such as different attribute values or one record versus no record.

## Plan of Work

First, add a new deterministic simulation in `crates/simulation-harness/src/tests/mod.rs` that writes a replicated record through the normal client path, mutates only one replica through a direct internode replicated write or delete, and then asserts that a client `Get` now returns an error. This proof should use a routed key so the request crosses the distributed `Get` path rather than a trivial single-node local read.

Next, change `crates/server/src/lib.rs` so `execute_get_with_replica_fallback` gathers every reachable replica answer, skips only clearly unavailable peers, and compares the logical record values. If more than one reachable replica answers and any answer differs from the first one, return an error that names the conflicting replica. If exactly one reachable replica answers, keep returning that answer so degraded reads still work during a one-node outage.

## Concrete Steps

From `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/active-failure-testing`:

1. Run the focused proof while iterating:

       cargo test -p simulation-harness turmoil_rejects_divergent_replica_get_results -- --exact --nocapture

   Before the runtime fix, expect the new test to fail because `Get` returns a record instead of an error.

2. Run the narrow runtime validator after the fix:

       cargo test -p server

3. Finish with the required broad validator:

       cargo test -p simulation-harness

## Validation and Acceptance

Acceptance requires three observable outcomes. First, the new focused simulation must fail before the runtime change and pass after it. Second, existing `server` tests must continue to pass, showing the `Get` change did not break other request handling. Third, `cargo test -p simulation-harness` must pass, showing degraded read coverage still works while the new split-brain proof now fails closed.

## Idempotence and Recovery

The tests in this plan are safe to rerun. If a focused test fails after an edit, rerun the same command after the next patch; there is no destructive state to clean up because the simulations create fresh in-memory runtimes each time. If a branch sync or validation step needs to be retried, keep the worktree on `origin/main` plus the new local commit only.

## Artifacts and Notes

Expected focused proof shape after the fix:

    running 1 test
    test tests::turmoil_rejects_divergent_replica_get_results ... ok

Expected final validator shape:

    test result: ok. <N> passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

## Interfaces and Dependencies

The runtime change stays within existing interfaces. `HyperdexClientService::handle` continues to call into `ClusterRuntime`. `execute_get_with_replica_fallback` in `crates/server/src/lib.rs` remains the single place that resolves distributed `Get` requests. The simulation proof will continue using `InternodeRequest::encode`, `DataPlaneRequest`, and `DataPlaneResponse` from `transport_core` to create a controlled one-replica divergence without adding new harness infrastructure.

Change note: created the initial ExecPlan after syncing the worktree and identifying distributed `Get` disagreement as the next missing deterministic failure case.
Change note: updated the plan after landing the `Get` fail-closed change, recording passing focused and broad validators plus the separate slow hyhac failure shape from the full `server` package run.
