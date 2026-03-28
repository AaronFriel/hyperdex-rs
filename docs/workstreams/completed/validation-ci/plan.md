# Workstream Plan: validation-ci

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `the installed `autoplan` skill fallback rules`.

## Purpose / Big Picture

This workstream turns the current local validation discipline into repository
automation. Its job is to make it hard to merge regressions by giving the Rust
rewrite the same kind of GitHub Actions coverage already used in stronger Rust
repositories nearby.

## Goal

Land a first GitHub Actions set that validates formatting, linting, workspace
tests, and the reusable live acceptance verifier shape.

## Acceptance Evidence

- `.github/workflows/**` exists on `main`.
- Workflow files pass `actionlint`.
- Workflow files pass local `act` runs for the implemented jobs.
- The workflow set covers at least formatting, clippy, and workspace tests.
- The workflow design includes a bounded way to run the reusable live
  acceptance verifier or an equivalent non-interactive acceptance command.

## Mutable Surface

- `.github/workflows/**`
- `.github/actions/**` if a local reusable action becomes justified
- `scripts/verify-live-acceptance.sh`
- `Cargo.toml` only if a CI-specific helper command or alias becomes justified

## Dependencies / Blockers

- None.

## Plan Of Work

Start with the proven patterns from nearby Rust repositories, but keep the
workflow set specific to `hyperdex-rs`. The first pass should land the core
checks, keep them non-flaky, and avoid pretending the CI story is complete if
the live acceptance path still needs a practical compromise.

## Progress

- [x] (2026-03-28 10:00Z) Created the workstream and made it an active root
  priority for the post-compatibility phase.
- [x] (2026-03-28 18:15Z) Bound this workstream to the dedicated
  `worktrees/validation-ci` checkout and made local `act` execution part of
  the validator.
- [x] (2026-03-28 23:55Z) Landed the first workflow set on `main` in
  `54c406f` and validated it with `actionlint`, `act`, and the repository
  helper scripts.

## Current Hypothesis

The first phase is complete. The next CI work, if promoted later, should be
about widening coverage honestly rather than bootstrapping the basics.

## Next Bounded Step

None in this phase. Promote a follow-up only when the current limited clippy
scope or broader acceptance coverage becomes the next highest-value gap.

## Surprises & Discoveries

- Observation: the repository currently has no `.github/workflows` directory.
  Evidence: `find .github -maxdepth 3 -type f` returned `No such file or
  directory` on root.

## Decision Log

- Decision: make CI one of the first active workstreams after the Hyhac
  baseline turned green.
  Rationale: the next phase should tighten the merge loop before it adds larger
  distributed features.
  Date/Author: 2026-03-28 / root

## Outcomes & Retrospective

- `54c406f` added act-backed GitHub Actions workflows plus CI helper scripts.
- The workflow set is explicit about its current limits instead of pretending
  to lint or test more than it really can under `act`.
