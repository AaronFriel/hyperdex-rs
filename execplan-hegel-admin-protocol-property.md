# Add a Hegel property for HyperDex admin request semantics

This ExecPlan is a living document. The sections `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work proceeds.

This repository does not contain a checked-in `PLANS.md`, so this document follows the fallback ExecPlan rules from the local Codex `execplan` skill reference.

## Purpose / Big Picture

After this change, `hyperdex-admin-protocol` will have generated coverage for a real protocol boundary instead of only example fixtures. The new property will prove that supported Replicant admin requests survive wire encode/decode without changing meaning and still map to the same coordinator request for HyperDex admin operations such as `wait_until_stable`, `config_get`, `space_rm`, and `space_add`.

## Progress

- [x] 2026-03-28 23:32Z Confirmed the worktree is synced to current `main` and chose `crates/hyperdex-admin-protocol` as the next uncovered Hegel boundary.
- [x] 2026-03-28 23:35Z Added `hegeltest` to `crates/hyperdex-admin-protocol/Cargo.toml`, registered `src/tests/hegel_properties.rs`, and implemented the generated property.
- [x] 2026-03-28 23:36Z Ran the narrow validator `cargo test -p hyperdex-admin-protocol hegel_admin_requests_round_trip_and_preserve_supported_coordinator_mapping -- --nocapture`.
- [x] 2026-03-28 23:36Z Ran `cargo test -p hyperdex-admin-protocol` successfully.
- [ ] Create the bounded commit for this property pass.

## Surprises & Discoveries

- Observation: `origin/main` was not the true current baseline for this machine; local `main` was one commit ahead.
  Evidence: rebasing onto `origin/main` reported no-op, while rebasing onto `main` moved `active-hegel-properties` to commit `431887e`.
- Observation: the new property passed on the first narrow validation loop once the generated packed-space fixtures were wired through the existing helper encoder.
  Evidence: `cargo test -p hyperdex-admin-protocol hegel_admin_requests_round_trip_and_preserve_supported_coordinator_mapping -- --nocapture` completed with `1 passed; 0 failed`.

## Decision Log

- Decision: target `crates/hyperdex-admin-protocol` instead of `transport-core`, `control-plane`, or `storage-core`.
  Rationale: this crate has an uncovered, high-value correctness boundary where the wire codec and coordinator mapping must agree on request meaning, and it already contains deterministic fixtures that can anchor a generated property.
  Date/Author: 2026-03-28 / Codex
- Decision: place the new property in `crates/hyperdex-admin-protocol/src/tests/hegel_properties.rs` instead of extending the existing 600+ line `src/tests/mod.rs`.
  Rationale: the repository rule for large Rust files favors external modules, and the dedicated file keeps generated-property logic separate from the example fixtures it reuses.
  Date/Author: 2026-03-28 / Codex

## Outcomes & Retrospective

The property landed as `hegel_admin_requests_round_trip_and_preserve_supported_coordinator_mapping` in `crates/hyperdex-admin-protocol/src/tests/hegel_properties.rs`. It now proves that generated Replicant admin requests round-trip exactly through the wire codec and preserve coordinator meaning for `wait_until_stable`, `config_get`, `space_rm`, and both `Call` and `CallRobust` forms of `space_add`. The package-level validator also stayed green, so the change expanded Hegel into a new protocol boundary without destabilizing the crate.

## Context and Orientation

`crates/hyperdex-admin-protocol/src/lib.rs` implements the HyperDex admin compatibility codec. The key boundary for this task is `ReplicantAdminRequestMessage`, which can encode admin requests to wire bytes, decode those bytes back into structured requests, and map supported requests into `CoordinatorAdminRequest` values with `into_coordinator_request()`.

`crates/hyperdex-admin-protocol/src/tests/mod.rs` already contains example-based tests plus helper routines that can pack HyperDex space metadata into the legacy binary format expected by `decode_packed_hyperdex_space()`. The new property should reuse that packing path so it validates real protocol bytes instead of a separate model-only representation.

## Plan of Work

Create a small Hegel-backed test module under `crates/hyperdex-admin-protocol/src/tests/` and register it from `src/tests/mod.rs`. Add `hegeltest` as a dev-dependency in the crate manifest.

In the new property module, generate several supported request shapes. The property will cover transport-only `get_robust_params`, supported coordinator-mapped `wait_until_stable` and `config_get` condition waits, `space_rm`, and both `Call` and `CallRobust` forms of `space_add`. For `space_add`, generate valid packed-space payloads and compute the exact `Space` value that the decoder should produce. For each generated message, encode it to bytes, decode it back, assert exact equality, re-encode to confirm canonical bytes, and then assert either the expected coordinator request or the expected transport-only behavior.

## Concrete Steps

From `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/active-hegel-properties`:

    cargo test -p hyperdex-admin-protocol hegel_admin_requests_round_trip_and_preserve_supported_coordinator_mapping -- --nocapture

Expected result after implementation: one Hegel property passes and prints the normal Rust test success line for that test name.

Then run:

    cargo test -p hyperdex-admin-protocol

Expected result after implementation: the crate test suite passes with the new property included.

## Validation and Acceptance

Acceptance is that generated Replicant admin requests keep the same meaning across the full wire boundary. The new property must fail if encode/decode changes bytes or structure, or if supported requests no longer map to the same `CoordinatorAdminRequest`. The narrow property command must pass, and the full `hyperdex-admin-protocol` package tests must also pass.

## Idempotence and Recovery

The test commands are safe to rerun. If a generated case exposes a bug, keep the failing seed output, fix the codec or generator, and rerun the narrow property command before rerunning the full package tests.

## Artifacts and Notes

Initial sync evidence:

    git -C /home/friel/c/aaronfriel/hyperdex-rs/worktrees/active-hegel-properties log --oneline --decorate --max-count=1
    431887e (HEAD -> active-hegel-properties, main) Clarify AutoPlan operating model in AGENTS

Validation evidence:

    cargo test -p hyperdex-admin-protocol hegel_admin_requests_round_trip_and_preserve_supported_coordinator_mapping -- --nocapture
    test tests::hegel_properties::hegel_admin_requests_round_trip_and_preserve_supported_coordinator_mapping ... ok

    cargo test -p hyperdex-admin-protocol
    test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

## Interfaces and Dependencies

`crates/hyperdex-admin-protocol/Cargo.toml` must contain a test-only dependency on `hegeltest` exposed as `hegel`.

`crates/hyperdex-admin-protocol/src/tests/hegel_properties.rs` must define a property test named `hegel_admin_requests_round_trip_and_preserve_supported_coordinator_mapping`.

`crates/hyperdex-admin-protocol/src/tests/mod.rs` must register the new module with `mod hegel_properties;`.

Change note: created the initial ExecPlan after syncing the worktree and selecting the target crate so the implementation and validation path are explicit for the remaining work.
Change note: updated the ExecPlan after landing the property and running validators so the remaining work is reduced to creating the bounded commit.
