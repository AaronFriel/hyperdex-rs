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

### Entry `asm-003` - Preregistration

- Timestamp: `2026-03-29 00:20Z`
- Kind: `preregister`
- Hypothesis: the remaining `#[tonic::async_trait]` uses in the gRPC service
  layer may still be removable with a bounded service-adapter change; if not,
  the workstream should return a precise tonic-generated constraint instead of
  treating the cleanup as done.
- Owner: forked worker on `async-modernization`
- Start commit: `fb02bcc`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/async-modernization`
  - `async-modernization`
- Mutable surface:
  - `crates/transport-grpc/**`
  - `crates/server/src/main.rs`
  - `crates/server/src/lib.rs` if needed for service wiring
- Validator:
  - fastest useful check:
    `rg -n "\\#\\[tonic::async_trait\\]|\\#\\[async_trait\\]|async_trait" crates/transport-grpc crates/server/src/main.rs`
  - strong checks:
    - `cargo test -p transport-grpc`
    - `cargo test -p server`
- Expected artifacts:
  - either a code change that removes the remaining tonic async-trait usage
  - or a source-backed blocker showing why tonic's generated service boundary
    still requires it

### Entry `asm-003` - Outcome

- Timestamp: `2026-03-29 01:05Z`
- Kind: `outcome`
- End commit: `7e79838`
- Artifact location:
  - `docs/workstreams/active/async-modernization/plan.md`
  - `target/debug/build/transport-grpc-85a685a876548cf5/out/hyperdex.v1.rs`
  - `/home/friel/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tonic-build-0.12.3/src/server.rs`
- Evidence summary:
  - The remaining uses are only `#[tonic::async_trait]` in
    `crates/transport-grpc/src/lib.rs` and `crates/server/src/main.rs`.
  - The generated gRPC server traits still carry `#[async_trait]` in
    `target/debug/build/*/out/hyperdex.v1.rs`.
  - tonic-build 0.12.3 emits `#[async_trait] pub trait #server_trait` from its
    server codegen path, so the remaining repo-local annotations match a
    generated contract rather than missed cleanup in our own trait surfaces.
- Conclusion: the current async pass ended in a precise tonic-generated
  blocker, not another missing local rewrite.
- Disposition: `reframe`
- Next move: investigate whether tonic/codegen changes or a manual gRPC service
  boundary can remove the final annotations.

### Entry `asm-004` - Preregistration

- Timestamp: `2026-03-29 01:05Z`
- Kind: `preregister`
- Hypothesis: the remaining tonic-generated `#[async_trait]` dependency can
  only be removed by changing code generation or replacing the generated
  service boundary, and one bounded pass can determine whether either path is
  practical without destabilizing the repository.
- Owner: forked worker on `async-modernization`
- Start commit: `7e79838`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/async-modernization`
  - `async-modernization`
- Mutable surface:
  - `crates/transport-grpc/**`
  - `crates/server/src/main.rs`
  - `proto/**`
  - crate manifests or build scripts as needed
- Validator:
  - fastest useful check:
    `rg -n "\\#\\[tonic::async_trait\\]|\\#\\[async_trait\\]|async_trait" crates/transport-grpc crates/server/src/main.rs target/debug/build/transport-grpc-*/out/hyperdex.v1.rs`
  - strong checks:
    - `cargo test -p transport-grpc`
    - `cargo test -p server`
- Expected artifacts:
  - either a code or build change that removes the remaining tonic async-trait
  usage
  - or a precise blocker that names the smallest redesign required to do it

### Entry `asm-004` - Outcome

- Timestamp: `2026-03-29 18:05Z`
- Kind: `outcome`
- End commit: `69020d9`
- Artifact location:
  - `crates/grpc-api/**`
  - `crates/server/src/lib.rs`
  - root `Cargo.toml`
  - `crates/storage-core/Cargo.toml`
  - `crates/transport-core/Cargo.toml`
  - `crates/simulation-harness/Cargo.toml`
- Evidence summary:
  - `11275fd` replaced the build-script rewrite path with the source-controlled
    `grpc-api` crate.
  - `69020d9` removed the remaining source-controlled `async-trait`
    dependency entries and finished the cleanup.
  - `rg -n "async-trait|async_trait" . --glob '!target/**' --glob '!docs/**'`
    is now clean on the merged branch.
- Conclusion: async modernization is complete for the current repository
  design.
- Disposition: `stop`
- Next move: keep this workstream completed unless a later transport redesign
  reintroduces macro-based async traits.
