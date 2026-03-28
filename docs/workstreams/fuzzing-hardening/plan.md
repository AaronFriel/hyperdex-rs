# Workstream Plan: fuzzing-hardening

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `/home/friel/.codex/skills/autoplan/references/PLANS.md`.

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
- [ ] Choose the first fuzz targets and land the initial harness.

## Current Hypothesis

The best first fuzz targets are likely the legacy protocol, admin bootstrap
framing, and request decode paths, because those areas already carried the most
compatibility risk.

## Next Bounded Step

Pick the first two or three targets and create the initial fuzz harness.

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
