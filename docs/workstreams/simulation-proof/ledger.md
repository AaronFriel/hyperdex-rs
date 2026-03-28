# Workstream Ledger: simulation-proof

### Entry `sim-001` - Preregistration

- Timestamp: `2026-03-27 04:19Z`
- Kind: `preregister`
- Hypothesis: a Hegel-backed routed numeric-mutation property over
  `profile_views` will prove the distributed atomic-add path without widening
  the mutable surface beyond `simulation-harness`.
- Owner: `root`; matching isolated worktree result available from paused worker
- Start commit: `2e6490e`
- Worktree / branch:
  - root checkout dirty state
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/sim-coverage` on
    `sim-coverage-numeric`
- Mutable surface:
  - `crates/simulation-harness/src/lib.rs`
- Validator:
  - `cargo test -p simulation-harness hegel_distributed_runtime_routes_numeric_mutation -- --nocapture`
  - `cargo test -p simulation-harness`
  - `cargo test --workspace`
- Expected artifacts:
  - green targeted numeric-mutation proof
  - green `simulation-harness`
  - green workspace
  - one bounded commit on `main`

### Entry `sim-001` - Outcome

- Timestamp: `2026-03-27 04:22Z`
- Kind: `outcome`
- End commit: `6d55620`
- Artifact location:
  - `crates/simulation-harness/src/lib.rs`
- Evidence summary:
  - `cargo test -p simulation-harness hegel_distributed_runtime_routes_numeric_mutation -- --nocapture` passed
  - `cargo test -p simulation-harness` passed
  - `cargo test --workspace` passed
- Conclusion: the routed numeric-mutation property is now on `main` and the
  workspace stays green with it.
- Disposition: `advance`
- Next move: preregister the single-node schema-correctness cleanup as the next
  bounded proof step in the dedicated worktree.

### Entry `sim-002` - Preregistration

- Timestamp: `2026-03-27 04:22Z`
- Kind: `preregister`
- Hypothesis: making the single-node Hegel sequence model use declared
  attributes instead of the undeclared `name` field will tighten generated
  coverage without widening the mutable surface beyond `simulation-harness`.
- Owner: dedicated worker in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/sim-coverage`
- Start commit: `6d55620`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/sim-coverage` on
    `sim-coverage-numeric`
- Mutable surface:
  - `crates/simulation-harness/src/lib.rs`
- Validator:
  - `cargo test -p simulation-harness hegel_single_node_runtime_matches_sequence_model -- --nocapture`
  - `cargo test -p simulation-harness`
  - `cargo test --workspace`
- Expected artifacts:
  - schema-correct single-node Hegel sequence property
  - green `simulation-harness`
  - green workspace
  - one bounded commit ready for reconciliation

### Entry `sim-002` - Outcome

- Timestamp: `2026-03-27 04:33Z`
- Kind: `outcome`
- End commit: `5cc0cf8`
- Artifact location:
  - `crates/simulation-harness/src/lib.rs`
- Evidence summary:
  - `cargo test -p simulation-harness hegel_single_node_runtime_matches_sequence_model -- --nocapture` passed
  - `cargo test -p simulation-harness` passed
  - `cargo test --workspace` passed
- Conclusion: the single-node Hegel sequence model now uses declared schema
  attributes and no longer relies on the permissive undeclared `name` field.
- Disposition: `advance`
- Next move: hold until the live compatibility thread exposes the next proof
  gap worth capturing deterministically.

### Entry `sim-003` - Preregistration

- Timestamp: `2026-03-28 03:27Z`
- Kind: `preregister`
- Hypothesis: the deterministic degraded-read simulation can be brought back in
  line with current distributed runtime behavior by fixing replica fallback or
  simulated transport handling inside the simulation-owned surface.
- Owner: delegated worker in
  `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/simulation-degraded-read-fix`
- Start commit: `281b8cb`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/simulation-degraded-read-fix`
    on `simulation-degraded-read-fix`
- Mutable surface:
  - `crates/simulation-harness/**`
  - `crates/server/**` only if the simulation failure exposes a real runtime
    bug instead of a stale proof assumption
- Validator:
  - fastest useful check:
    `cargo test -p simulation-harness turmoil_preserves_degraded_read_correctness_after_one_node_loss -- --nocapture`
  - strong checks:
    - `cargo test -p simulation-harness`
    - `cargo test --workspace`
- Expected artifacts:
  - a substantive fix for the degraded-read simulation failure
  - green `simulation-harness`
  - green workspace or an exact remaining blocker with proof
