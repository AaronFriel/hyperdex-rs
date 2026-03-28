# Workstream Plan: async-modernization

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `the installed `autoplan` skill fallback rules`.

## Purpose / Big Picture

This workstream reduces implementation drag by replacing macro-based async
traits where modern Rust can express the same contracts directly. Its job is to
improve compile-time ergonomics and make async call stacks easier to debug.

## Goal

Remove or tightly justify the remaining `#[tonic::async_trait]` usage in the
gRPC service layer while preserving the existing behavior with green tests.

## Acceptance Evidence

- One meaningful `async_trait`-based surface is replaced with native Rust async
  traits.
- The repository builds and the relevant tests pass after that removal.
- The dependency surface is reduced or more isolated than before.

## Mutable Surface

- `crates/transport-grpc/**`
- `crates/server/src/main.rs`
- `crates/server/src/lib.rs` if service wiring needs to move
- `Cargo.toml` and crate manifests as needed

## Dependencies / Blockers

- The remaining surface is defined by tonic-generated server traits.

## Plan Of Work

Start where the change will matter across crate boundaries, but keep the first
pass bounded enough to land. Prefer one coherent surface over scattered local
edits.

## Progress

- [x] (2026-03-28 10:00Z) Created the workstream and made it an active root
  priority for the next phase.
- [x] (2026-03-28 18:15Z) Bound this workstream to the dedicated
  `worktrees/async-modernization` checkout for one owned fork.
- [x] (2026-03-28 23:35Z) Removed `async_trait` from the admin/client protocol
  service traits and their server implementations in `ef0879f`.
- [x] (2026-03-28 23:55Z) Removed `async_trait` from `consensus-core` and then
  from the transport boundary itself in `c2b2c10` and `5777625`.
- [x] (2026-03-29 10:10Z) Replaced tonic-generated async-trait service impls
  with generated `BoxFuture` server traits and boxed `Send` futures in
  `e9bb387`.

## Current Hypothesis

The main source-level cleanup is now complete. The remaining `async_trait`
text is only inside the build-script rewrite patterns that convert tonic's
generated server traits into `BoxFuture`-returning traits. That is grep noise,
not live async-trait usage in the runtime or generated outputs.

## Next Bounded Step

Decide whether the remaining build-script string literals are worth a final
grep-purity cleanup, or move this workstream back out of the active board once
root is satisfied with the generated-output and runtime state.

## Surprises & Discoveries

- Observation: `async_trait` is still spread across several major crates.
  Evidence: `rg -n "async_trait|\\#\\[async_trait\\]" crates Cargo.toml`
  reports uses in transport, consensus, client/admin protocol, and server
  crates.
- Observation: after the first cleanup passes, only `#[tonic::async_trait]`
  remains.
  Evidence: `rg -n "\\#\\[tonic::async_trait\\]|\\#\\[async_trait\\]|async_trait"
  crates/transport-grpc crates/server/src/main.rs` now reports only
  `crates/server/src/main.rs:43` and `crates/transport-grpc/src/lib.rs`.
- Observation: tonic-build 0.12.3 still generates `#[async_trait] pub trait`
  server traits for the gRPC boundary.
  Evidence:
  `/home/friel/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tonic-build-0.12.3/src/server.rs:223`
  emits `#[async_trait] pub trait #server_trait`, and the generated
  `target/debug/build/*/out/hyperdex.v1.rs` files contain
  `#[async_trait] pub trait HyperdexAdmin`, `HyperdexClient`, and
  `InternodeTransport`.

## Decision Log

- Decision: make async cleanup active now instead of deferring it until later.
  Rationale: this is low-ceremony technical debt removal that should make the
  next feature work easier to debug and maintain.
  Date/Author: 2026-03-28 / root

## Outcomes & Retrospective

- `ef0879f` removed `async_trait` from `hyperdex-admin-protocol`,
  `hyperdex-client-protocol`, and the corresponding `ClusterRuntime`
  implementations.
- `c2b2c10` removed `async_trait` from `consensus-core`.
- `5777625` redesigned the transport boundary so `ClusterTransport::send`
  returns a boxed future and no longer needs `async_trait`.
- The next phase is narrower and more honest than the previous closeout:
  determine whether tonic service impls can also be modernized, or explicitly
  stop there with source-backed justification.
- The redesign path succeeded: the generated server traits now return
  `BoxFuture` and the runtime no longer uses `#[tonic::async_trait]`.
