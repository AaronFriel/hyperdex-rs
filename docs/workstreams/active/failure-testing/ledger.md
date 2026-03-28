# Workstream Ledger: failure-testing

### Entry `flt-001` - Preregistration

- Timestamp: `2026-03-28 10:00Z`
- Kind: `preregister`
- Hypothesis: the current deterministic proof surface can support a more
  adversarial distributed failure test without a large harness rewrite, and
  that test will either harden the runtime or expose the next concrete bug.
- Owner: next forked worker
- Start commit: `HEAD`
- Worktree / branch:
  - worktree to be created from current `main`
- Mutable surface:
  - `crates/simulation-harness/**`
  - `crates/server/**` only if the proof finds a runtime bug
- Validator:
  - fastest useful check:
    `cargo test -p simulation-harness turmoil_preserves_degraded_read_correctness_after_one_node_loss -- --nocapture`
  - strong checks:
    - `cargo test -p simulation-harness`
    - `cargo test --workspace`
- Expected artifacts:
  - one new deterministic failure-oriented proof
  - a green workspace or a concrete runtime bug with evidence
  - one bounded commit ready for reconciliation
