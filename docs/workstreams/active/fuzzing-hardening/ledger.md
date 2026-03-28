# Workstream Ledger: fuzzing-hardening

### Entry `fzh-001` - Preregistration

- Timestamp: `2026-03-29 18:20Z`
- Kind: `preregister`
- Hypothesis: the highest-value first fuzz targets are the pure compatibility
  decoders that already proved risky: BusyBee frame decode and legacy protocol
  request decode. Those targets should be isolatable enough to land an initial
  harness quickly.
- Owner: next forked worker on `fuzzing-hardening`
- Start commit: `2b7d144`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/fuzzing-hardening`
  - `fuzzing-hardening`
- Mutable surface:
  - `fuzz/**`
  - `crates/hyperdex-admin-protocol/**`
  - `crates/legacy-protocol/**`
  - workspace manifests or scripts only if needed for fuzzing setup
- Validator:
  - fastest useful check:
    build and run the first fuzz targets for a short bounded iteration
  - strong checks:
    - `cargo test -p hyperdex-admin-protocol -p legacy-protocol`
    - the new fuzz target list builds cleanly
- Expected artifacts:
  - an initial fuzz harness with at least two meaningful targets
  - a short local run path for those targets
  - one bounded commit ready for reconciliation
