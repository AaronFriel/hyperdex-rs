# Workstream Ledger: validation-ci

### Entry `vci-001` - Preregistration

- Timestamp: `2026-03-28 10:00Z`
- Kind: `preregister`
- Hypothesis: a first GitHub Actions set modeled on the stronger nearby Rust
  repositories can land quickly and raise the validation floor without needing
  a large supporting framework first.
- Owner: next forked worker
- Start commit: `HEAD`
- Worktree / branch:
  - worktree to be created from current `main`
- Mutable surface:
  - `.github/workflows/**`
  - `scripts/verify-live-acceptance.sh`
- Validator:
  - fastest useful check:
    `find .github/workflows -maxdepth 1 -type f | sort`
  - strong checks:
    - `actionlint`
    - `cargo fmt --all -- --check`
    - `cargo clippy --workspace --all-targets -- -D warnings`
    - `cargo test --workspace`
- Expected artifacts:
  - first workflow set on `main`
  - actionlint-clean YAML
  - one bounded commit ready for reconciliation
