# Workstream Plan: simulation-applicability

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `/home/friel/.codex/skills/autoplan/references/PLANS.md`.

## Purpose / Big Picture

This workstream exists to get more real proof value out of Turmoil, Madsim,
and Hegel instead of using them only for a narrow set of point checks. Its job
is to identify where each tool best fits this codebase and to turn that into a
concrete queue of stronger proofs.

## Goal

Produce a concrete simulation/proof map for the repository and land the first
new proof target that follows from that map.

## Acceptance Evidence

- The repository has an explicit mapping from proof tool to applicable system
  property.
- At least one new high-value proof target is selected from that mapping.
- The selected target is concrete enough to hand to an implementation worker.

## Mutable Surface

- `docs/**` for the workstream files only
- product crates only if the first proof target is immediately implemented

## Dependencies / Blockers

- None.

## Plan Of Work

Inventory the current Turmoil, Madsim, and Hegel use, identify under-covered
distributed properties, and translate that into a short ordered list of new
proof targets that are worth implementing.

## Progress

- [x] (2026-03-29 19:20Z) Created the workstream and promoted it into the
  active board.
- [ ] Build the first explicit proof map and select the next implementation
  target.

## Current Hypothesis

The repository has more coverage for transport failure and degraded reads than
for recovery ordering, reconfiguration sequencing, or operation ordering after
node return. That is likely where the next proof value sits.

## Next Bounded Step

Map Turmoil, Madsim, and Hegel to concrete property families in this repo and
pick the first new proof target from that map.

## Surprises & Discoveries

- None yet.

## Decision Log

- Decision: separate proof-tool applicability from individual bug hunts.
  Rationale: it is easier to get leverage from these tools when their roles are
  made explicit instead of rediscovered each iteration.
  Date/Author: 2026-03-29 / root

## Outcomes & Retrospective

- Pending.
