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

### Entry `cce-003` - Preregistration

- Timestamp: `2026-03-27 20:24Z`
- Kind: `preregister`
- Hypothesis: a third read-only pass that follows the original HyperDex
  partition logic all the way to concrete packed bytes for the live `profiles`
  primary subspace will produce an exact expected fixture for the region-bounds
  fix, shortening the product worker’s next code pass materially.
- Owner: next delegated read-only worker
- Start commit: `f8306b3`
- Worktree / branch:
  - none required for the bounded step
- Mutable surface:
  - none; read-only evidence gathering only
- Validator:
  - source-backed interval table for the `64` primary regions in the live
    `profiles` config body
  - exact expected packed-byte examples for the first few primary-region bounds
    as emitted by the original HyperDex contract
  - clear pointer to the Rust encoder location that must match those bytes
- Expected artifacts:
  - exact contiguous primary-region interval contract for the live `profiles`
    schema
  - byte-level expected fixture for the packed region bounds
  - a tighter implementation target for `hyh-035`

### Entry `cce-003` - Outcome

- Timestamp: `2026-03-27 20:28Z`
- Kind: `outcome`
- End commit: `1d6093c`
- Artifact location:
  - `/home/friel/c/aaronfriel/HyperDex/admin/hyperspace_builder.cc`
  - `/home/friel/c/aaronfriel/HyperDex/admin/partition.cc`
  - `/home/friel/c/aaronfriel/HyperDex/common/hyperspace.cc`
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/lib.rs`
- Evidence summary:
  - the original `profiles` layout is a one-dimensional primary subspace on
    attribute `0`
  - the original partition math is `interval = 0x0400000000000000`,
    `lower_i = i * interval`, `upper_i = lower_{i+1} - 1`, with the last upper
    bound `UINT64_MAX`
  - the packed region contract is `u64 id, u16 num_hashes, u8 num_replicas,
    (u64 lower, u64 upper)*, replicas...`
  - exact packed lower/upper bytes for the first four regions were recovered,
    matching the encoder locations already changed in `crates/server/src/lib.rs`
- Conclusion: `cce-003` fully verified the region-interval contract that
  `1d6093c` implements. The interval mismatch is no longer the active
  coordinator-config question.
- Disposition: `advance`
- Next move: reopen this workstream for the next read-only comparison after the
  interval fix and identify the next exact packed-config mismatch, if any.

### Entry `cce-004` - Preregistration

- Timestamp: `2026-03-27 20:28Z`
- Kind: `preregister`
- Hypothesis: a fresh read-only comparison of the interval-corrected Rust
  packed config body against the original HyperDex `configuration` / `space`
  packing and client-consumption paths will identify the next exact mismatch
  that still prevents the focused large-object path from reaching `REQ_ATOMIC`.
- Owner: next delegated read-only worker
- Start commit: `1d6093c`
- Worktree / branch:
  - none required; read-only evidence gathering only
- Mutable surface:
  - none
- Validator:
  - source-backed statement of the next exact packed-config or schema-contract
    mismatch after region intervals
  - concrete pointer to the original HyperDex producer or consumer code that
    proves that mismatch
- Expected artifacts:
  - the next exact coordinator-side contract mismatch after region intervals
  - concise explanation of how that mismatch prevents the client from preparing
    the first atomic write

### Entry `cce-004` - Outcome

- Timestamp: `2026-03-27 20:31Z`
- Kind: `outcome`
- End commit: `1c18705`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/lib.rs`
  - `/home/friel/c/aaronfriel/HyperDex/coordinator/coordinator.cc`
  - `/home/friel/c/aaronfriel/HyperDex/common/configuration.cc`
  - `/home/friel/c/aaronfriel/HyperDex/client/client.cc`
  - `/home/friel/c/aaronfriel/HyperDex/common/ids.h`
- Evidence summary:
  - Rust still emits zero-based `space_id`, `subspace_id`, `region_id`, and
    `virtual_server_id` values from defaulted counters in
    `crates/server/src/lib.rs`
  - the original coordinator seeds one shared counter at `1` and assigns
    `space`, `subspace`, `region`, and then replica `virtual_server_id` values
    from that same counter
  - for a one-space, one-subspace, 64-region layout, the first replica tuple
    should be `server_id=1, virtual_server_id=67`, while Rust currently emits
    `server_id=1, virtual_server_id=0`
  - the original client treats `virtual_server_id()` as the null sentinel and
    refuses to send when routing returns that default value
  - the focused public validator still reproduces `Left ClientGarbage`
- Conclusion: the next exact packed-config mismatch after region intervals is
  the ID-allocation contract, with `virtual_server_id=0` as a plausible route-
  preparation blocker before any `REQ_ATOMIC` can be sent.
- Disposition: `advance`
- Next move: hand the ID-allocation mismatch to the active product worker and
  reopen this workstream for one narrower tie-off pass on the failing
  large-object key.

### Entry `cce-005` - Preregistration

- Timestamp: `2026-03-27 20:31Z`
- Kind: `preregister`
- Hypothesis: a final narrow read-only pass can tie the zero-based
  ID-allocation mismatch directly to the focused failing key `"large"`, or
  prove that the next packed-config field after ID allocation is the active
  blocker for that key.
- Owner: next delegated read-only worker
- Start commit: `1c18705`
- Worktree / branch:
  - none required; read-only evidence gathering only
- Mutable surface:
  - none
- Validator:
  - source-backed explanation of whether the failing key `"large"` maps to a
    route that depends on the first replica `virtual_server_id`
  - or the next exact packed-config field after ID allocation if the key-path
    blocker lies one field deeper
- Expected artifacts:
  - direct tie-off between the `"large"` key path and the ID-allocation
    mismatch, or a tighter next mismatch beyond IDs
  - concrete file/function pointers for the original producer and consumer
