# Workstream Ledger: distributed-simulation

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
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/recovery-ordering`
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
