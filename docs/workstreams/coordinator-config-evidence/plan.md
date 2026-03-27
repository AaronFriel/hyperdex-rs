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
- [x] (2026-03-27 20:43Z) Decoded the captured coordinator bytes as BusyBee
  identify plus Replicant bootstrap traffic and tied the next product target
  to the packed `hyperdex::configuration` body behind the `hyperdex/config`
  follow reply.
- [x] (2026-03-27 20:14Z) Reopened this workstream for a second read-only step
  that compares the Rust `default_legacy_config_encoder` output against the
  original HyperDex `configuration` / `space` packing rules on a live
  `profiles` config body.

## Current Hypothesis

The focused large-object path is still blocked by the coordinator-side packed
`hyperdex::configuration` body that the HyperDex client consumes after
`replicant_client_cond_follow("hyperdex", "config", ...)`. The first captured
pair is only bootstrap traffic. The next likely mismatch is now specific
enough to compare directly: the bytes produced by Rust
`default_legacy_config_encoder` for the live `profiles` schema may not satisfy
the original HyperDex `configuration` and `space` unpackers, especially around
container and map datatype encoding.

## Next Bounded Step

Keep this workstream read-only. Compare the Rust `default_legacy_config_encoder`
output against the original HyperDex `configuration` / `space` packing rules on
a live `profiles` config body, then state the first concrete mismatch if one
exists.

## Surprises & Discoveries

- Observation: the harness can now prove that the failing path never reaches a
  decodable legacy daemon frame.
  Evidence: `853e290` captures only partial BusyBee-style coordinator frames
  on the focused large-object repro.
- Observation: the captured `trailing_bytes=45` and `trailing_bytes=100` values
  are not malformed single frames.
  Evidence: they decompose cleanly into BusyBee identify and Replicant
  bootstrap traffic sizes from the original BusyBee and Replicant sources.
- Observation: the HyperDex client cannot prepare the first atomic write until
  it has successfully unpacked the coordinator's `hyperdex/config` follow
  payload as `hyperdex::configuration`.
  Evidence: `client::maintain_coord_connection` blocks on
  `replicant_client_cond_follow("hyperdex", "config", ...)`, then unpacks
  `m_config_data` into `configuration` before `get_schema`, `point_leader`,
  and `prepare_funcs` are used.

## Decision Log

- Decision: keep the first bounded step read-only.
  Rationale: the highest-value missing information is exact coordinator-side
  evidence, and the active product worker already owns the code surface.
  Date/Author: 2026-03-27 / root
- Decision: keep the second bounded step read-only as well.
  Rationale: the next question is still contract comparison, not product-code
  ownership, and the product worker already owns the code surface where fixes
  will land.
  Date/Author: 2026-03-27 / root

## Outcomes & Retrospective

- The first bounded read-only step finished with a concrete source-backed
  target: the product worker should stop treating the coordinator bootstrap as
  the active mismatch and should instead verify the packed
  `hyperdex::configuration` body returned by the `hyperdex/config` follow reply
  for the full `profiles` schema.
- The second bounded step is now narrower and more useful than generic packet
  decoding: directly compare the Rust packed-config bytes against the original
  HyperDex unpacking rules for the same live `profiles` body.
