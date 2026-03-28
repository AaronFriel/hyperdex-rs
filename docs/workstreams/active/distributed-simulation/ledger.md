# Workstream Ledger: distributed-simulation

### Entry `dsm-004` - Preregistration

- Timestamp: `2026-03-28 23:54Z`
- Kind: `preregister`
- Hypothesis: the next useful deterministic recovery proof can stay in the
  current stale-rejoin family if it adds a scheduler-backed guarantee that is
  not already covered under Madsim, namely the delete boundary between an
  accepted write and a later rewrite.
- Owner: forked worker on `active-distributed-simulation`
- Start commit: `0e7f809`
- Worktree / branch:
  - `worktrees/active-distributed-simulation`
  - `active-distributed-simulation`
- Mutable surface:
  - `crates/simulation-harness/**`
- Validator:
  - fastest useful check:
    `RUSTFLAGS='--cfg madsim' cargo test -p simulation-harness madsim_recovery_preserves_delete_then_rewrite_visibility_after_stale_local_primary_rejoin -- --nocapture`
  - strong checks:
    - `cargo test -p simulation-harness`
- Expected artifacts:
  - one new deterministic Madsim recovery proof
  - one bounded commit ready for reconciliation

### Entry `dsm-004` - Outcome

- Timestamp: `2026-03-28 23:54Z`
- Kind: `outcome`
- End commit:
  - pending local commit
- Artifact location:
  - `crates/simulation-harness/src/tests/distributed_simulation.rs`
  - `docs/workstreams/active/distributed-simulation/plan.md`
- Evidence summary:
  - Added
    `madsim_recovery_preserves_delete_then_rewrite_visibility_after_stale_local_primary_rejoin`.
  - The new proof extends the stale local-primary rejoin family under Madsim:
    - a stale local-primary write is rejected before recovery
    - the authoritative node recreates the converged two-node view
    - the recovered node observes an accepted write
    - the recovered node then observes a delete with zero visible records
    - a later rewrite becomes visible and matches the authoritative view
  - Validation passed with:
    - `RUSTFLAGS='--cfg madsim' cargo test -p simulation-harness madsim_recovery_preserves_delete_then_rewrite_visibility_after_stale_local_primary_rejoin -- --nocapture`
    - `cargo test -p simulation-harness`
- Conclusion: Madsim now proves both ordered writes and delete-boundary
  visibility after stale local-primary rejoin, not only outage-retry recovery.
- Disposition: `advance`
- Next move: choose a three-node failover or handoff recovery proof so the
  suite moves beyond the current two-node stale-rejoin and replica-outage
  families.

### Entry `rco-001` - Preregistration

- Timestamp: `2026-03-29 19:20Z`
- Kind: `preregister`
- Hypothesis: the parked ownership-convergence patch and its proof are a good
  seed for a broader recovery-ordering effort, because they ask whether a
  stale node can still accept mutations after another node has the newer view.
- Owner: next forked worker on `recovery-ordering`
- Start commit: `e76e696`
- Worktree / branch:
  - `worktrees/recovery-ordering`
  - `recovery-ordering`
- Mutable surface:
  - `crates/simulation-harness/**`
  - `crates/server/**`
  - `crates/transport-core/**`
- Validator:
  - fastest useful check:
    one deterministic recovery-ordering test
  - strong checks:
    - `cargo test -p simulation-harness`
    - `cargo test -p server`
- Expected artifacts:
  - one recovery-ordering proof
  - a runtime fix if the proof exposes a bug
  - one bounded commit ready for reconciliation

### Entry `rco-001` - Outcome

- Timestamp: `2026-03-29 20:25Z`
- Kind: `outcome`
- End commit:
  - `754c6b9`
  - `f9f8b0f`
- Artifact location:
  - `crates/server/src/lib.rs`
  - `crates/simulation-harness/src/tests/mod.rs`
- Evidence summary:
  - Reused the stale-local-primary hardening and added
    `turmoil_recovery_preserves_operation_order_after_stale_local_primary_rejoin`.
  - The new proof exercises this sequence:
    - a stale local-primary write is rejected before recovery
    - the recovered node rejoins with the converged two-node view
    - two later authoritative writes are applied in order
    - the recovered node observes the final ordered state and never sees the
      rejected pre-recovery write
  - Root validation passed with:
    - `cargo test -p simulation-harness turmoil_recovery_preserves_operation_order_after_stale_local_primary_rejoin -- --nocapture`
    - `cargo test -p simulation-harness`
    - `cargo test -p server`
- Conclusion: the distributed simulation track now has a real recovery-ordering
  proof, not just a target statement.
- Disposition: `advance`
- Next move: add another deterministic recovery scenario on a different
  operation family or failure shape.

### Entry `rco-002` - Outcome

- Timestamp: `2026-03-29 20:45Z`
- Kind: `outcome`
- End commit: `9ae5d2a`
- Artifact location:
  - `crates/simulation-harness/src/tests/mod.rs`
- Evidence summary:
  - Added
    `turmoil_recovery_preserves_delete_then_rewrite_visibility_after_stale_local_primary_rejoin`.
  - The new proof covers a different recovery shape from the first stale-rejoin
    ordering case:
    - a stale local-primary write is rejected before recovery
    - the recovered node rejoins with the converged two-node view
    - the authoritative side performs `Put -> Delete -> Put`
    - the recovered node observes each visibility transition in order
    - the final recovered view matches the authoritative view without stale
      resurrection across the delete boundary
  - Root validation passed with:
    - `cargo test -p simulation-harness turmoil_recovery_preserves_delete_then_rewrite_visibility_after_stale_local_primary_rejoin -- --nocapture`
- Conclusion: recovery coverage now includes an explicit delete/rewrite
  visibility guarantee, not only ordered writes.
- Disposition: `advance`
- Next move: choose a recovery scenario outside the current stale-rejoin family.

### Entry `rco-002` - Preregistration

- Timestamp: `2026-03-29 20:40Z`
- Kind: `preregister`
- Hypothesis: the next useful Turmoil or Madsim result should couple recovery
  with a different state transition than the ordered-write path, so the repo
  proves more than one recovery shape.
- Owner: next forked worker on `distributed-simulation`
- Start commit: `46949a1`
- Worktree / branch:
  - `worktrees/recovery-ordering`
  - `recovery-ordering`
- Mutable surface:
  - `crates/simulation-harness/**`
  - `crates/server/**` if the proof exposes a bug
  - `crates/transport-core/**` if the proof needs a transport-level fix
- Validator:
  - fastest useful check:
    one targeted Turmoil or Madsim recovery test
  - strong checks:
    - `cargo test -p simulation-harness`
    - `cargo test -p server`
- Expected artifacts:
  - one new deterministic recovery proof
  - a product fix if the proof exposes a bug
  - one bounded commit ready for reconciliation

### Entry `dsm-003` - Preregistration

- Timestamp: `2026-03-28 19:24Z`
- Kind: `preregister`
- Hypothesis: the next useful distributed-simulation result is a recovery proof
  outside the current stale-rejoin family, ideally a failover or handoff
  sequence where operation order must still hold after node loss and return.
- Owner: forked worker on `distributed-simulation`
- Start commit: `0d395b6`
- Worktree / branch:
  - `worktrees/distributed-simulation-active`
  - `distributed-simulation-active`
- Mutable surface:
  - `crates/simulation-harness/**`
  - `crates/server/**`
  - `crates/transport-core/**` only if the proof exposes a transport bug
- Validator:
  - fastest useful check:
    one targeted Turmoil or Madsim recovery test for the chosen scenario
  - strong checks:
    - `cargo test -p simulation-harness`
    - `cargo test -p server`
- Expected artifacts:
  - one new deterministic recovery proof outside the current stale-rejoin pair
  - a product fix if the proof exposes a bug
  - one bounded commit ready for reconciliation

### Entry `dsm-003` - Outcome

- Timestamp: `2026-03-28 19:34Z`
- Kind: `outcome`
- End commit: `d3c7aee`
- Artifact location:
  - `crates/simulation-harness/src/tests/distributed_simulation.rs`
  - `crates/simulation-harness/src/tests/mod.rs`
- Evidence summary:
  - Added
    `turmoil_recovery_preserves_delete_group_retry_then_put_visibility_after_replica_outage`.
  - The new proof covers a different recovery family from the current
    stale-rejoin proofs:
    - one replica becomes unavailable
    - a `DeleteGroup` fails and rolls back during outage
    - the replica returns
    - `DeleteGroup` is retried successfully
    - a later `Put` rewrites the deleted key
    - both runtimes observe the recovered state in order
  - Validation passed with:
    - `cargo test -p simulation-harness turmoil_recovery_preserves_delete_group_retry_then_put_visibility_after_replica_outage -- --nocapture`
    - `cargo test -p simulation-harness`
- Conclusion: the distributed-simulation track now covers outage/recovery
  ordering around `DeleteGroup` retry and later rewrite, not only stale-view
  rejoin behavior.
- Disposition: `advance`
- Next move: choose another recovery family, preferably one that uses Madsim or
  a different operation class from `DeleteGroup`.
