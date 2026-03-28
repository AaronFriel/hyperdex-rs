# Workstream Plan: warp-transactions

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `the installed `autoplan` skill fallback rules`.

## Purpose / Big Picture

This workstream adds real transaction capability in the style of HyperDex
Warp, using the now-proven distributed runtime as the substrate instead of
trying to bolt transactions onto an unproven core.

## Goal

Turn the Warp paper direction into a bounded implementation track with real
code, validators, and a staged delivery plan for `hyperdex-rs`.

## Acceptance Evidence

- The repository has a concrete design and first implementation step for
  Warp-style transactions.
- The design is tied to current coordinator, placement, and replication code.
- The first code step has focused validation.

## Mutable Surface

- `crates/server/**`
- `crates/consensus-core/**`
- `crates/transport-core/**`
- `crates/transport-grpc/**`
- new crates only if the transaction surface warrants them
- supporting docs under `docs/**`

## Dependencies / Blockers

- Depends on keeping the current live acceptance baseline green.
- Benefits from the validation and failure-testing workstreams running first.

## Plan Of Work

Start with a bounded transaction capability that fits the current runtime
architecture. Do not try to implement the full paper in one pass.

## Progress

- [x] (2026-03-28 10:00Z) Created the workstream and recorded it as a ready
  feature-expansion priority.
- [ ] Turn the paper direction into the first bounded implementation step.

## Current Hypothesis

The right first pass is likely a minimal transaction coordinator and commit
path over a restricted operation set, rather than broad multi-key semantics
everywhere at once.

## Next Bounded Step

Write the first bounded transaction design and identify the first code path to
implement on top of the current runtime.

## Surprises & Discoveries

- None yet.

## Decision Log

- Decision: keep transactions as an explicit workstream now instead of a loose
  future note.
  Rationale: this is a first-class feature request, not optional polish.
  Date/Author: 2026-03-28 / root

## Outcomes & Retrospective

- Pending.
