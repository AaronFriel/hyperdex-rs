# Workstream Plan: coordinator-config-evidence

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `/home/friel/.codex/skills/autoplan/references/PLANS.md`.

## Purpose / Big Picture

This workstream exists to keep the product worker from guessing at the
coordinator-side contract once the large-object `ClientGarbage` failure moved
earlier than the daemon request path. Its job is to turn the harness capture
and the original HyperDex sources into exact protocol evidence about packed
coordinator config and the client-side request-preparation contract for the
full `profiles` schema.

## Goal

Identify the exact coordinator-side packet or schema-contract mismatch that
prevents the focused large-object path from ever reaching `REQ_ATOMIC`.

## Acceptance Evidence

- The workstream names the exact coordinator-side exchange and the specific
  schema/config contract the original client expects.
- The result is tied to the harness capture, the original HyperDex sources,
  and the current Rust implementation rather than guesswork.
- The product worker gets a concrete target for the next code change.

## Mutable Surface

- none for the first bounded step; this is a read-only evidence pass
- if a later bounded step needs a tiny helper or diagnostic test, root must
  preregister that separately before any code changes begin

## Dependencies / Blockers

- Depends on the harness capture now on `main` in
  `legacy_hyhac_large_object_probe_reports_first_coordinator_frame_pair`.
- Should not overlap with the active product worker’s write surface.

## Plan Of Work

Use the focused harness capture, the original HyperDex coordinator/client
sources, and the current Rust packed-config path to decode what the client is
doing on the coordinator connection before the large-object path fails. The
first bounded step is read-only and should return the exact meaning of the
captured partial BusyBee-style frames and the likely packed config or schema
contract the original client is consuming before it decides whether to send the
first atomic write.

## Progress

- [x] (2026-03-27 20:04Z) Created this workstream after the harness and product
  worker both showed that the large-object failure still occurs before the
  daemon sees `REQ_ATOMIC`.

## Current Hypothesis

The focused large-object path is still blocked by a coordinator-side contract,
not by the daemon request decoder. The likely fault line is packed config or
schema metadata for the full `profiles` space, especially container and map
datatype encoding.

## Next Bounded Step

Decode the first captured coordinator frame pair against the original
HyperDex/Replicant sources and the current Rust packed-config path, then state
the exact coordinator-side contract the client is still missing.

## Surprises & Discoveries

- Observation: the harness can now prove that the failing path never reaches a
  decodable legacy daemon frame.
  Evidence: `853e290` captures only partial BusyBee-style coordinator frames
  on the focused large-object repro.

## Decision Log

- Decision: keep the first bounded step read-only.
  Rationale: the highest-value missing information is exact coordinator-side
  evidence, and the active product worker already owns the code surface.
  Date/Author: 2026-03-27 / root

## Outcomes & Retrospective

- None yet. This workstream starts from the harness result in `853e290` and the
  product blocker recorded in `hyh-033`.
