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

### Entry `hsc-002` - Outcome

- Timestamp: `2026-03-28 19:44Z`
- Kind: `outcome`
- End commit: `4845f21`
- Artifact location:
  - `scripts/history-scrub/count-home-friel.sh`
  - `scripts/history-scrub/count_home_friel.py`
  - `scripts/history-scrub/make-fixups.sh`
  - `scripts/history-scrub/rewrite-and-count.sh`
  - `scripts/history-scrub/scrub-easy-paths.py`
  - `docs/workstreams/active/history-scrub/plan.md`
  - `docs/workstreams/active/history-scrub/ledger.md`
- Evidence summary:
  - Reconciled the first worker-owned pass onto `main` and reran the merged
    validators.
  - Merged-branch current-tree count is:
    - `total_refs=101`
    - `repo_local_refs=0`
    - `external_local_refs=101`
  - Merged-branch rewrite rehearsal count is:
    - `total_refs=2377`
    - `repo_local_refs=2`
    - `external_local_refs=2375`
  - The remaining current-tree references are all in the deferred
    external-local bucket.
  - The rewrite rehearsal still reports `repo_local_refs=2`, but the direct
    debug grep found zero literal surviving easy-path matches, so the mismatch
    is believed to be in the counter rather than in the rewrite result.
- Conclusion: the first `history-scrub` pass is safely merged, the easy bucket
  is gone on `main`, and the workstream can now focus on the deferred
  external-local bucket plus the small counter discrepancy.
- Disposition: `advance`
- Next move: choose one external-local sub-bucket, starting with either the
  live test-fixture paths or the archived historical references.
