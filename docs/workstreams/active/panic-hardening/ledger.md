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
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/panic-hardening`
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
