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
