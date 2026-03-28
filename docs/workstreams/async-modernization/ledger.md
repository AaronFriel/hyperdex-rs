# Workstream Ledger: async-modernization

### Entry `asm-001` - Preregistration

- Timestamp: `2026-03-28 10:00Z`
- Kind: `preregister`
- Hypothesis: at least one important async service boundary can be converted
  from `async_trait` to native Rust async traits in one bounded pass without
  destabilizing the repository.
- Owner: next forked worker
- Start commit: `HEAD`
- Worktree / branch:
  - worktree to be created from current `main`
- Mutable surface:
  - `crates/transport-core/**`
  - `crates/consensus-core/**`
  - `crates/hyperdex-admin-protocol/**`
  - `crates/hyperdex-client-protocol/**`
  - `crates/server/**`
- Validator:
  - fastest useful check:
    `rg -n "async_trait|\\#\\[async_trait\\]" crates Cargo.toml`
  - strong checks:
    - `cargo test -p server`
    - `cargo test --workspace`
- Expected artifacts:
  - one coherent native-async conversion
  - reduced or isolated `async_trait` usage
  - one bounded commit ready for reconciliation
