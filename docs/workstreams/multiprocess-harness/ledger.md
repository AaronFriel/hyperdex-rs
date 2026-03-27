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

### Entry `mph-001` - Outcome

- Timestamp: `2026-03-27 04:22Z`
- Kind: `outcome`
- End commit: `98def36`
- Artifact location:
  - `Cargo.toml`
  - `crates/server/Cargo.toml`
  - `crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - `cargo test -p server --test dist_multiprocess_harness -- --nocapture` passed
  - `cargo test --workspace` passed
- Conclusion: the immediate same-process harness collision is contained and the
  workspace is green again.
- Disposition: `advance`
- Next move: preregister the next bounded readiness cleanup in the dedicated
  multiprocess worktree.

### Entry `mph-002` - Preregistration

- Timestamp: `2026-03-27 04:22Z`
- Kind: `preregister`
- Hypothesis: replacing ephemeral port reuse and log-text waits with
  protocol-based readiness checks will keep the multiprocess harness stable
  without further broad serialization.
- Owner: dedicated worker in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/dist-multiprocess-harness`
- Start commit: `98def36`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/dist-multiprocess-harness`
- Mutable surface:
  - `crates/server/tests/dist_multiprocess_harness.rs`
  - `crates/server/src/main.rs` only if the harness truly needs a small startup
    signal change
- Validator:
  - `cargo test -p server --test dist_multiprocess_harness -- --nocapture`
  - `cargo test --workspace`
- Expected artifacts:
  - no same-process port reuse inside multiprocess tests
  - readiness based on observable protocol state rather than log text
  - green multiprocess harness
  - green workspace
  - one bounded commit ready for reconciliation

### Entry `mph-002` - Outcome

- Timestamp: `2026-03-27 04:33Z`
- Kind: `outcome`
- End commit: `faa6cb6`
- Artifact location:
  - `crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - `cargo test -p server --test dist_multiprocess_harness -- --nocapture` passed
  - `cargo test --workspace` passed
- Conclusion: the multiprocess harness now uses held port reservations and
  protocol-based readiness checks, so it no longer depends on ephemeral port
  reuse or log-text polling.
- Disposition: `advance`
- Next move: hold until a new real-cluster failure requires another harness
  change.

### Entry `mph-003` - Preregistration

- Timestamp: `2026-03-27 07:10Z`
- Kind: `preregister`
- Hypothesis: a targeted coordinator-plus-daemon admin probe harness test can
  give the live compatibility workstream a much faster and more repeatable
  signal than the current manual free-port probe sequence, without touching
  product code.
- Owner: forked worker in
  `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-probe-harness`
- Start commit: `4ccf113`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-probe-harness` on
    `admin-probe-harness`
- Mutable surface:
  - `Cargo.toml`
  - `crates/server/Cargo.toml`
  - `crates/server/tests/**`
  - `crates/server/src/main.rs` only if the harness truly needs a small
    readiness adjunct
- Validator:
  - fastest useful check: focused `dist_multiprocess_harness` target for the
    new admin probe
  - strong checks:
    - `cargo test -p server --test dist_multiprocess_harness -- --nocapture`
    - `cargo test --workspace`
- Expected artifacts:
  - a fast coordinator-plus-daemon admin probe harness
  - captured evidence about whether the C admin client advances beyond
    bootstrap
  - one bounded commit ready for reconciliation

### Entry `mph-003` - Outcome

- Timestamp: `2026-03-27 07:20Z`
- Kind: `outcome`
- End commit: `6f061b3`
- Artifact location:
  - `crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - `cargo test -p server --test dist_multiprocess_harness legacy_admin_wait_until_stable_probe_reports_bootstrap_progress -- --nocapture` passed
  - the focused test reports `advanced=false` on current `main`
  - the captured frame summary from that test shows the first meaningful
    server reply is still `ClientResponse`
  - `cargo test -p server --test dist_multiprocess_harness -- --nocapture`
    passed
  - `cargo test --workspace` passed
- Conclusion: the fast admin-probe harness is now on `main`, and it provides
  the shortest trustworthy check for whether the original C admin client
  progresses beyond bootstrap.
- Disposition: `advance`
- Next move: hold until the product worker or another real-cluster failure
  needs more harness work.
