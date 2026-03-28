# Workstream Ledger: validation-ci

### Entry `vci-001` - Preregistration

- Timestamp: `2026-03-28 10:00Z`
- Kind: `preregister`
- Hypothesis: a first GitHub Actions set modeled on the stronger nearby Rust
  repositories can land quickly and raise the validation floor without needing
  a large supporting framework first.
- Owner: next forked worker
- Owner: forked worker on `validation-ci`
- Start commit: `9104047`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/validation-ci`
  - `validation-ci`
- Mutable surface:
  - `.github/workflows/**`
  - `scripts/verify-live-acceptance.sh`
  - repository-local CI helper files if needed
- Validator:
  - fastest useful check:
    `act --list`
  - strong checks:
    - `actionlint`
    - local `act` execution for the implemented jobs
    - `cargo fmt --all -- --check`
    - `cargo clippy --workspace --all-targets -- -D warnings`
    - `cargo test --workspace`
- Expected artifacts:
  - first workflow set on `main`
  - actionlint-clean YAML
  - one bounded commit ready for reconciliation

### Entry `vci-001` - Outcome

- Timestamp: `2026-03-28 23:55Z`
- Kind: `outcome`
- End commit: `54c406f`
- Artifact location:
  - `.actrc`
  - `.github/workflows/actionlint.yml`
  - `.github/workflows/lint.yml`
  - `.github/workflows/test.yml`
  - `.github/workflows/acceptance.yml`
  - `scripts/check-clippy.sh`
  - `scripts/check-workspace-tests.sh`
  - `scripts/verify-live-acceptance.sh`
- Evidence summary:
  - `actionlint` passes on the workflow set.
  - `act --list` sees the expected jobs.
  - Root validation passed
    `act -W .github/workflows/acceptance.yml -j quick-live-acceptance --pull=false`.
  - The branch validation also passed the `format`, `clippy`, and `workspace`
    jobs under `act`.
- Conclusion: the repository now has a truthful first CI layer backed by local
  `act` proof.
- Disposition: `advance`
- Next move: revisit CI only when it is time to widen the clippy scope or add
  more acceptance coverage.
