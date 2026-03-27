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

### Entry `cce-001` - Outcome

- Timestamp: `2026-03-27 20:43Z`
- Kind: `outcome`
- End commit: `d8ac0ad`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/docs/workstreams/coordinator-config-evidence/plan.md`
- Evidence summary:
  - The captured client-side `trailing_bytes=45` stream matches a BusyBee
    `IDENTIFY` frame (`0x80000014`, 20 bytes) plus a Replicant bootstrap
    request frame (`0x00000005 0x1c`, 5 bytes), with another 20-byte BusyBee
    identify frame worth of bytes still in the stream. This matches the Rust
    captured bootstrap request constant and the original Replicant
    `start_bootstrap` request size.
  - The captured server-side `trailing_bytes=100` stream starts with the same
    20-byte BusyBee `IDENTIFY` frame shape, followed by a normal 60-byte
    Replicant bootstrap response frame (`0x0000003c`) and another 20-byte
    BusyBee identify frame worth of bytes. The 60-byte middle frame size is the
    exact size of the Rust `ReplicantBootstrapResponse` for one bootstrap
    server and also matches the original Replicant bootstrap decode path.
  - The original HyperDex client does not build `REQ_ATOMIC` until
    `client::maintain_coord_connection` has completed
    `replicant_client_cond_follow("hyperdex", "config", ...)` and unpacked the
    returned bytes as `hyperdex::configuration`.
- Conclusion:
  - The focused large-object repro is not blocked on the first BusyBee or
    Replicant bootstrap exchange. That capture is healthy bootstrap traffic.
  - The next coordinator-side contract that matters is the packed
    `hyperdex::configuration` body returned by the `hyperdex/config` follow
    reply. That payload must satisfy the original HyperDex unpackers for
    `configuration`, `space`, `subspace`, `region`, `replica`, and the full
    container-heavy `profiles` schema before the client can call `get_schema`,
    `point_leader`, `prepare_funcs`, and finally send `REQ_ATOMIC`.
- Disposition: `advance`
- Next move:
  - Keep the product worker focused on the `hyperdex/config` follow payload,
    not bootstrap.
  - Compare the Rust `default_legacy_config_encoder` output against the
    original HyperDex `configuration` / `space` packing rules on a live
    `profiles` config body.

### Entry `cce-002` - Preregistration

- Timestamp: `2026-03-27 20:14Z`
- Kind: `preregister`
- Hypothesis: a second read-only pass that compares the Rust
  `default_legacy_config_encoder` output against the original HyperDex
  `configuration` / `space` packing rules on a live `profiles` config body
  will identify the first concrete mismatch, if any, in the `hyperdex/config`
  follow payload that the client consumes before it prepares the first atomic
  write.
- Owner: next delegated read-only worker
- Start commit: `d8ac0ad`
- Worktree / branch:
  - none required for the bounded step
- Mutable surface:
  - none; read-only evidence gathering only
- Validator:
  - source-backed comparison between Rust packed-config bytes and the original
    HyperDex `configuration` / `space` packing rules
  - a concrete mismatch if one exists, or a concrete statement that the next
    contract boundary lies later in the same follow path
- Expected artifacts:
  - exact comparison of Rust `default_legacy_config_encoder` output versus the
    original HyperDex packing contract for a live `profiles` config body
  - a tighter product target for `hyh-034`

### Entry `cce-002` - Outcome

- Timestamp: `2026-03-27 20:20Z`
- Kind: `outcome`
- End commit: `9e108b8`
- Artifact location:
  - read-only comparison across the current Rust encoder and the original
    HyperDex `configuration` / `space` packing sources
- Evidence summary:
  - the live `profiles` schema is the container-heavy definition in
    `/home/friel/c/aaronfriel/hyhac/test/Test/HyperDex/Space.hs`
  - the original HyperDex builder inserts the primary subspace on attribute `0`
    and fills each region with `hyperdex::partition(...)` from
    `/home/friel/c/aaronfriel/HyperDex/admin/hyperspace_builder.cc` and
    `/home/friel/c/aaronfriel/HyperDex/admin/partition.cc`
  - for `64` partitions on a 1-attribute primary subspace, the first primary
    region is `lower=0`, `upper=0x03ffffffffffffff`, the second begins at
    `0x0400000000000000`, and the last ends at `UINT64_MAX`
  - Rust’s `default_legacy_config_encoder` currently emits
    `lower=partition`, `upper=partition` for every region in
    `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/lib.rs`
  - the original HyperDex client consumes those region bounds in
    `configuration::point_leader`, so singleton bounds leave ordinary key
    hashes outside every region before the client can route and prepare the
    first atomic write
- Conclusion:
  - the first concrete mismatch in the packed `hyperdex/config` follow payload
    is the primary-subspace region bounds
  - bootstrap is healthy, string-slice and datatype encoding already moved
    forward, and the next product fix should replace singleton bounds with the
    original contiguous partition hash intervals
- Disposition: `advance`
- Next move:
  - hand the exact region-bound mismatch to the active product worker
  - park this read-only workstream until the product fix lands or another
    packed-config mismatch remains
