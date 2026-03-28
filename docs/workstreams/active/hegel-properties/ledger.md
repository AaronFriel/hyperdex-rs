# Workstream Ledger: hegel-properties

### Entry `hgl-001` - Preregistration

- Timestamp: `2026-03-29 19:20Z`
- Kind: `preregister`
- Hypothesis: the current repository uses Turmoil, Madsim, and Hegel, but not
  yet in a way that maximizes proof value across failure, recovery, and
  ordering properties.
- Owner: next forked worker on `hegel-properties`
- Start commit: `e76e696`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/simulation-applicability`
  - `simulation-applicability`
- Mutable surface:
  - `crates/simulation-harness/**`
  - the workstream files
- Validator:
  - fastest useful check:
    `cargo test -p simulation-harness hegel_distributed_runtime_preserves_logical_delete_group_search_and_count -- --nocapture`
  - strong checks:
    - `cargo test -p simulation-harness`
- Expected artifacts:
  - one new Hegel-backed distributed property
  - a green targeted validator
  - one bounded commit ready for reconciliation

### Entry `hgl-001` - Outcome

- Timestamp: `2026-03-29 20:05Z`
- Kind: `outcome`
- End commit: `c295710`
- Artifact location:
  - `crates/simulation-harness/src/tests/mod.rs`
- Evidence summary:
  - Added
    `hegel_distributed_runtime_preserves_logical_delete_group_search_and_count`.
  - The new property generates distributed routed `Put`, `DeleteGroup`, and
    `Get` sequences and checks after every step that `Search` and `Count`
    remain logically deduplicated from both runtimes.
  - `cargo test -p simulation-harness hegel_distributed_runtime_preserves_logical_delete_group_search_and_count -- --nocapture`
    passed.
- Conclusion: Hegel now has a real distributed logical-state proof role in the
  repository instead of sitting mostly unused.
- Disposition: `advance`
- Next move: add another Hegel property on a distinct distributed behavior
  family.
