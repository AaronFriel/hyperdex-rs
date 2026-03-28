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
improves engineering cycle time, and drive remaining compatibility or proof
gaps through real changes in `crates/**` or a directly relevant live
verification harness. The current baseline is broader now: the live Hyhac
surface is green on both a single-daemon cluster and a real two-daemon
cluster, and a reusable verifier exists for the single-daemon acceptance path.
The next pass should broaden public distributed or operability proof from that
green baseline without regressing it.

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
- [x] (2026-03-28 03:10Z) Landed the remaining map-valued atomic compatibility
  work, turned the stale map-failure probes into green checks, and added a
  split live acceptance proof that runs Hyhac admin add/remove on a fresh
  cluster, pooled/shared data checks on a full-schema cluster, and CBString
  directly.
- [x] (2026-03-28 03:27Z) Extended the live public proof to a real two-daemon
  cluster and added a reusable `scripts/verify-live-acceptance.sh` entrypoint
  for the current green single-daemon acceptance path.
- [ ] Decide the next broader public distributed or operability proof from the
  new green live baseline.

## Current Hypothesis

There is no current live mismatch on the Hyhac-facing public surface that is
already covered: the suite is green on a split single-daemon path, and the
same surface is also green on a real two-daemon cluster. The next useful gap is
not a known compatibility failure but the next broader public distributed or
operability proof worth adding.

## Next Bounded Step

Keep the split live acceptance check and the reusable verifier green. Use a
supporting harness or script only if it broadens public distributed proof or
materially shortens the next live acceptance loop without hiding setup
requirements. The next bounded step is to choose and implement the next broader
public distributed or operability proof from this green baseline.

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
- Observation: the old map-atomic failure boundary is cleared on integrated
  `main`.
  Evidence: `77885e7` and `8a33f55` are on `main`; the focused pooled
  `map-int-int add`, `map-int-string prepend`, and `map-string-string prepend`
  probes now all pass; and the broader pooled live check is fully green.
- Observation: the live Hyhac suite can now be proven green on a real cluster
  without modifying Hyhac itself, but it must be exercised in split phases
  that respect the suite’s own setup assumptions.
  Evidence: `legacy_hyhac_split_acceptance_suite_passes_live_cluster` passes on
  `main`, proving `Can add a space`, `Can remove a space`, `*pooled*`,
  `*shared*`, and `*CBString*` on live Rust-backed clusters.
- Observation: the same Hyhac-facing surface is also green on a real
  two-daemon cluster.
  Evidence: `legacy_hyhac_split_acceptance_suite_passes_two_daemon_live_cluster`
  passes on `main`.
- Observation: the current live proof is no longer trapped inside cargo test
  filters.
  Evidence: `scripts/verify-live-acceptance.sh --quick` passes on `main` and
  runs the current green acceptance path directly.

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
