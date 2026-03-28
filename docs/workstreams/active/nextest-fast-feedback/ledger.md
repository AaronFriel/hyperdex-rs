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
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/nextest-fast-feedback`
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
