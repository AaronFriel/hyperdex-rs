# Workstream Ledger: nextest-fast-feedback

### Entry `ntx-001` - Preregistration

- Timestamp: `2026-03-29 18:25Z`
- Kind: `preregister`
- Hypothesis: the repository can adopt `cargo nextest` quickly if the first
  pass focuses on a practical fast suite, a clear slow-test convention, and
  `.agent/check.sh` integration instead of trying to classify every test in one
  round.
- Owner: next forked worker on `nextest-fast-feedback`
- Start commit: `2b7d144`
- Worktree / branch:
  - `worktrees/nextest-fast-feedback`
  - `nextest-fast-feedback`
- Mutable surface:
  - `.config/nextest.toml`
  - `.agent/check.sh`
  - test names, attributes, or support scripts as needed
  - manifests only if needed for nextest setup
- Validator:
  - fastest useful check:
    one bounded `cargo nextest run` fast-suite invocation
  - strong checks:
    - `.agent/check.sh`
    - the chosen slow-test filter or naming convention working as intended
- Expected artifacts:
  - a checked-in nextest configuration
  - a fast-suite contract and slow-test convention
  - `.agent/check.sh` using the fast nextest path

### Entry `ntx-001` - Outcome

- Timestamp: `2026-03-29 19:05Z`
- Kind: `outcome`
- End commit: `e76e696`
- Artifact location:
  - `.config/nextest.toml`
  - `.agent/check.sh`
  - `crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - Added `.config/nextest.toml` with the requested default profile settings.
  - `.agent/check.sh` now runs the fast nextest suite by default.
  - Adopted a `slow_` prefix convention for the heavier multiprocess tests so
    the fast suite can exclude them cleanly.
  - `cargo nextest run --workspace -E 'not test(/^slow_/)'` passes in about
    17s cold and about 10s warmed on this repository.
- Conclusion: the repository now has a practical fast-feedback nextest path.
- Disposition: `advance`
- Next move: decide whether the next step is to broaden nextest use in CI or
  keep it as the local default loop while other product work proceeds.
