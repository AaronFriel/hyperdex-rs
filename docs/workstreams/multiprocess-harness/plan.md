# Workstream Plan: multiprocess-harness

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `/home/friel/.codex/skills/autoplan/references/PLANS.md`.

## Purpose / Big Picture

This workstream keeps the real coordinator-plus-daemons validator reliable
enough that product work can trust it. It owns test and probe reduction only
when that materially improves the speed or truthfulness of the live loop.

## Goal

Keep the multiprocess harness deterministic and reactivate it only when a
smaller or more trustworthy public repro is needed for a live compatibility
fix.

## Acceptance Evidence

- `cargo test -p server --test dist_multiprocess_harness -- --nocapture`
  passes when this workstream changes harness code.
- `cargo test --workspace` passes after harness changes.
- Any new probe added here clearly shortens or strengthens the live product
  loop.

## Mutable Surface

- `Cargo.toml`
- `crates/server/Cargo.toml`
- `crates/server/tests/**`
- `crates/server/src/main.rs` only if a readiness-protocol change becomes
  necessary for harness correctness

## Dependencies / Blockers

- None at the moment.
- This workstream should not drift into product implementation that belongs in
  `live-hyhac`.

## Plan Of Work

Stay quiet unless the active product step needs a smaller truthful repro or a
more trustworthy process-level check. When reactivated, keep the scope limited
to test or probe changes that directly improve cycle time or confidence for the
active product pass.

## Progress

- [x] (2026-03-27 04:33Z) Stabilized the process-spawning multiprocess tests
  and their readiness checks on `main`.
- [x] (2026-03-27 07:45Z) Added the first short Hyhac large-object repro.
- [x] (2026-03-27 21:42Z) Proved the cleaned-baseline fast failure was still
  looping in coordinator bootstrap traffic, which was enough evidence to hand
  control back to product work.
- [x] (2026-03-28 01:02Z) The broader truthful full-schema pooled run showed
  the next real failure after the cleared large-object boundary: `roundtrip`
  fails first with `ClientReconfigure`.
- [ ] Reduce that full-schema pooled `ClientReconfigure` failure to a smaller
  truthful probe, or prove the broader full-schema pooled loop is already the
  right engineering loop.

## Current Hypothesis

The honest live baseline now exists, and this workstream is active only to test
whether the full-schema pooled `ClientReconfigure` failure can be shortened
without becoming dishonest. If it cannot produce a materially smaller truthful
repro, it should say so cleanly and park again.

## Next Bounded Step

Produce a smaller truthful repro for the first full-schema pooled
`ClientReconfigure` failure, or prove that the broader full-schema pooled loop
is already the smallest trustworthy loop worth keeping.

## Surprises & Discoveries

- Observation: harness work can easily become activity without leverage.
  Evidence: earlier retries added motion but did not move the real failure
  boundary until the probes were tied directly to the product question.
- Observation: the best harness contributions so far were the ones that made a
  public failure boundary concrete and quickly repeatable.
  Evidence: the bootstrap-progress probe and the later large-object repro both
  changed product iteration speed materially.
- Observation: the old pooled probe is no longer trustworthy after the
  large-object fix because it fails before truthful setup.
  Evidence: once the full `profiles` schema is created and stable, the first
  honest pooled failure is later and different: `roundtrip` with
  `ClientReconfigure`.

## Decision Log

- Decision: reactivate this workstream for one bounded repro-reduction pass.
  Rationale: the active product step benefits from a smaller truthful loop if
  one exists, but the harness worker must prove that value instead of assuming
  it.
  Date/Author: 2026-03-28 / root

## Outcomes & Retrospective

- This workstream improved the engineering loop when the repository needed
  faster admin/bootstrap and large-object repros. It should now stay quiet
  until a new harness change clearly helps the active product step land.
