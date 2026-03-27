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

