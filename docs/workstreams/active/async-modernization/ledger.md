# Workstream Ledger: async-modernization

### Entry `asm-001` - Preregistration

- Timestamp: `2026-03-28 10:00Z`
- Kind: `preregister`
- Hypothesis: at least one important async service boundary can be converted
  from `async_trait` to native Rust async traits in one bounded pass without
  destabilizing the repository.
- Owner: forked worker on `async-modernization`
- Start commit: `9104047`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/async-modernization`
  - `async-modernization`
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

### Entry `asm-001` - Outcome

- Timestamp: `2026-03-28 23:35Z`
- Kind: `outcome`
- End commit: `ef0879f`
- Artifact location:
  - `crates/hyperdex-admin-protocol/Cargo.toml`
  - `crates/hyperdex-admin-protocol/src/lib.rs`
  - `crates/hyperdex-client-protocol/Cargo.toml`
  - `crates/hyperdex-client-protocol/src/lib.rs`
  - `crates/server/src/lib.rs`
- Evidence summary:
  - The admin/client protocol service traits now use native `async fn` traits.
  - The matching `ClusterRuntime` implementations no longer require
    `#[async_trait]`.
  - `async-trait` was removed from the two protocol crate manifests.
- Conclusion: the first bounded async cleanup pass landed cleanly on a
  coherent cross-crate boundary.
- Disposition: `advance`
- Next move: choose the next shared async surface, likely transport or
  consensus.
