# Workstream Ledger: failure-testing

### Entry `flt-001` - Preregistration

- Timestamp: `2026-03-28 10:00Z`
- Kind: `preregister`
- Hypothesis: the current deterministic proof surface can support a more
  adversarial distributed failure test without a large harness rewrite, and
  that test will either harden the runtime or expose the next concrete bug.
- Owner: forked worker on `failure-testing`
- Start commit: `9104047`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/failure-testing`
  - `failure-testing`
- Mutable surface:
  - `crates/simulation-harness/**`
  - `crates/server/**` only if the proof finds a runtime bug
- Validator:
  - fastest useful check:
    `cargo test -p simulation-harness turmoil_preserves_degraded_read_correctness_after_one_node_loss -- --nocapture`
  - strong checks:
    - `cargo test -p simulation-harness`
    - `cargo test --workspace`
- Expected artifacts:
  - one new deterministic failure-oriented proof
  - a green workspace or a concrete runtime bug with evidence
  - one bounded commit ready for reconciliation

### Entry `flt-001` - Outcome

- Timestamp: `2026-03-28 23:35Z`
- Kind: `outcome`
- End commit: `adc5b25`
- Artifact location:
  - `crates/server/src/lib.rs`
  - `crates/simulation-harness/src/tests/mod.rs`
- Evidence summary:
  - Added
    `turmoil_preserves_search_and_count_during_schema_convergence_gap`.
  - The new proof initially exposed a real bug: a live replica without the
    space definition could abort distributed `Search` and `Count`.
  - The server now skips a replica that is merely behind on schema state in
    the distributed read path.
- Conclusion: the first failure-oriented simulation step found and fixed a
  real distributed read correctness bug.
- Disposition: `advance`
- Next move: pick the next distributed assumption and add the next adversarial
  proof.

### Entry `flt-002` - Outcome

- Timestamp: `2026-03-28 23:55Z`
- Kind: `outcome`
- End commit: `f4e4215`
- Artifact location:
  - `crates/server/src/lib.rs`
  - `crates/simulation-harness/src/tests/mod.rs`
- Evidence summary:
  - Added `turmoil_reverts_primary_put_when_replica_transport_fails`.
  - The new proof exposed a bug where failed replicated writes could still
    leave a locally visible record on the primary.
  - The primary write path now restores the prior local record when replica
    fanout fails.
- Conclusion: failed replicated writes now roll back cleanly instead of
  leaking primary-local state.
- Disposition: `advance`
- Next move: test another mutation path under replica-loss conditions.

### Entry `flt-003` - Outcome

- Timestamp: `2026-03-28 23:55Z`
- Kind: `outcome`
- End commit: `4a3e876`
- Artifact location:
  - `crates/server/src/lib.rs`
  - `crates/simulation-harness/src/tests/mod.rs`
- Evidence summary:
  - Added `turmoil_reverts_primary_delete_when_replica_transport_fails`.
  - The new proof exposed a bug where failed replicated deletes removed the
    primary record without a committed replication result.
  - The primary delete path now restores the prior local record when replica
    fanout fails.
- Conclusion: failed replicated deletes now match the write rollback contract.
- Disposition: `advance`
- Next move: choose the next distinct routing or mutation assumption to break.

### Entry `flt-004` - Preregistration

- Timestamp: `2026-03-29 00:25Z`
- Kind: `preregister`
- Hypothesis: `ConditionalPut` may still expose a partially committed primary
  result when replica fanout fails, because it follows a distinct compare-and-
  write control path from the already-fixed put and delete cases.
- Owner: forked worker on `failure-testing`
- Start commit: `fb02bcc`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/failure-testing`
  - `failure-testing`
- Mutable surface:
  - `crates/simulation-harness/**`
  - `crates/server/**` only if the new proof exposes a runtime bug
- Validator:
  - fastest useful check:
    `cargo test -p simulation-harness turmoil_reverts_primary_conditional_put_when_replica_transport_fails -- --nocapture`
  - strong checks:
    - `cargo test -p simulation-harness`
    - `cargo test --workspace`
- Expected artifacts:
  - one deterministic `ConditionalPut` failure proof
  - either a green proof-only commit or a runtime fix for a discovered bug
  - one bounded commit ready for reconciliation

### Entry `flt-004` - Outcome

- Timestamp: `2026-03-29 00:39Z`
- Kind: `outcome`
- End commit: `7f02478`
- Artifact location:
  - `crates/simulation-harness/src/tests/mod.rs`
- Evidence summary:
  - Added `turmoil_reverts_primary_conditional_put_when_replica_transport_fails`.
  - The new proof showed that `ConditionalPut` already preserves the old value
    on the primary and the replica when replica transport fails after the
    compare-and-write begins.
  - `cargo test -p simulation-harness turmoil_reverts_primary_conditional_put_when_replica_transport_fails -- --nocapture`
    passed, and `cargo test -p simulation-harness` stayed green after the
    merge.
- Conclusion: the rollback contract already covered the `ConditionalPut` path,
  so this pass adds real adversarial proof coverage without needing a server
  fix.
- Disposition: `advance`
- Next move: pick the next distributed assumption outside the existing
  rollback family.

### Entry `flt-005` - Preregistration

- Timestamp: `2026-03-29 01:05Z`
- Kind: `preregister`
- Hypothesis: a stale placement view on one runtime may still allow a routed
  mutation to target the wrong primary or apply under the wrong ownership
  assumptions, and the current deterministic harness can express that without a
  large rewrite.
- Owner: forked worker on `failure-testing`
- Start commit: `7e79838`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/failure-testing`
  - `failure-testing`
- Mutable surface:
  - `crates/simulation-harness/**`
  - `crates/server/**` only if the new proof exposes a runtime bug
- Validator:
  - fastest useful check:
    `cargo test -p simulation-harness turmoil_rejects_or_recovers_routed_mutation_under_stale_placement -- --nocapture`
  - strong checks:
    - `cargo test -p simulation-harness`
    - `cargo test --workspace`
- Expected artifacts:
  - one deterministic stale-placement mutation proof
  - either a green proof-only commit or a runtime fix for a discovered bug
  - one bounded commit ready for reconciliation

### Entry `flt-005` - Outcome

- Timestamp: `2026-03-29 10:00Z`
- Kind: `outcome`
- End commit: `8da80c8`
- Artifact location:
  - `crates/simulation-harness/src/tests/mod.rs`
  - `crates/server/src/lib.rs`
- Evidence summary:
  - Added `turmoil_rejects_or_recovers_routed_mutation_under_stale_placement`.
  - The new proof exposed a real bug: primary-only internode `Put`, `Delete`,
    and `ConditionalPut` requests were accepted even if the receiving node's
    current placement view no longer considered it the primary for that key.
  - `handle_internode_request` now rejects those primary-only operations when
    local placement ownership does not match.
- Conclusion: routed mutations now reject stale-placement primary writes
  instead of applying them under diverged cluster views.
- Disposition: `advance`
- Next move: pick the next broken distributed assumption in the rejoin or
  recovery path.

### Entry `flt-006` - Preregistration

- Timestamp: `2026-03-29 10:00Z`
- Kind: `preregister`
- Hypothesis: a node that rejoins after cluster-view drift or outage may still
  expose stale reads or accept incorrect internode traffic during convergence,
  and the deterministic harness can express that without a broad rewrite.
- Owner: forked worker on `failure-testing`
- Start commit: `8da80c8`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/failure-testing`
  - `failure-testing`
- Mutable surface:
  - `crates/simulation-harness/**`
  - `crates/server/**` only if the new proof exposes a runtime bug
- Validator:
  - fastest useful check:
    `cargo test -p simulation-harness turmoil_preserves_correctness_when_stale_node_rejoins_cluster -- --nocapture`
  - strong checks:
    - `cargo test -p simulation-harness`
    - `cargo test --workspace`
- Expected artifacts:
  - one deterministic rejoin-or-recovery proof
  - either a green proof-only commit or a runtime fix for a discovered bug
  - one bounded commit ready for reconciliation

### Entry `flt-006` - Outcome

- Timestamp: `2026-03-29 10:10Z`
- Kind: `outcome`
- End commit: `06370d6`
- Artifact location:
  - `crates/simulation-harness/src/tests/mod.rs`
- Evidence summary:
  - Added `turmoil_preserves_correctness_when_stale_node_rejoins_cluster`.
  - The new proof exercised node rejoin after cluster-view drift and did not
    expose a correctness bug on the merged runtime.
- Conclusion: stale-node rejoin now has explicit deterministic proof coverage.
- Disposition: `advance`
- Next move: test mutation and delete-group behavior during schema or ownership
  convergence instead of another read-only recovery case.

### Entry `flt-007` - Outcome

- Timestamp: `2026-03-29 18:05Z`
- Kind: `outcome`
- End commit: `2b7d144`
- Artifact location:
  - `crates/server/src/lib.rs`
  - `crates/simulation-harness/src/tests/mod.rs`
- Evidence summary:
  - `b6ae810` added rollback for distributed `DeleteGroup` when replica
    deletion fails.
  - `a4ea7d3` hardened distributed `Search` and `Count` during local schema
    gaps.
  - `fb77107` let distributed `DeleteGroup` skip schema-gap replicas.
  - `2b7d144` corrected the merged-state regression by ensuring delete-group
    snapshot collection skips only schema-gap replicas, not true transport
    failures.
- Conclusion: delete-group, search, and count now handle schema convergence
  and replica failure more consistently.
- Disposition: `advance`
- Next move: move to ownership-change and primary-handoff mutation cases.

### Entry `flt-008` - Preregistration

- Timestamp: `2026-03-29 18:20Z`
- Kind: `preregister`
- Hypothesis: a mutation or delete routed during primary handoff or partial
  config convergence may still be accepted, skipped, or rolled back against
  the wrong ownership view, and the current deterministic harness can express
  that without a broad rewrite.
- Owner: next forked worker on `failure-testing`
- Start commit: `2b7d144`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/failure-testing`
  - `failure-testing`
- Mutable surface:
  - `crates/simulation-harness/**`
  - `crates/server/**`
  - `crates/control-plane/**` only if the proof requires cluster-view plumbing
- Validator:
  - fastest useful check:
    one new targeted deterministic simulation test for ownership-change
    mutation behavior
  - strong checks:
    - `cargo test -p simulation-harness`
    - `cargo test -p server`
    - `cargo test --workspace`
- Expected artifacts:
  - one deterministic proof for primary-handoff or config-convergence mutation
    behavior
  - a product fix if the proof exposes a bug
  - one bounded commit ready for reconciliation
