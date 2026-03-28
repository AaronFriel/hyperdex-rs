# Workstream Plan: panic-hardening

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `/home/friel/.codex/skills/autoplan/references/PLANS.md`.

## Purpose / Big Picture

This workstream reduces uncontrolled panic paths in the runtime and public
protocol surface. Its job is to replace casual `unwrap` and `expect` usage
with explicit error handling where it matters, add no-panic contracts where
they are practical, and tighten linting so the repository can ratchet toward
safer defaults.

## Goal

Land the first bounded no-panic and unwrap-reduction pass over entry points or
important public functions, with a validator strong enough to keep the change
honest.

## Acceptance Evidence

- One meaningful runtime or protocol surface removes unchecked panic sites.
- The first pass introduces `#[no_panic]` where the contract is practical.
- The first pass documents or lands the next Clippy ratchet for `unwrap`,
  `expect`, `todo`, or related panic paths.
- The repository stays green after the change.

## Mutable Surface

- `crates/server/**`
- `crates/legacy-frontend/**`
- `crates/legacy-protocol/**`
- `crates/hyperdex-admin-protocol/**`
- `crates/hyperdex-client-protocol/**`
- crate manifests and lint configuration as needed

## Dependencies / Blockers

- None.

## Plan Of Work

Start with entry points and important public runtime functions, not with a
repo-wide panic purge. Prefer one coherent surface that can move from casual
panic behavior to explicit contracts and then set the next ratchet.

## Progress

- [x] (2026-03-28 10:40Z) Created the workstream and promoted it into the
  active root priority set.
- [x] (2026-03-28 18:15Z) Bound this workstream to the dedicated
  `worktrees/panic-hardening` checkout for one owned fork.
- [x] (2026-03-28 23:55Z) Landed decoder hardening in `legacy-protocol` and
  `hyperdex-admin-protocol` in `694545e` and `44f8c58`.
- [x] (2026-03-29 00:53Z) Hardened legacy frontend identify decoding in
  `dd00c13`.
- [ ] Choose the next public/runtime boundary after the legacy frontend pass.

## Current Hypothesis

After the legacy frontend pass, the next target should be a different
public/runtime boundary with more than one remaining panic site. The single
fixed-width identify decode panic in `legacy-frontend` is now gone.

## Next Bounded Step

Choose the next public/runtime boundary after `legacy-frontend`, remove the
most meaningful unchecked panic sites there, and carry forward the concrete
no-panic evidence from this pass instead of retrying the same annotation
blindly.

## Surprises & Discoveries

- Observation: unchecked panic paths are still widespread in current runtime
  and protocol code.
  Evidence: `rg -n "unwrap\\(|expect\\(|todo!|panic!|no_panic"` on the current
  tree reports many hits across `server`, `legacy-frontend`,
  `legacy-protocol`, `hyperdex-admin-protocol`, and related test surfaces.

## Decision Log

- Decision: treat panic hardening as active foundation work instead of a later
  cleanup item.
  Rationale: panic behavior at public or runtime boundaries is part of the
  correctness contract, not just code style.
  Date/Author: 2026-03-28 / root

## Outcomes & Retrospective

- Two meaningful decoder boundaries are already hardened.
- In both decoder passes, `#[no_panic]` proved harder than the raw
  unwrap/expect removal itself, which suggests the next step should keep that
  contract narrow and evidence-driven.
- The same pattern held on the legacy frontend identify helper: checked
  decoding was straightforward, but a narrow `#[no_panic]` attempt still
  failed at link time.
