# Workstream Plan: live-hyhac

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `/home/friel/.codex/skills/autoplan/references/PLANS.md`.

## Purpose / Big Picture

This workstream is the direct path to the user-visible objective: a live
`hyperdex-rs` cluster that can run `hyhac` without semantic drift. It should
consume the improved proof and harness signal from the other workstreams, then
use observed `hyhac` failures to choose the next compatibility step.

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

- The workstream benefits from the current proof and multiprocess harness fixes
  landing first so the next observed `hyhac` failure is trustworthy.

## Plan Of Work

Once the current proof and harness steps are reconciled, run the live `hyhac`
suite against `main`, record the first failing operation or return-code
mismatch, and narrow the next compatibility change to that observed surface.

## Progress

- [x] (2026-03-27 04:19Z) Created the workstream package and recorded its
  mutable surface and validator boundary.
- [ ] Run the live `hyhac` harness against the updated `main`.
- [ ] Record the next failing operation in the workstream ledger.
- [ ] Narrow the next compatibility change to that observed failure.

## Current Hypothesis

The next missing compatibility step is more likely to be an observed admin or
client behavior mismatch than a missing distributed-runtime primitive, because
the runtime and proof surface are already broad enough that the remaining gaps
should now show up as `hyhac`-visible behavior.

## Next Bounded Step

After the current proof and harness edits land, run the live `hyhac` harness
against `main`, capture the first failing operation, and preregister the
follow-up implementation step in this ledger.

## Surprises & Discoveries

- None yet in the new workstream package. The next discovery should come from a
  real `hyhac` run, not from speculation.

## Decision Log

- Decision: keep this workstream ready rather than blocked.
  Rationale: the root can launch it as soon as the current proof and harness
  edits are committed, and there is no need to wait for another file rewrite to
  make that possible.
  Date/Author: 2026-03-27 / root

## Outcomes & Retrospective

- Pending.

