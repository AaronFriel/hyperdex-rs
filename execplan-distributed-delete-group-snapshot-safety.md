# Preserve Delete-Group Safety When Replicas Disagree

This ExecPlan is a living document. The sections `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work proceeds.

`PLANS.md` is not checked into this repository, so this document follows `/home/friel/.codex/skills/execplan/references/PLANS.md`.

## Purpose / Big Picture

After this change, a replicated `DeleteGroup` request will fail closed when reachable replicas disagree about which logical keys match the predicate. Before this change, the runtime can delete different logical keys on different replicas and still report success. The proof is a deterministic simulation that forces replica divergence, runs `DeleteGroup`, and shows the operation now returns an error without deleting either surviving local copy.

## Progress

- [x] (2026-03-28 18:05Z) Read the repository instructions, the fallback `PLANS.md`, the current simulation coverage, and the `ClusterRuntime::execute_distributed_delete_group` implementation.
- [x] (2026-03-28 18:17Z) Patched `crates/server/src/lib.rs` so distributed delete-group rejects mismatched logical snapshots before applying deletes and rolls back if the final physical delete count still violates the replica-factor invariant.
- [x] (2026-03-28 18:20Z) Added deterministic Turmoil proofs in `crates/simulation-harness/src/tests/distributed_simulation.rs` for both divergent replica snapshots and incomplete snapshot coverage.
- [x] (2026-03-28 18:25Z) Validated with focused simulation tests, `cargo test -p server runtime_supports_put_get_count_and_delete_group`, and the final `cargo test -p simulation-harness`.

## Surprises & Discoveries

- Observation: `execute_distributed_delete_group` only checks `deleted_total % replica_factor` after applying deletes.
  Evidence: `crates/server/src/lib.rs` applies every delete in the loop, then performs the invariant check and returns an error without rollback on that path.
- Observation: the old implementation can report `ClientResponse::Deleted(1)` even when each replica deletes a different logical key.
  Evidence: a two-key Turmoil repro that removes `left_key` from node 2 and `right_key` from node 1 causes the old code to sum two physical deletes and divide by replica factor two, even though no replica snapshot agrees on one shared logical key set.

## Decision Log

- Decision: Fix the runtime instead of adding only another proof.
  Rationale: the current behavior can report a successful logical delete while actually deleting different keys on different replicas, so a proof alone would only document a live correctness bug.
  Date/Author: 2026-03-28 / Codex

## Outcomes & Retrospective

The runtime now refuses to apply `DeleteGroup` when reachable replicas disagree about the matching logical keys, which closes the strongest incorrect-success case. The remaining arithmetic guard also rolls the applied deletes back before returning an error, so partial coverage no longer drops the surviving local copy. Two deterministic Turmoil proofs now pin both behaviors in `crates/simulation-harness/src/tests/distributed_simulation.rs`.

## Context and Orientation

`crates/server/src/lib.rs` contains the distributed mutation paths for `ClusterRuntime`, including `execute_distributed_delete_group`. That function first asks each reachable replica for a matching-record snapshot, then issues the replicated delete-group requests, then derives the logical deletion count from the summed physical deletions. `crates/simulation-harness/src/tests/distributed_simulation.rs` is the focused place for new deterministic recovery and ordering proofs in this worktree.

The important term here is “logical key”: one user record that may have one physical copy on each replica. A delete-group must only report success if every reachable replica agrees on the same logical keys before any delete is applied.

## Plan of Work

In `crates/server/src/lib.rs`, add a small helper that turns a snapshot of `Record` values into a comparable set of keys. Use the first collected snapshot as the expected logical key set. If any later snapshot differs, return an error before applying deletes. Keep the existing replica-factor arithmetic check as a final guard, but if that guard still fails after deletes, call the existing rollback path before returning the error.

In `crates/simulation-harness/src/tests/distributed_simulation.rs`, add a deterministic Turmoil test that starts with a replicated record set, then deliberately removes different matching records from each replica through internode replicated-delete requests. Run `ClientRequest::DeleteGroup` against the cluster and assert that it now errors and leaves each node’s surviving local record unchanged.

## Concrete Steps

From `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/active-distributed-simulation` run:

    cargo test -p simulation-harness turmoil_delete_group_rejects_divergent_replica_snapshots

Observed after the patch: `1 passed; 0 failed`.

Then run:

    cargo test -p simulation-harness turmoil_delete_group_
    cargo test -p server runtime_supports_put_get_count_and_delete_group
    cargo test -p simulation-harness

Observed after the patch:

    turmoil_delete_group_ => 3 passed; 0 failed
    runtime_supports_put_get_count_and_delete_group => 1 passed; 0 failed
    cargo test -p simulation-harness => 31 passed; 0 failed

## Validation and Acceptance

Acceptance is observable in the new deterministic simulation. The test must build a two-node replicated runtime, create replica disagreement about the delete-group predicate, run `DeleteGroup`, and observe an error. After that error, direct local reads on each node must still show the same per-node records that existed immediately before the delete-group attempt.

## Idempotence and Recovery

The test and cargo commands are safe to rerun. If a compile or test failure happens mid-change, inspect the reported file and line, update the code, and rerun the same command. No persistent external state is modified outside the temporary in-memory simulation fixtures.

## Artifacts and Notes

Key runtime behavior added in `crates/server/src/lib.rs`:

    self.ensure_delete_group_snapshots_agree(&space, &snapshots)?;

    if deleted_total % replica_factor != 0 {
        let rollback = self
            .rollback_delete_group_snapshots(&space, &snapshots[..applied_snapshot_count])
            .await;
        ...
    }

The divergent-snapshot proof now forces one matching key to survive only on node 1 and another only on node 2, then asserts that `DeleteGroup` returns an error containing `snapshot mismatch`.

## Interfaces and Dependencies

The runtime change stays inside `server::ClusterRuntime`. The proof uses the existing `transport_core::{DataPlaneRequest, DataPlaneResponse, InternodeRequest, DATA_PLANE_METHOD}` types so the test can inspect or mutate one node’s local data plane without going through distributed routing.

Change note: updated the plan after implementation to record the completed server fix, the two Turmoil proofs, and the validator results.
