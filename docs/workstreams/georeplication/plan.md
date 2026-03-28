# Workstream Plan: georeplication

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `/home/friel/.codex/skills/autoplan/references/PLANS.md`.

## Purpose / Big Picture

This workstream adds region-aware replication and placement in the style of
Consus. Its job is to make the Rust rewrite span failure domains larger than a
single cluster without abandoning the current HyperDex-compatible model.

## Goal

Define and land the first bounded implementation step for region-aware
placement and replication.

## Acceptance Evidence

- The repository has a concrete design and first implementation step for
  region-aware placement or replication.
- The design uses current placement, control-plane, and replication surfaces
  instead of hand-waving a future rewrite.
- The first code step has focused validation.

## Mutable Surface

- `crates/cluster-config/**`
- `crates/placement-core/**`
- `crates/control-plane/**`
- `crates/server/**`
- `crates/transport-core/**`
- supporting docs under `docs/**`

## Dependencies / Blockers

- Depends on the existing runtime staying green.
- Benefits from stronger validation and failure-testing loops first.

## Plan Of Work

Start by making region and cluster grouping explicit in current configuration
and placement terms, then land a first bounded implementation that uses those
new concepts.

## Progress

- [x] (2026-03-28 10:00Z) Created the workstream and recorded it as a ready
  feature-expansion priority.
- [ ] Turn the region-aware replication direction into the first bounded
  implementation step.

## Current Hypothesis

The right first pass is probably region-aware configuration and placement
metadata, not a full geo-transaction system in one move.

## Next Bounded Step

Write the first bounded design for region-aware placement and identify the
first code path to implement.

## Surprises & Discoveries

- None yet.

## Decision Log

- Decision: make georeplication an explicit workstream now instead of a vague
  note attached to transactions.
  Rationale: the user asked for Consus-style direction specifically, and the
  work needs its own owned surface and validation story.
  Date/Author: 2026-03-28 / root

## Outcomes & Retrospective

- Pending.
