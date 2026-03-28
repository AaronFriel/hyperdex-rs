# Workstream Ledger: simulation-applicability

### Entry `sap-001` - Preregistration

- Timestamp: `2026-03-29 19:20Z`
- Kind: `preregister`
- Hypothesis: the current repository uses Turmoil, Madsim, and Hegel, but not
  yet in a way that maximizes proof value across failure, recovery, and
  ordering properties.
- Owner: next forked worker on `simulation-applicability`
- Start commit: `e76e696`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/simulation-applicability`
  - `simulation-applicability`
- Mutable surface:
  - the workstream files
  - product crates only if the first selected proof target is immediately
    implemented
- Validator:
  - fastest useful check:
    a concrete proof map plus a selected next proof target
  - strong checks:
    - the selected target is specific enough to implement without another broad
      planning pass
- Expected artifacts:
  - a proof-tool applicability map
  - a short ordered list of next proof targets
  - one selected target ready for implementation
