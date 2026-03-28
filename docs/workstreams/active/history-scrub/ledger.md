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

### Entry `hsc-001` - Outcome

- Timestamp: `2026-03-28 19:31Z`
- Kind: `outcome`
- End commit: `d4d9e0d`
- Artifact location:
  - `scripts/history-scrub/count-home-friel.sh`
  - `scripts/history-scrub/count_home_friel.py`
  - `scripts/history-scrub/make-fixups.sh`
  - `scripts/history-scrub/rewrite-and-count.sh`
  - `scripts/history-scrub/scrub-easy-paths.py`
  - `docs/workstreams/active/history-scrub/plan.md`
- Evidence summary:
  - Added a repeatable current-tree counter, a history counter, an absorb
    helper, a mechanical easy-path scrubber, and a throwaway rewrite runner.
  - Baseline current-tree count was `391` total references:
    - `273` repository-local
    - `118` deferred external-local
  - Baseline branch-history count was `8520` total references:
    - `6158` repository-local
    - `2362` deferred external-local
  - After the first reduction pass, the current tree is `100` total
    references:
    - `0` repository-local
    - `100` deferred external-local
  - The throwaway rewritten branch is `2376` total references according to the
    scripted counter:
    - `2` repository-local
    - `2374` deferred external-local
  - A direct debug pass over the rewritten clone found `0` literal
    repository-local matches, so the remaining scripted `2` looks like a
    counter-accounting bug rather than a surviving current-tree path.
- Conclusion: the easy repository-local bucket is removed from the current
  tree, and the repository now has a real rewrite rehearsal toolchain instead
  of one-off grep work. The remaining work is the deferred external-local
  bucket plus one small history-counter discrepancy.
- Disposition: `advance`
- Next move: choose the next external-local sub-bucket and either remove it
  directly or document why it must wait for a dedicated later rewrite.
