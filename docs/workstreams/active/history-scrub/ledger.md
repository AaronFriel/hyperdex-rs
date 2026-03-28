# Workstream Ledger: history-scrub

### Entry `hsc-001` - Preregistration

- Timestamp: `2026-03-28 19:18Z`
- Kind: `preregister`
- Hypothesis: the home-directory path cleanup can be driven with a repeatable metric
  instead of one-off grep work by combining fixup commits, an autosquash
  rewrite runner, and current-tree/history counters.
- Owner: dedicated worker on `history-scrub`
- Start commit: `06d7338`
- Worktree / branch:
  - `worktrees/history-scrub`
  - `history-scrub`
- Mutable surface:
  - `AGENTS.md`
  - `docs/autoplan.md`
  - `docs/ledger.md`
  - `docs/workstreams.md`
  - `docs/workstreams/active/history-scrub/**`
  - `scripts/**`
  - tracked repository files that contain the machine-specific home-directory
    prefix, except
    `docs/research/**`
- Validator:
  - fastest useful check:
    `scripts/history-scrub/count-home-friel.sh --tree`
  - strong checks:
    - `scripts/history-scrub/count-home-friel.sh --history`
    - `scripts/history-scrub/rewrite-and-count.sh`
- Expected artifacts:
  - a repeatable current-tree and history counter
  - a throwaway rewrite runner with autosquash support
  - the first reduction pass over easy repository-local home-directory
    references
  - an explicit remaining external-local bucket, if any
