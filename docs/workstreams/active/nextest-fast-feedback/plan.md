# Workstream Plan: nextest-fast-feedback

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `/home/friel/.codex/skills/autoplan/references/PLANS.md`.

## Purpose / Big Picture

This workstream shortens the feedback loop by moving the repository toward a
reliable `cargo nextest` fast suite. Its job is to define the fast-path test
contract, separate slower tests cleanly, and make `.agent/check.sh` use the
fast path by default.

## Goal

Land the first `cargo nextest` setup with a fast suite that reliably finishes
in about 30 seconds, plus a naming or filtering convention for slower tests.

## Acceptance Evidence

- The repository has `.config/nextest.toml` with the default timeout behavior.
- `.agent/check.sh` uses a fast nextest check.
- The repository has a clear convention for slower tests so the fast suite can
  exclude them.
- The nextest fast path is runnable locally and returns useful signal quickly.

## Mutable Surface

- `.config/nextest.toml`
- `.agent/check.sh`
- test names, attributes, or support scripts as needed
- crate manifests only if required for nextest integration

## Dependencies / Blockers

- None.

## Plan Of Work

Start by defining the fast suite contract, not by trying to nextest every
single long-running integration path on day one. Keep the first pass honest:
fast suite by default, slower tests named or grouped clearly, and a short path
to run both.

## Progress

- [x] (2026-03-29 18:25Z) Created the workstream and promoted it into the
  active board.
- [ ] Land the first nextest fast-feedback path.

## Current Hypothesis

The best first move is to adopt a simple slow-test convention on the heaviest
integration paths, then use nextest filters so `.agent/check.sh` gets a fast
default without hiding the slower suite.

## Next Bounded Step

Add `.config/nextest.toml`, define the slow-test convention, wire the fast
path into `.agent/check.sh`, and prove that the default fast run finishes
within roughly 30 seconds on this repository.

## Surprises & Discoveries

- None yet.

## Decision Log

- Decision: treat fast feedback as an active product concern, not just tooling
  polish.
  Rationale: this repository now has enough distributed and Hyhac-facing proof
  that day-to-day iteration needs a faster default path.
  Date/Author: 2026-03-29 / root

## Outcomes & Retrospective

- Pending.
