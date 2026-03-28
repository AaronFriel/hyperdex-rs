# Workstream Plan: async-modernization

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `/home/friel/.codex/skills/autoplan/references/PLANS.md`.

## Purpose / Big Picture

This workstream reduces implementation drag by replacing macro-based async
traits where modern Rust can express the same contracts directly. Its job is to
improve compile-time ergonomics and make async call stacks easier to debug.

## Goal

Remove `async_trait` from at least one meaningful cross-crate surface and
preserve the existing behavior with green tests.

## Acceptance Evidence

- One meaningful `async_trait`-based surface is replaced with native Rust async
  traits.
- The repository builds and the relevant tests pass after that removal.
- The dependency surface is reduced or more isolated than before.

## Mutable Surface

- `crates/consensus-core/**`
- `crates/transport-core/**`
- `crates/hyperdex-admin-protocol/**`
- `crates/hyperdex-client-protocol/**`
- `crates/server/**`
- `Cargo.toml` and crate manifests as needed

## Dependencies / Blockers

- None.

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
- [ ] Choose the next meaningful cross-crate surface after the protocol traits
  and repeat the conversion with a similarly small validator.

## Current Hypothesis

The protocol service boundary was a good first target. The next useful pass is
likely in transport or consensus traits, where the cross-crate surface is
still shared enough to matter and still small enough to land cleanly.

## Next Bounded Step

Pick the next cross-crate async trait surface after the protocol traits,
convert it to native Rust async traits end to end, and validate the result
without broad unrelated refactoring.

## Surprises & Discoveries

- Observation: `async_trait` is still spread across several major crates.
  Evidence: `rg -n "async_trait|\\#\\[async_trait\\]" crates Cargo.toml`
  reports uses in transport, consensus, client/admin protocol, and server
  crates.

## Decision Log

- Decision: make async cleanup active now instead of deferring it until later.
  Rationale: this is low-ceremony technical debt removal that should make the
  next feature work easier to debug and maintain.
  Date/Author: 2026-03-28 / root

## Outcomes & Retrospective

- `ef0879f` removed `async_trait` from `hyperdex-admin-protocol`,
  `hyperdex-client-protocol`, and the corresponding `ClusterRuntime`
  implementations.
