# Workstream Ledger: recovery-ordering

### Entry `rco-001` - Preregistration

- Timestamp: `2026-03-29 19:20Z`
- Kind: `preregister`
- Hypothesis: the parked ownership-convergence patch and its proof are a good
  seed for a broader recovery-ordering effort, because they ask whether a
  stale node can still accept mutations after another node has the newer view.
- Owner: next forked worker on `recovery-ordering`
- Start commit: `e76e696`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/recovery-ordering`
  - `recovery-ordering`
- Mutable surface:
  - `crates/simulation-harness/**`
  - `crates/server/**`
  - `crates/transport-core/**`
- Validator:
  - fastest useful check:
    one deterministic recovery-ordering test
  - strong checks:
    - `cargo test -p simulation-harness`
    - `cargo test -p server`
- Expected artifacts:
  - one recovery-ordering proof
  - a runtime fix if the proof exposes a bug
  - one bounded commit ready for reconciliation
