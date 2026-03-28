# Workstream Plan: validation-ci

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `/home/friel/.codex/skills/autoplan/references/PLANS.md`.

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
- [ ] Land the first workflow set and verify it locally with `actionlint` plus
  local `act` plus the equivalent cargo commands.

## Current Hypothesis

The repository is missing basic CI structure, not suffering from an obscure CI
bug. A first bounded workflow set should land quickly and immediately improve
merge discipline.

## Next Bounded Step

Install or bootstrap `act` if needed, create `.github/workflows` with
formatting, clippy, workspace tests, and a bounded acceptance path derived
from `scripts/verify-live-acceptance.sh`, then prove the implemented jobs
locally with `act`.

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

- Pending.
