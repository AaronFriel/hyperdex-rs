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
  - `worktrees/simulation-applicability`
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

### Entry `hgl-002` - Outcome

- Timestamp: `2026-03-29 20:35Z`
- Kind: `outcome`
- End commit: `a253e11`
- Artifact location:
  - `crates/simulation-harness/src/tests/mod.rs`
- Evidence summary:
  - Added `hegel_distributed_runtime_preserves_mixed_mutation_query_model`.
  - The new property generates mixed routed operations across a replicated
    two-node runtime:
    - `Put`
    - `ConditionalPut`
    - `Delete`
    - `Get`
    - threshold `Search`
    - threshold `Count`
  - After every step it checks both runtimes against one logical model, so the
    property covers routed mutation semantics plus query consistency across a
    mixed operation sequence.
  - Root validation passed with:
    - `cargo test -p simulation-harness hegel_distributed_runtime_preserves_mixed_mutation_query_model -- --nocapture`
- Conclusion: the Hegel track now has a broader mixed-operation runtime model,
  not just the delete-group/search/count property.
- Disposition: `advance`
- Next move: push Hegel into another crate boundary where the invariant belongs.

### Entry `hgl-003` - Outcome

- Timestamp: `2026-03-29 20:55Z`
- Kind: `outcome`
- End commit: `fdd1158`
- Artifact location:
  - `crates/placement-core/Cargo.toml`
  - `crates/placement-core/src/tests/mod.rs`
  - `crates/placement-core/src/tests/hegel.rs`
- Evidence summary:
  - Added
    `hegel_placement_strategies_preserve_replica_invariants_and_input_order_independence`.
  - The property proves for generated layouts and keys that both
    `RendezvousPlacement` and `HyperSpacePlacement` preserve core placement
    invariants:
    - `primary == replicas[0]`
    - replica count is clamped correctly
    - replica owners are unique
    - every replica belongs to the layout
    - partition metadata is in range
  - It also proves both strategies are independent of input node order for the
    same logical layout.
  - Root validation passed with:
    - `cargo test -p placement-core hegel_placement_strategies_preserve_replica_invariants_and_input_order_independence -- --nocapture`
- Conclusion: Hegel now exercises a reusable correctness boundary in
  `placement-core`, not only full-runtime behavior.
- Disposition: `advance`
- Next move: choose another crate-local invariant, ideally in protocol or
  storage.

### Entry `hgl-002` - Preregistration

- Timestamp: `2026-03-29 20:40Z`
- Kind: `preregister`
- Hypothesis: Hegel can cover another distributed behavior family beyond
  delete-group/search/count by mixing more replicated operations into one
  generated state model.
- Owner: next forked worker on `hegel-properties`
- Start commit: `46949a1`
- Worktree / branch:
  - `worktrees/simulation-applicability`
  - `simulation-applicability`
- Mutable surface:
  - `crates/simulation-harness/**`
  - workstream files for this track if needed
- Validator:
  - fastest useful check:
    one targeted Hegel test for the new property
  - strong checks:
    - `cargo test -p simulation-harness`
- Expected artifacts:
  - one new Hegel-backed distributed property
  - a green targeted validator
  - one bounded commit ready for reconciliation

### Entry `hgl-004` - Preregistration

- Timestamp: `2026-03-28 19:24Z`
- Kind: `preregister`
- Hypothesis: `engine-memory` is the next high-value crate-local Hegel target,
  because it can support a generated state model for conditional writes,
  deletes, `delete_matching`, search, and count without needing full runtime
  orchestration.
- Owner: forked worker on `hegel-properties`
- Start commit: `0d395b6`
- Worktree / branch:
  - `worktrees/hegel-properties-active`
  - `hegel-properties-active`
- Mutable surface:
  - `crates/engine-memory/**`
  - `crates/engine-memory/src/tests/**`
- Validator:
  - fastest useful check:
    `cargo test -p engine-memory hegel_memory_engine_preserves_conditional_and_delete_matching_model -- --nocapture`
  - strong checks:
    - `cargo test -p engine-memory`
- Expected artifacts:
  - one new Hegel-backed storage-state property
  - a green targeted validator
  - one bounded commit ready for reconciliation

### Entry `hgl-004` - Outcome

- Timestamp: `2026-03-28 19:28Z`
- Kind: `outcome`
- End commit: `9ebcf1c`
- Artifact location:
  - `crates/engine-memory/Cargo.toml`
  - `crates/engine-memory/src/tests/mod.rs`
  - `crates/engine-memory/src/tests/hegel.rs`
- Evidence summary:
  - Added
    `hegel_memory_engine_preserves_conditional_and_delete_matching_model`.
  - The property exercises generated storage operations directly against
    `MemoryEngine`:
    - `Put`
    - `ConditionalPut`
    - `Delete`
    - `DeleteMatching`
    - `Get`
    - threshold `Search`
    - threshold `Count`
  - After every step, it checks `MemoryEngine` against one explicit logical
    state model, including scalar numeric state, map-numeric state, and
    logical search/count behavior.
  - Root validation passed with:
    - `cargo test -p engine-memory hegel_memory_engine_preserves_conditional_and_delete_matching_model -- --nocapture`
    - `cargo test -p engine-memory`
- Conclusion: Hegel now covers a real crate-local storage-state invariant in
  `engine-memory`, not only runtime and placement behavior.
- Disposition: `advance`
- Next move: choose the next Hegel target in another protocol, query, or
  storage boundary.
