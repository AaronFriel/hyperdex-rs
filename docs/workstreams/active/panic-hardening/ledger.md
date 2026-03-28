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
