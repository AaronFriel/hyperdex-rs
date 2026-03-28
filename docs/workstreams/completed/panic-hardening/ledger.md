# Workstream Ledger: panic-hardening

### Entry `pnh-001` - Preregistration

- Timestamp: `2026-03-28 10:40Z`
- Kind: `preregister`
- Hypothesis: one bounded pass over startup or public-runtime entry points can
  remove meaningful panic sites, introduce the first practical `#[no_panic]`
  contracts, and set the next lint ratchet without broad repository churn.
- Owner: forked worker on `panic-hardening`
- Start commit: `9104047`
- Worktree / branch:
  - `worktrees/panic-hardening`
  - `panic-hardening`
- Mutable surface:
  - `crates/server/**`
  - `crates/legacy-frontend/**`
  - `crates/legacy-protocol/**`
  - `crates/hyperdex-admin-protocol/**`
  - manifests or lint config as needed
- Validator:
  - fastest useful check:
    `rg -n "unwrap\\(|expect\\(|todo!|panic!|no_panic" crates/server crates/legacy-frontend crates/legacy-protocol crates/hyperdex-admin-protocol`
  - strong checks:
    - `cargo test -p server`
    - `cargo test --workspace`
- Expected artifacts:
  - one bounded panic-hardening commit
  - the first practical `#[no_panic]` usage or explicit justification for the
    next surface
  - the next lint-ratchet step defined from real code

### Entry `pnh-001` - Outcome

- Timestamp: `2026-03-28 23:55Z`
- Kind: `outcome`
- End commit: `694545e`
- Artifact location:
  - `crates/legacy-protocol/src/lib.rs`
- Evidence summary:
  - Removed a broad set of fixed-width decode `expect` paths from
    `legacy-protocol`.
  - Replaced them with checked readers and explicit error returns.
- Conclusion: the first panic-hardening pass landed on the legacy protocol
  boundary.
- Disposition: `advance`
- Next move: harden another public decoder or startup boundary.

### Entry `pnh-002` - Outcome

- Timestamp: `2026-03-28 23:55Z`
- Kind: `outcome`
- End commit: `44f8c58`
- Artifact location:
  - `crates/hyperdex-admin-protocol/src/lib.rs`
  - `crates/hyperdex-admin-protocol/src/tests/mod.rs`
- Evidence summary:
  - Removed `unwrap` panic paths from BusyBee frame decoding, bootstrap socket
    decoding, and packed-space decoder readers.
  - Added regression tests for truncated input cases.
  - `#[no_panic]` was attempted on tiny encoder methods and failed at link
    time, so the annotations were not kept.
- Conclusion: the second panic-hardening pass landed with concrete no-panic
  evidence about the current boundary limits.
- Disposition: `advance`
- Next move: move to `server/src/main.rs` or `legacy-frontend` for the next
  bounded hardening pass.

### Entry `pnh-003` - Preregistration

- Timestamp: `2026-03-29 00:25Z`
- Kind: `preregister`
- Hypothesis: `legacy-frontend` still has public-boundary `expect` paths that
  can be converted to checked decoding with a practical no-panic contract on a
  smaller surface than `server/src/main.rs`.
- Owner: forked worker on `panic-hardening`
- Start commit: `fb02bcc`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/panic-hardening`
  - `panic-hardening`
- Mutable surface:
  - `crates/legacy-frontend/**`
  - `crates/legacy-protocol/**` only if helper changes are needed
  - manifests or lint config as needed
- Validator:
  - fastest useful check:
    `cargo test -p legacy-frontend`
  - strong checks:
    - `cargo test -p server`
    - `cargo test --workspace`
    - `rg -n "unwrap\\(|expect\\(|todo!|panic!|no_panic" crates/legacy-frontend crates/legacy-protocol`
- Expected artifacts:
  - one bounded hardening pass over the legacy frontend boundary
  - either a practical `#[no_panic]` annotation or a concrete justification for
    leaving it off this surface
  - one bounded commit ready for reconciliation

### Entry `pnh-003` - Outcome

- Timestamp: `2026-03-29 00:53Z`
- Kind: `outcome`
- End commit: `dd00c13`
- Artifact location:
  - `crates/legacy-frontend/src/lib.rs`
- Evidence summary:
  - Replaced the legacy frontend identify-path
    `expect(\"fixed-width slice\")` with a checked
    `decode_identify_remote_server_id` helper.
  - `cargo test -p legacy-frontend` passed after the change.
  - `cargo test -p server` passed after the change, including the live-cluster
    multiprocess tail.
  - A narrow `#[no_panic]` attempt on the helper was tried and rejected:
    the `no-panic` link-time check reported
    `ERROR[no-panic]: detected panic in function decode_identify_remote_server_id`,
    so the annotation was not kept.
- Conclusion: the public identify decode panic site is removed, and this pass
  adds concrete evidence that `#[no_panic]` is still not practical on this
  boundary in its current form.
- Disposition: `advance`
- Next move: choose the next public/runtime boundary with multiple remaining
  panic sites instead of retrying the same helper-level annotation.

### Entry `pnh-004` - Preregistration

- Timestamp: `2026-03-29 01:05Z`
- Kind: `preregister`
- Hypothesis: `server/src/main.rs` still has public entrypoint panic paths
  around validated socket addresses and daemon identity that can be converted
  to checked startup errors without broad churn.
- Owner: forked worker on `panic-hardening`
- Start commit: `7e79838`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/panic-hardening`
  - `panic-hardening`
- Mutable surface:
  - `crates/server/src/main.rs`
  - `crates/server/src/lib.rs` only if startup helpers need a checked result
  - manifests or lint config as needed
- Validator:
  - fastest useful check:
    `cargo test -p server`
  - strong checks:
    - `cargo test --workspace`
    - `rg -n "unwrap\\(|expect\\(|todo!|panic!|no_panic" crates/server/src/main.rs crates/server/src/lib.rs`
- Expected artifacts:
  - one bounded startup hardening pass over `server/src/main.rs`
  - either a practical `#[no_panic]` annotation or a concrete justification for
    leaving it off this boundary
  - one bounded commit ready for reconciliation

### Entry `pnh-004` - Outcome

- Timestamp: `2026-03-29 10:05Z`
- Kind: `outcome`
- End commit: `db696ce`
- Artifact location:
  - `crates/server/src/main.rs`
  - `crates/server/src/lib.rs`
- Evidence summary:
  - Replaced startup `expect("validated socket address")` paths with
    `parse_socket_address(...)` plus `anyhow` context.
  - Replaced `expect("daemon mode has a node identity")` in the daemon path
    with checked startup error handling.
  - Removed `expect("daemon mode has a node")` from `daemon_cluster_config` by
    deriving the daemon node directly from the matched `ProcessMode`.
  - `cargo test -p server` and `cargo test --workspace` both passed after the
    change.
  - A temporary `#[no_panic]` attempt on `daemon_registration_node` failed at
    link time and was intentionally removed from the final commit.
- Conclusion: the public startup boundary now returns checked errors instead of
  panicking on validated socket-address and daemon-identity assumptions.
- Disposition: `advance`
- Next move: move deeper into `server/src/lib.rs` for the next product-only
  panic sites.

### Entry `pnh-005` - Preregistration

- Timestamp: `2026-03-29 10:05Z`
- Kind: `preregister`
- Hypothesis: `server/src/lib.rs` still has several meaningful product-only
  panic sites in fixed-width legacy decode helpers and poisoned-lock access
  that can be converted to checked behavior in one bounded pass.
- Owner: forked worker on `panic-hardening`
- Start commit: `db696ce`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/panic-hardening`
  - `panic-hardening`
- Mutable surface:
  - `crates/server/src/lib.rs`
  - tests only if needed for the chosen boundary
- Validator:
  - fastest useful check:
    `cargo test -p server`
  - strong checks:
    - `cargo test --workspace`
    - `rg -n "unwrap\\(|expect\\(|todo!|panic!|no_panic" crates/server/src/lib.rs`
- Expected artifacts:
  - one bounded `server/src/lib.rs` hardening pass
  - either a practical `#[no_panic]` annotation or a concrete justification for
    leaving it off the chosen helper
  - one bounded commit ready for reconciliation

### Entry `pnh-005` - Outcome

- Timestamp: `2026-03-29 18:05Z`
- Kind: `outcome`
- End commit: `20c6d71`
- Artifact location:
  - `crates/server/src/lib.rs`
  - `crates/hyperdex-admin-protocol/src/lib.rs`
  - `crates/legacy-frontend/src/lib.rs`
  - `crates/placement-core/src/lib.rs`
- Evidence summary:
  - `ea85af6` replaced fixed-width server decode panic paths with checked
    readers.
  - `e49866d` replaced poisoned-lock panic paths in the coordinator and search
    state with checked errors.
  - `621692b` returned placement errors instead of panicking on empty layouts.
  - `f9f76af` rejected oversized BusyBee frames before payload allocation.
  - `20c6d71` removed the remaining non-test placement panic fallback.
- Conclusion: the main product-code unwrap/expect and panic cleanup phase is
  substantially complete.
- Disposition: `advance`
- Next move: shift from raw panic removal to a narrow `#[no_panic]` or Clippy
  ratchet step on a pure boundary.

### Entry `pnh-006` - Preregistration

- Timestamp: `2026-03-29 18:20Z`
- Kind: `preregister`
- Hypothesis: a small pure decoder or parser boundary can now carry either a
  real `#[no_panic]` contract or a crate-level Clippy ratchet against
  `unwrap` and `expect` without destabilizing the runtime.
- Owner: next forked worker on `panic-hardening`
- Start commit: `2b7d144`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/panic-hardening`
  - `panic-hardening`
- Mutable surface:
  - `crates/hyperdex-admin-protocol/**`
  - `crates/legacy-protocol/**`
  - `crates/legacy-frontend/**`
  - lint configuration only if the chosen ratchet is practical
- Validator:
  - fastest useful check:
    `cargo clippy -p hyperdex-admin-protocol -p legacy-protocol -p legacy-frontend --lib -- -W clippy::unwrap_used -W clippy::expect_used`
  - strong checks:
    - `cargo test -p hyperdex-admin-protocol -p legacy-protocol -p legacy-frontend`
    - `cargo test --workspace`
- Expected artifacts:
  - one narrow `#[no_panic]` or Clippy ratchet step on a pure boundary
  - focused regression tests if needed
  - one bounded commit ready for reconciliation
