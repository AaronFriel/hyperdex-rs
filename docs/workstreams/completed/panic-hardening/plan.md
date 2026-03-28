# Workstream Plan: panic-hardening

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `the installed `autoplan` skill fallback rules`.

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
- [x] (2026-03-29 10:05Z) Hardened server startup panic paths in `db696ce`.
- [x] (2026-03-29 10:15Z) Hardened fixed-width server decode helpers in
  `ea85af6` and coordinator lock access in `e49866d`.
- [x] (2026-03-29 10:20Z) Replaced empty-layout placement panic behavior with
  checked errors in `621692b`.
- [x] (2026-03-29 18:05Z) Rejected oversized BusyBee frames earlier in
  `f9f76af` and removed the remaining placement panic fallback in `20c6d71`.
- [ ] Choose the next narrow no-panic or lint-ratchet boundary after the
  current public decoder and placement passes.

## Current Hypothesis

The broad unwrap/expect removal pass is largely complete for product code, so
the next useful step should move from raw panic removal to ratcheting:
introduce a narrow `#[no_panic]` contract where it is actually practical or
make the next Clippy panic lint enforceable on a bounded crate surface.

## Next Bounded Step

Choose one small pure boundary, likely a parser or frame decoder, and either
land a real `#[no_panic]` contract there or add a crate-level Clippy ratchet
that bans new `unwrap` and `expect` use on that surface.

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
- The startup pass showed the same theme on a larger entrypoint boundary:
  checked startup errors were straightforward, but `#[no_panic]` still failed
  on `daemon_registration_node`.
- The later passes removed most remaining product panic behavior without making
  `#[no_panic]` broadly practical yet, which means the next step should keep
  the target narrow and mechanical instead of trying to annotate a large async
  boundary.
