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
  - `worktrees/fuzzing-hardening`
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

### Entry `fzh-001` - Outcome

- Timestamp: `2026-03-29 19:05Z`
- Kind: `outcome`
- End commit: `b719b65`
- Artifact location:
  - `fuzz/**`
  - `crates/legacy-protocol/src/lib.rs`
  - `crates/legacy-protocol/src/tests/mod.rs`
- Evidence summary:
  - Added repo-local `cargo-fuzz` setup with `busybee_frame_decode` and
    `legacy_request_decode`.
  - The first fuzz pass found and fixed a real issue in
    `legacy-protocol`: impossible varint counts could drive oversized
    `Vec::with_capacity(...)` allocation.
  - Added regression coverage for the crashing fuzz input.
- Conclusion: the repository now has a real initial fuzz harness, and the
  first pass already paid off with a decoder hardening fix.
- Disposition: `advance`
- Next move: expand fuzz targets beyond pure decoder entrypoints and decide
  whether the next target should be another protocol parser or a request
  handler boundary.
