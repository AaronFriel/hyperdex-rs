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

### Entry `asm-002` - Outcome

- Timestamp: `2026-03-28 23:55Z`
- Kind: `outcome`
- End commit: `5777625`
- Artifact location:
  - `crates/consensus-core/Cargo.toml`
  - `crates/consensus-core/src/lib.rs`
  - `crates/consensus-core/src/openraft_backend.rs`
  - `crates/transport-core/src/lib.rs`
  - `crates/transport-grpc/src/lib.rs`
  - `crates/server/src/main.rs`
  - `crates/simulation-harness/src/tests/mod.rs`
- Evidence summary:
  - `c2b2c10` removed `async_trait` from `consensus-core`.
  - `5777625` changed `ClusterTransport::send` to return a boxed future, which
    removed the remaining plain `async_trait` use from the transport boundary.
  - The remaining uses on this surface are tonic-generated
    `#[tonic::async_trait]` impls.
- Conclusion: the current async cleanup phase is complete.
- Disposition: `advance`
- Next move: move this workstream out of the active board and revisit only if
  a larger gRPC service redesign becomes worthwhile.
