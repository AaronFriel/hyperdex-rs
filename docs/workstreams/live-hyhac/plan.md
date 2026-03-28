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
baseline that already proves the large-object write path for both pooled and
shared clients, and now also proves pooled `roundtrip`, `conditional`,
`search`, `count`, integer atomics, and float atomics. The next product pass
should clear the later map-valued atomic failures and rerun the live check
before returning.

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
- [x] (2026-03-28 00:48Z) Landed the concurrent-connection fix in
  `legacy-frontend`, and the full-schema large-object baseline now passes on
  integrated `main`.
- [x] (2026-03-28 01:02Z) Ran the broader full-schema pooled Hyhac surface and
  found the next honest failure: `roundtrip` is the first later pooled failure
  and returns `ClientReconfigure`.
- [x] (2026-03-28 01:18Z) Landed a sparse-record legacy `get` fix that keeps
  the large-object boundary green and moves the broader pooled live boundary
  forward: `roundtrip` and `conditional` now pass.
- [x] (2026-03-28 02:35Z) Landed `83e6003`, which fixes legacy integer
  `div`/`mod` semantics and moves the honest pooled baseline through integer
  and float atomic sections.
- [ ] Land the next product fix for map-valued atomic mutation and move the
  live baseline forward again.

## Current Hypothesis

The remaining live mismatch is later than bootstrap, later than schema
creation, later than the first daemon round-trip, later than the
large-object post-success stall that `3c72516` removed, and later than the
pooled `roundtrip` reconfigure that `b23458c` fixed. On the honest
full-schema baseline, pooled `search`, `count`, integer atomics, and float
atomics now pass too. The next failures are map-valued atomic mutation.

## Next Bounded Step

Keep the full-schema large-object probe green as a regression check. Keep the
broader pooled live check as the honest surface, use the supporting harness
workstream only if it materially shortens the map-atomic boundary without
losing truthful setup, and launch the next product-owned passes on the map
numeric and map string mutation surfaces in parallel.

## Surprises & Discoveries

- Observation: the old fast large-object repro was not a truthful validator.
  Evidence: `eb6d093` proved it failed on immediate `UnknownSpace` before any
  daemon request because the `profiles` space had not been created.
- Observation: the corrected live boundary is materially later than earlier
  bootstrap and coordinator-path failures.
  Evidence: `589ce4f` proves native C success and one successful Hyhac
  `put` plus `loop` before the next operation hangs.
- Observation: that later large-object hang was caused by connection handling
  in the legacy frontend rather than by daemon storage or coordinator setup.
  Evidence: after `3c72516`, the focused `legacy-frontend` regression passes
  and the full-schema Hyhac large-object probe completes both pooled and shared
  writes successfully on integrated `main`.
- Observation: the next honest failure is pooled `roundtrip`, not another
  startup or setup problem.
  Evidence: on a live cluster with the full `profiles` schema already added and
  stable, `--select-tests='*pooled*'` reports `Can store a large object: [OK]`
  and then fails first at `roundtrip` with `ClientReconfigure`.
- Observation: sparse record reads were one real source of that pooled
  `ClientReconfigure` path.
  Evidence: after `b23458c`, `legacy_get_fills_defaults_for_sparse_record_attributes`
  passes and the broader pooled live run now reports both `roundtrip: [OK]`
  and `conditional: [OK]`.
- Observation: the honest pooled boundary moved forward again after the
  integer `div`/`mod` fix.
  Evidence: `83e6003` is on `main`; the focused pooled integer-div probe now
  shows `div: [OK, passed 100 tests]`; the focused map-int-int `add` probe now
  isolates the next truthful failure; and the broader pooled check stays green
  through integer and float atomics before failing in map-valued atomic
  mutation with `ClientServererror`.
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
  creation, stability waits, native C writes, and the full-schema Hyhac
  large-object subset all succeed. The remaining work is to clear the next
  later client-visible divergence on that honest baseline: map-valued atomic
  mutation after the now-fixed `roundtrip`, `conditional`, `search`, `count`,
  integer atomic, and float atomic paths.
