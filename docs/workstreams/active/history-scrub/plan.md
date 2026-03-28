# Workstream Plan: history-scrub

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules from the installed `autoplan` skill.

## Purpose / Big Picture

This workstream removes machine-specific `/home/friel` paths from the
repository, then proves the cleanup can be applied as a history rewrite
instead of only a tip-of-tree cosmetic pass.

## Goal

Land a repeatable scan-and-rewrite toolchain, fix the easy repository-local
path cases, and drive the measured `/home/friel` count in rewritten history
down to zero for the easy bucket while leaving external-local dependency paths
in an explicit later bucket.

## Acceptance Evidence

- The repository has a repeatable script that counts `/home/friel` references
  in the current tree and in rewritten history.
- The repository has a repeatable script that can rewrite history in a throwaway
  clone or worktree and report the post-rewrite count.
- New repository-authored paths in tracked files are repository-root-relative
  rather than `/home/friel/...`.
- The remaining `/home/friel` references, if any, are only in the explicitly
  deferred external-local bucket or other intentionally documented exclusions.

## Mutable Surface

- `AGENTS.md`
- `docs/autoplan.md`
- `docs/ledger.md`
- `docs/workstreams.md`
- `docs/workstreams/active/history-scrub/**`
- `scripts/**`
- tracked repository files that contain `/home/friel`, except `docs/research/**`

## Dependencies / Blockers

- `docs/research/**` is out of scope until the user reopens it.
- External-local dependency references to sibling checkouts such as HyperDex,
  BusyBee, or `hyhac` may need a later dedicated pass if they cannot be
  converted cleanly now.

## Plan Of Work

1. Build the measurement loop first:
   - count `/home/friel` references in the tracked tree
   - count them in full history
   - count them again after applying fixups in a throwaway rewritten clone
2. Separate easy repository-local paths from deferred external-local paths.
3. Convert easy repository-local paths to repository-root-relative form.
4. Use fixup commits so the cleanup can be autosquashed into history later.
5. Drive the rewritten-history count for the easy bucket down to zero.

## Progress

- [x] (2026-03-28 19:18Z) Promoted `history-scrub` to the active root board.
- [x] (2026-03-28 19:18Z) Added the repository-root-relative path rule to
  `AGENTS.md`.
- [ ] Create the dedicated `history-scrub` worktree and preregister the first
  owned implementation pass.
- [ ] Land the scan-and-rewrite tooling.
- [ ] Remove the easy repository-local `/home/friel` references.
- [ ] Report the remaining deferred external-local bucket.

## Current Hypothesis

Most of the remaining current-tree `/home/friel` paths are repository-authored
markdown links and workstream records that can be converted mechanically to
root-relative paths. The hard part is not finding them; it is making the
cleanup repeatable and safe enough to apply as a history rewrite later.

## Next Bounded Step

Create the dedicated worktree, preregister the first owned implementation
pass, and land the initial history-scrub toolchain:

- a current-tree counter
- a history counter
- a throwaway rewrite runner that autosquashes fixups and reports the
  post-rewrite count

## Surprises & Discoveries

- Observation: the easy bucket is large but mostly mechanical.
  Evidence: `git grep -n "/home/friel" -- ':(exclude)docs/research/**'` shows
  many repository-local markdown links and workstream records, plus only a
  small number of real external-local code references in
  `crates/server/tests/dist_multiprocess_harness.rs`.

## Decision Log

- Decision: keep `docs/research/**` out of scope for this workstream.
  Rationale: the user explicitly assigned that area to another agent.
  Date/Author: 2026-03-28 / root

- Decision: separate external-local dependency paths into a later bucket.
  Rationale: the user explicitly asked to handle HyperDex, BusyBee, and
  `hyhac`-style references separately if they are harder to move.
  Date/Author: 2026-03-28 / root

## Outcomes & Retrospective

- Pending.
