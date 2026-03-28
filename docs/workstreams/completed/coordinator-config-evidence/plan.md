# Workstream Plan: coordinator-config-evidence

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `the installed `autoplan` skill fallback rules`.

## Purpose / Big Picture

This workstream exists to keep product workers from guessing when a live client
failure still depends on a precise coordinator-side contract. It is a read-only
support workstream, not a permanent parallel track.

## Goal

Produce exact source-backed coordinator or client-contract evidence only when
the active product pass cannot move honestly without it.

## Acceptance Evidence

- The workstream names one exact contract mismatch or rules out one exact
  theory with repository-local evidence.
- The result is tied to current Rust code, the original HyperDex sources, and
  an observed public failure.
- The evidence shortens the next product step instead of expanding speculation.

## Mutable Surface

- None by default. Read-only comparison is the normal mode for this workstream.
- Any helper or diagnostic code change must be preregistered separately before
  it begins.

## Dependencies / Blockers

- No current blocker.
- This workstream should stay parked unless the active product step needs exact
  source comparison again.

## Plan Of Work

Stay parked. If reactivated, reduce the question to one exact coordinator-side
or client-contract comparison, answer it from repository-local evidence, and
return control to the product workstream immediately afterward.

## Progress

- [x] (2026-03-27 20:56Z) Identified and reduced several coordinator-side and
  daemon-side contract mismatches until the active failure boundary moved later
  into the client path.
- [x] (2026-03-27 23:46Z) Contributed the final useful comparison for the prior
  phase by moving the question to the HyperDex-client handle/completion
  contract that Hyhac wraps.
- [ ] Stay parked until the active product step needs another exact source
  comparison.

## Current Hypothesis

This workstream is no longer on the critical path. The live boundary now sits
later than the coordinator bootstrap and follow path, so the next useful work
belongs in product code unless a new exact contract question appears.

## Next Bounded Step

Hold. If reactivated, answer one narrow source-backed contract question for the
active live failure and then park again.

## Surprises & Discoveries

- Observation: exact read-only comparisons were valuable when they converted
  broad protocol guessing into one concrete product fix.
  Evidence: earlier passes around bootstrap, packed config, and daemon atomic
  validation each produced direct implementation targets that later landed on
  `main`.
- Observation: once the live boundary moved beyond coordinator follow and
  bootstrap, continuing this workstream by default would have created more
  note-taking than delivery.
  Evidence: the current honest live failure appears after one successful Hyhac
  round-trip on the full-schema baseline.

## Decision Log

- Decision: park this workstream until the active product pass needs it again.
  Rationale: the current blocker is later than the coordinator path this
  workstream was narrowing, and continuing read-only comparison by default
  would add noise rather than delivery.
  Date/Author: 2026-03-28 / root

## Outcomes & Retrospective

- This workstream earned its keep by turning broad compatibility uncertainty
  into exact coordinator and daemon-side implementation targets. It should stay
  parked until the next concrete contract question appears.
