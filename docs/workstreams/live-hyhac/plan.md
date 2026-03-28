# Workstream Plan: live-hyhac

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `/home/friel/.codex/skills/autoplan/references/PLANS.md`.

## Purpose / Big Picture

This workstream is the direct path to the user-visible objective: a live
`hyperdex-rs` cluster that can run `hyhac` without semantic drift. Its job is
to keep the public compatibility target honest, reduce failures to the smallest
truthful repro, and then land product code that clears those failures.

## Goal

Run `hyhac` against the live cluster, capture the next failing operation or
semantic mismatch, and narrow implementation work to that concrete evidence
until the suite passes.

## Acceptance Evidence

- The `hyhac` harness runs against a real `hyperdex-rs` cluster.
- The next failing operation, if any, is recorded from observed output rather
  than guessed from the code.
- Eventually, the live `hyhac` suite passes.

## Mutable Surface

- `crates/legacy-protocol/**`
- `crates/legacy-frontend/**`
- `crates/hyperdex-admin-protocol/**`
- `crates/hyperdex-client-protocol/**`
- `crates/server/**`
- `/home/friel/c/aaronfriel/hyhac/scripts/**` only when launcher or harness
  wiring must point at `hyperdex-rs`

## Dependencies / Blockers

- None. The current blocker is inside this workstream’s owned product surface.
- Blocker-only outcomes are not acceptable here when the missing capability is
  implementable from repository-local HyperDex sources and the current Rust
  codebase.

## Plan Of Work

Keep one honest live validator on `main`, shorten it only when that directly
improves engineering cycle time, and drive the remaining mismatch through real
product changes in `crates/**`. The current step starts from the full-schema
baseline that already proves one successful Hyhac round-trip. The next product
pass should isolate the first later operation that diverges, patch the server
or protocol behavior that causes it, and rerun the live check before returning.

## Progress

- [x] (2026-03-27 05:48Z) Landed the legacy coordinator admin protocol and
  startup path needed for the original admin tools to create spaces and wait
  for stability against `hyperdex-rs`.
- [x] (2026-03-27 23:05Z) Moved the focused Hyhac path beyond bootstrap and
  coordinator follow traffic so the active failure is no longer in the admin
  path.
- [x] (2026-03-27 23:58Z) Proved the old fast large-object check was invalid
  because it failed before schema creation with immediate `UnknownSpace`.
- [x] (2026-03-28 00:17Z) Replaced that invalid check with a full-schema
  baseline that creates the real 19-attribute `profiles` space, waits until
  stable, proves native C success, and proves one successful Hyhac round-trip.
- [ ] Fix the remaining later Hyhac failure after the first successful
  round-trip on the full-schema baseline.
  Current owner: `019d31bc-e8da-7af3-b40a-bfa04fd8ec4b` (`Gauss`) on
  `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-roundtrip-fix`.

## Current Hypothesis

The remaining live mismatch is later than bootstrap, later than schema
creation, and later than the first daemon round-trip. The shortest honest
public validator now proves that `hyperdex-rs` can serve one full large-object
round-trip to Hyhac on the real `profiles` schema. The next failing operation
after that first success is the one that should drive the next product change.

## Next Bounded Step

Use the full-schema probe as the main validator. Reduce the failure to the
first later operation that diverges after a proven successful round-trip, patch
the responsible product code, and rerun the honest live check plus
`cargo test -p server` before returning control. This step is active now on the
fresh `live-hyhac-roundtrip-fix` worktree.

## Surprises & Discoveries

- Observation: the old fast large-object repro was not a truthful validator.
  Evidence: `eb6d093` proved it failed on immediate `UnknownSpace` before any
  daemon request because the `profiles` space had not been created.
- Observation: the corrected live boundary is materially later than earlier
  bootstrap and coordinator-path failures.
  Evidence: `589ce4f` proves native C success and one successful Hyhac
  `put` plus `loop` before the next operation hangs.
- Observation: the remaining work now belongs primarily in product code, not
  in more coordinator-protocol archaeology.
  Evidence: the active live failure appears after the system already accepts
  schema creation, stability waits, and one large-object client round-trip.

## Decision Log

- Decision: keep this workstream active and give it the next substantial product
  ownership.
  Rationale: it is the closest path to the user-visible goal and now has an
  honest live validator.
  Date/Author: 2026-03-28 / root
- Decision: keep the current live validator truthful even if it is slower than
  the earlier invalid fast check.
  Rationale: shorter but false validators create wasted code motion.
  Date/Author: 2026-03-28 / root

## Outcomes & Retrospective

- The admin/bootstrap rewrite is no longer the active story in this
  workstream. The repository now has a real live baseline where schema
  creation, stability waits, native C writes, and one Hyhac round-trip all
  succeed. The remaining work is to clear the next later client-visible
  divergence on that honest baseline.
