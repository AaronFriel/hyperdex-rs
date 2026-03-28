# Workstream Plan: fuzzing-hardening

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `the installed `autoplan` skill fallback rules`.

## Purpose / Big Picture

This workstream hardens the rewrite against malformed input by adding fuzz
targets for the highest-risk compatibility and parser surfaces.

## Goal

Add focused fuzz targets for critical parsers, frame decoders, or request
handlers and make them practical to run locally and in CI.

## Acceptance Evidence

- The repository has an explicit fuzz target set for at least one critical
  compatibility surface.
- The fuzz entrypoints are documented and runnable.
- The work fits cleanly into the repository’s validation story.

## Mutable Surface

- `fuzz/**`
- protocol and parser crates selected by the first target
- `.github/workflows/**` only if a fuzz workflow becomes part of the first pass

## Dependencies / Blockers

- Best started after the first CI workflow lands, but not blocked on all
  other workstreams completing.

## Plan Of Work

Choose the first targets by risk and by how easy they are to isolate. Favor
legacy framing, admin/client protocol decoding, and API handler boundaries over
lower-value internal helpers.

## Progress

- [x] (2026-03-28 10:00Z) Created the workstream and recorded it as a ready
  priority for the next phase.
- [x] (2026-03-29 18:10Z) Promoted this workstream into the active board after
  the current async-modernization round completed.
- [ ] Choose the first fuzz targets and land the initial harness.

## Current Hypothesis

The best first fuzz targets are the pure decoder boundaries that already proved
fragile or high-value in compatibility work: BusyBee frame decode,
legacy-protocol request decode, and packed admin-space decoding.

## Next Bounded Step

Create the first fuzz harness for BusyBee frame decode and one legacy request
decode path, and keep the entrypoints cheap enough to run locally before
expanding the corpus.

## Surprises & Discoveries

- None yet.

## Decision Log

- Decision: keep fuzzing in the next priority group instead of treating it as
  a later polish task.
  Rationale: protocol compatibility code benefits disproportionately from
  malformed-input testing.
  Date/Author: 2026-03-28 / root

## Outcomes & Retrospective

- Pending.
