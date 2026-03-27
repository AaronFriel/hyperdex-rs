# Workstream Ledger: coordinator-config-evidence

### Entry `cce-001` - Preregistration

- Timestamp: `2026-03-27 20:04Z`
- Kind: `preregister`
- Hypothesis: a read-only pass over the harness capture, the original
  HyperDex/Replicant sources, and the current Rust packed-config path can
  identify the exact coordinator-side contract mismatch that prevents the
  focused large-object path from reaching `REQ_ATOMIC`.
- Owner: next delegated read-only worker
- Start commit: `8871797`
- Worktree / branch:
  - none required for the first bounded step
- Mutable surface:
  - none; read-only evidence gathering only
- Validator:
  - exact source-backed explanation of the captured coordinator frame pair
  - concrete statement of the missing packed config or schema contract the
    original client expects before it prepares the first atomic write
- Expected artifacts:
  - a decoded or source-mapped interpretation of the `trailing_bytes=45` and
    `trailing_bytes=100` partial BusyBee-style frames
  - a concrete coordinator-side contract target for the product worker
