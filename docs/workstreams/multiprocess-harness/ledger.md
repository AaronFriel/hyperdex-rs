# Workstream Ledger: multiprocess-harness

### Entry `mph-001` - Preregistration

- Timestamp: `2026-03-27 04:19Z`
- Kind: `preregister`
- Hypothesis: serializing the three process-spawning multiprocess-harness tests
  will remove the current workspace false failure caused by same-process port
  collisions.
- Owner: `root`; matching isolated worktree result available from paused worker
- Start commit: `2e6490e`
- Worktree / branch:
  - root checkout dirty state
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/dist-multiprocess-harness`
- Mutable surface:
  - `Cargo.toml`
  - `crates/server/Cargo.toml`
  - `crates/server/tests/dist_multiprocess_harness.rs`
- Validator:
  - `cargo test -p server --test dist_multiprocess_harness -- --nocapture`
  - `cargo test --workspace`
- Expected artifacts:
  - green multiprocess harness
  - green workspace
  - one bounded commit on `main`

