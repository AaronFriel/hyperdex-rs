# Workstream Ledger: failure-testing

### Entry `flt-001` - Preregistration

- Timestamp: `2026-03-28 10:00Z`
- Kind: `preregister`
- Hypothesis: the current deterministic proof surface can support a more
  adversarial distributed failure test without a large harness rewrite, and
  that test will either harden the runtime or expose the next concrete bug.
- Owner: forked worker on `failure-testing`
- Start commit: `9104047`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/failure-testing`
  - `failure-testing`
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

### Entry `flt-001` - Outcome

- Timestamp: `2026-03-28 23:35Z`
- Kind: `outcome`
- End commit: `pending root commit`
- Artifact location:
  - `crates/server/src/lib.rs`
  - `crates/simulation-harness/src/tests/mod.rs`
- Evidence summary:
  - Added
    `turmoil_preserves_search_and_count_during_schema_convergence_gap`.
  - The new proof initially exposed a real bug: a live replica without the
    space definition could abort distributed `Search` and `Count`.
  - The server now skips a replica that is merely behind on schema state in
    the distributed read path.
- Conclusion: the first failure-oriented simulation step found and fixed a
  real distributed read correctness bug.
- Disposition: `advance`
- Next move: pick the next distributed assumption and add the next adversarial
  proof.
