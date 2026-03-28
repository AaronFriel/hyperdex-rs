# Workstream Plan: history-scrub

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules from the installed `autoplan` skill.

## Purpose / Big Picture

This workstream removes machine-specific home-directory paths from the
repository, then proves the cleanup can be applied as a history rewrite
instead of only a tip-of-tree cosmetic pass.

## Goal

Land a repeatable scan-and-rewrite toolchain, fix the easy repository-local
path cases, and drive the measured home-directory path count in rewritten history
down to zero for the easy bucket while leaving external-local dependency paths
in an explicit later bucket.

## Acceptance Evidence

- The repository has a repeatable script that counts machine-specific
  home-directory references
  in the current tree and in rewritten history.
- The repository has a repeatable script that can rewrite history in a throwaway
  clone or worktree and report the post-rewrite count.
- New repository-authored paths in tracked files are repository-root-relative
  rather than machine-specific home-directory paths.
- The remaining home-directory references, if any, are only in the explicitly
  deferred external-local bucket or other intentionally documented exclusions.

## Mutable Surface

- `AGENTS.md`
- `docs/autoplan.md`
- `docs/ledger.md`
- `docs/workstreams.md`
- `docs/workstreams/active/history-scrub/**`
- `scripts/**`
- tracked repository files that contain the machine-specific home-directory
  prefix, except `docs/research/**`

## Dependencies / Blockers

- `docs/research/**` is out of scope until the user reopens it.
- External-local dependency references to sibling checkouts such as HyperDex,
  BusyBee, or `hyhac` may need a later dedicated pass if they cannot be
  converted cleanly now.

## Plan Of Work

1. Build the measurement loop first:
   - count machine-specific home-directory references in the tracked tree
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
- [x] (2026-03-28 19:23Z) Created the dedicated `history-scrub` worktree and
  used it for the first owned implementation pass.
- [x] (2026-03-28 19:26Z) Landed the scan-and-rewrite tooling under
  `scripts/history-scrub/`.
- [x] (2026-03-28 19:25Z) Removed the easy repository-local home-directory
  references from the current tree.
- [x] (2026-03-28 19:31Z) Reported the deferred external-local bucket and the
  rewritten-history reduction.

## Current Hypothesis

The easy repository-local bucket is now gone from the current tree. The
remaining work is no longer broad search-and-replace; it is a narrower pass
over the deferred external-local bucket, especially HyperDex and `hyhac`
sibling-repo references in historical ledgers and the two live test-fixture
paths in `crates/server/tests/dist_multiprocess_harness.rs`.

## Next Bounded Step

Pick one external-local sub-bucket and remove it cleanly:

- either convert the two live test-fixture paths in
  `crates/server/tests/dist_multiprocess_harness.rs` to environment-driven or
  repository-relative discovery
- or reduce the historical HyperDex and `hyhac` command/source references in
  archived ledgers without damaging their evidence value

## Surprises & Discoveries

- Observation: the easy bucket is large but mostly mechanical.
  Evidence: `scripts/history-scrub/count-home-friel.sh --tree` and the
  matching `git grep` output show many repository-local markdown links and
  workstream records, plus only a small number of real external-local code references in
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

- The first pass delivered real leverage instead of another grep-only note:
  - `scripts/history-scrub/count-home-friel.sh` measures the current tree and
    history
  - `scripts/history-scrub/make-fixups.sh` creates absorb-based fixups
  - `scripts/history-scrub/rewrite-and-count.sh` rehearses a throwaway
    autosquash plus history rewrite
  - `scripts/history-scrub/scrub-easy-paths.py` performs the mechanical easy
    cleanup
- The current-tree easy bucket dropped from `273` repo-local references to `0`.
- The remaining current-tree references are all in the deferred external-local
  bucket.
