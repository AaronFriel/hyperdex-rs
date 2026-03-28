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
  - `docs/workstreams/coordinator-config-evidence/plan.md`
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
    `crates/server/src/lib.rs`
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
  - `crates/server/src/lib.rs`
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
  - `crates/server/src/lib.rs`
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

### Entry `cce-005` - Outcome

- Timestamp: `2026-03-27 20:36Z`
- Kind: `outcome`
- End commit: `05f3abd`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyhac/test/Test/HyperDex/Shared.hs`
  - `/home/friel/c/aaronfriel/hyhac/test/Test/HyperDex/Pool.hs`
  - `/home/friel/c/aaronfriel/HyperDex/common/datatype_string.cc`
  - `/home/friel/c/aaronfriel/HyperDex/common/configuration.cc`
  - `/home/friel/c/aaronfriel/HyperDex/client/client.cc`
  - `crates/server/src/lib.rs`
- Evidence summary:
  - the failing key is `"large"` in the `hyhac` large-object tests
  - `CityHash64("large") = 0xe2d4d8f959c0215c`, which maps to primary region
    index `56` under the corrected 64-partition intervals
  - current Rust config already gives region `56` a non-null replica tuple
    `(server_id=1, virtual_server_id=56)`
  - the original client only rejects the null sentinel path when
    `point_leader(...) == virtual_server_id()`, so `"large"` does not fail
    specifically because of `virtual_server_id=0`
  - the focused public validator still reproduces `Left ClientGarbage`
- Conclusion: the ID-allocation mismatch is real, but it is not the specific
  cause of the failing `"large"` route. The remaining blocker is one step later
  in the client path, beyond coordinator route selection.
- Disposition: `reframe`
- Next move: reopen this workstream one step later in the path and identify
  the next exact pre-daemon contract after route selection for the large-object
  put.

### Entry `cce-006` - Preregistration

- Timestamp: `2026-03-27 20:36Z`
- Kind: `preregister`
- Hypothesis: a new read-only pass that follows the original client path one
  step beyond route selection for the large-object put will identify the next
  exact pre-daemon contract that must hold before `REQ_ATOMIC` is actually
  sent.
- Owner: next delegated read-only worker
- Start commit: `05f3abd`
- Worktree / branch:
  - none required; read-only evidence gathering only
- Mutable surface:
  - none
- Validator:
  - source-backed explanation of the next client-side contract after
    `configuration::point_leader` for the large-object put path
  - or the next exact encoding or function-selection mismatch before daemon
    traffic is sent
- Expected artifacts:
  - the next exact pre-daemon contract after route selection for the large
    object path
  - concrete file/function pointers for the original producer and consumer

### Entry `cce-006` - Outcome

- Timestamp: `2026-03-27 20:41Z`
- Kind: `outcome`
- End commit: `7afc90f`
- Artifact location:
  - `/home/friel/c/aaronfriel/HyperDex/client/client.cc`
  - `/home/friel/c/aaronfriel/HyperDex/common/configuration.cc`
  - `/home/friel/c/aaronfriel/HyperDex/common/mapper.cc`
  - `/home/friel/c/aaronfriel/HyperDex/daemon/communication.cc`
  - `/home/friel/c/aaronfriel/HyperDex/daemon/daemon.cc`
  - `/home/friel/c/aaronfriel/HyperDex/common/key_change.cc`
- Evidence summary:
  - after `configuration::point_leader`, the original client only allocates a
    nonce and sends a header `(mt, flags=0, version, vidt, nonce)`
  - that send path first resolves `vidt -> server_id` through unpacked config
    reverse maps, then resolves `server_id -> address`
  - the daemon-side consumer only treats the frame as a daemon request if the
    header decodes, `vidt` maps back to the receiving server, and the stamped
    config version is acceptable
  - only after those header checks does the daemon reach `process_req_atomic`
    and unpack the `key_change` body
  - the focused public validator still reproduces `Left ClientGarbage`
- Conclusion: the next exact pre-daemon contract after route selection is the
  client-to-daemon routing header, especially the reverse mapping from chosen
  `virtual_server_id` to `server_id` and address, plus the version and `vidt`
  values stamped into the outgoing header.
- Disposition: `advance`
- Next move: hand this routing-header contract to the active product worker and
  reopen this workstream for a direct comparison of Rust’s reverse mapping and
  stamped header against the original acceptance contract.

### Entry `cce-007` - Preregistration

- Timestamp: `2026-03-27 20:41Z`
- Kind: `preregister`
- Hypothesis: a final read-only comparison of Rust’s current
  `virtual_server_id -> server_id -> address` mapping and stamped request
  header against the original HyperDex daemon-side acceptance contract will
  identify the next exact pre-daemon mismatch, if any, before the product
  worker needs to broaden into the atomic body.
- Owner: next delegated read-only worker
- Start commit: `7afc90f`
- Worktree / branch:
  - none required; read-only evidence gathering only
- Mutable surface:
  - none
- Validator:
  - source-backed statement of whether Rust currently produces a valid reverse
    mapping and request header for the `"large"` path
  - or the next exact mismatch in that routing-header contract
- Expected artifacts:
  - the next exact mismatch, if any, in Rust’s reverse mapping or stamped
    request header for the large-object put
  - concrete file/function pointers for the original producer and consumer

### Entry `cce-007` - Outcome

- Timestamp: `2026-03-27 20:46Z`
- Kind: `outcome`
- End commit: `50c064c`
- Artifact location:
  - `crates/server/src/lib.rs`
  - `crates/server/tests/dist_multiprocess_harness.rs`
  - `/home/friel/c/aaronfriel/HyperDex/client/client.cc`
  - `/home/friel/c/aaronfriel/HyperDex/common/configuration.cc`
  - `/home/friel/c/aaronfriel/HyperDex/common/mapper.cc`
  - `/home/friel/c/aaronfriel/HyperDex/daemon/communication.cc`
- Evidence summary:
  - for the concrete failing key `"large"`, current Rust config already gives
    the selected region a non-null replica tuple and matching server-table
    entry from the same config source
  - the original client-side header producer and daemon-side header consumer
    line up cleanly on `(mt, flags=0, version, vidt, nonce)` plus
    `vidt -> server_id -> address`
  - no concrete routing-header mismatch was found for the single-daemon live
    repro path
  - the focused public validator still reproduces `Left ClientGarbage`
- Conclusion: the remaining blocker is later than the routing-header contract
  for the concrete failing key. The next read-only question is the first body
  contract after an accepted header.
- Disposition: `reframe`
- Next move: reopen this workstream one step later and identify the first exact
  body, function-selection, or `key_change` encoding contract after an accepted
  daemon header.

### Entry `cce-008` - Preregistration

- Timestamp: `2026-03-27 20:46Z`
- Kind: `preregister`
- Hypothesis: a new read-only pass that follows the large-object put one step
  beyond an accepted daemon header will identify the first exact body contract
  after the header, whether that is function selection, `key_change` packing,
  or large-object attribute encoding.
- Owner: next delegated read-only worker
- Start commit: `50c064c`
- Worktree / branch:
  - none required; read-only evidence gathering only
- Mutable surface:
  - none
- Validator:
  - source-backed explanation of the first body contract after a daemon-
    acceptable header for the large-object put
  - or the next exact mismatch in that body contract
- Expected artifacts:
  - the next exact post-header contract for the large-object put
  - concrete file/function pointers for the original producer and consumer

### Entry `cce-008` - Outcome

- Timestamp: `2026-03-27 20:51Z`
- Kind: `outcome`
- End commit: `dd2553b`
- Artifact location:
  - `/home/friel/c/aaronfriel/HyperDex/client/c.cc`
  - `/home/friel/c/aaronfriel/HyperDex/client/client.cc`
  - `/home/friel/c/aaronfriel/HyperDex/client/keyop_info.cc`
  - `/home/friel/c/aaronfriel/HyperDex/common/key_change.cc`
  - `/home/friel/c/aaronfriel/HyperDex/common/funcall.cc`
  - `crates/legacy-frontend/src/lib.rs`
  - `crates/legacy-protocol/src/lib.rs`
  - `crates/server/src/lib.rs`
- Evidence summary:
  - original HyperDex lowers `put` to `FUNC_SET` funcalls before send
  - original daemon reads `nonce >> key_change` after the accepted header
  - `key_change` and `funcall` field orderings match the current Rust decode
    path
  - current Rust explicitly accepts `FUNC_SET` with empty `arg2` as a plain
    mutation for the atomic request path
  - the focused public validator still reproduces `Left ClientGarbage`
- Conclusion: the first post-header body contract is structurally aligned on
  current `main`. The remaining blocker is later than function selection and
  later than `key_change` field packing.
- Disposition: `reframe`
- Next move: reopen this workstream for the first daemon-side processing or
  response contract after a structurally valid atomic request.

### Entry `cce-009` - Preregistration

- Timestamp: `2026-03-27 20:51Z`
- Kind: `preregister`
- Hypothesis: a new read-only pass that starts at daemon-side atomic handling
  and the first client-visible response path will identify the next exact
  mismatch after a structurally valid `REQ_ATOMIC`, whether in daemon mutation
  processing, large-value handling, or response encoding.
- Owner: next delegated read-only worker
- Start commit: `dd2553b`
- Worktree / branch:
  - none required; read-only evidence gathering only
- Mutable surface:
  - none
- Validator:
  - source-backed explanation of the first daemon-side processing or response
    contract after a valid large-object atomic request
  - or the next exact mismatch in that post-request path
- Expected artifacts:
  - the next exact daemon-side processing or response contract for the large
    object path
  - concrete file/function pointers for the original producer and consumer

### Entry `cce-009` - Outcome

- Timestamp: `2026-03-27 20:56Z`
- Kind: `outcome`
- End commit: `e29034a`
- Artifact location:
  - `/home/friel/c/aaronfriel/HyperDex/daemon/replication_manager.cc`
  - `/home/friel/c/aaronfriel/HyperDex/common/key_change.cc`
  - `/home/friel/c/aaronfriel/HyperDex/common/funcall.cc`
  - `/home/friel/c/aaronfriel/HyperDex/common/attribute_check.cc`
  - `/home/friel/c/aaronfriel/HyperDex/client/pending_atomic.cc`
  - `crates/server/src/lib.rs`
- Evidence summary:
  - upstream `replication_manager::client_atomic` validates `key_change`
    against the target region schema before execution
  - when that validation fails, upstream emits immediate
    `RESP_ATOMIC/NET_BADDIMSPEC`
  - the original client response path expects a two-byte network return code
    inside `RESP_ATOMIC`
  - current Rust legacy atomic handling decodes and translates directly into
    execution with no upstream-equivalent `kc->validate(schema)` branch and no
    explicit `RESP_ATOMIC/NET_BADDIMSPEC` path
  - the focused public validator still reproduces `Left ClientGarbage`
- Conclusion: the first exact daemon-side divergence after a structurally valid
  atomic request is missing schema validation plus missing explicit
  `RESP_ATOMIC/NET_BADDIMSPEC` response semantics.
- Disposition: `advance`
- Next move: hand this validation-and-error-response contract to the active
  product worker and let product work take priority.

### Entry `cce-010` - Preregistration

- Timestamp: `2026-03-27 21:30Z`
- Kind: `preregister`
- Hypothesis: on the cleaned post-`5879fab` baseline, a new read-only pass can
  verify whether the large-object public failure still reaches daemon traffic,
  or whether the active blocker has moved back into the coordinator
  follow/config contract the client must finish before request preparation.
- Owner: forked read-only worker
- Start commit: `0f7e9e4`
- Worktree / branch:
  - none required; read-only evidence gathering only
- Mutable surface:
  - none
- Validator:
  - source-backed statement of whether the focused large-object path reaches
    daemon request handling on the cleaned baseline
  - if not, the exact coordinator/client-visible contract that remains ahead of
    daemon traffic
- Expected artifacts:
  - clean-baseline evidence about whether daemon traffic exists on the failing
    path
  - the next exact coordinator or daemon contract target for the product worker

### Entry `cce-010` - Outcome

- Timestamp: `2026-03-27 21:34Z`
- Kind: `outcome`
- End commit: `0f7e9e4`
- Artifact location:
  - `/home/friel/c/aaronfriel/HyperDex/client/client.cc`
  - `/home/friel/c/aaronfriel/HyperDex/client/pending_get_partial.cc`
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/lib.rs`
  - manual clean-baseline live-cluster probe with daemon capture cleared after
    startup
- Evidence summary:
  - the fast public check still reproduces `Left ClientGarbage` on current
    `main`
  - a clean manual coordinator-plus-daemon cluster with daemon capture cleared
    after startup reproduced the same failure while recording no daemon request
    frames for the failing large-object path
  - this pushes the active blocker back into the coordinator follow/config path
    that the original client must finish in `client::maintain_coord_connection`
    before it can even reach normal request preparation
  - one exact daemon-side gap is now explicitly queued behind that barrier:
    current Rust still lacks `ReqGetPartial -> RespGetPartial`, while the
    original producer/consumer contract is
    `hyperdex_client_get_partial` -> `pending_get_partial::handle_message`
- Conclusion: the cleaned baseline does not support the earlier daemon-side
  diagnosis as the current blocker. The immediate compatibility gap is again in
  the coordinator follow/config contract, while `ReqGetPartial` remains a real
  downstream daemon gap once that barrier is cleared.
- Disposition: `reframe`
- Next move: hand this reframe to the product and harness workers, then reopen
  this workstream for one narrower read-only step on the remaining
  coordinator follow/config mismatch.

### Entry `cce-011` - Preregistration

- Timestamp: `2026-03-27 21:34Z`
- Kind: `preregister`
- Hypothesis: a new read-only pass focused only on the coordinator
  follow/config path can name the remaining exact mismatch between the original
  client’s `maintain_coord_connection` expectations and the current Rust
  `hyperdex/config` follow behavior on the cleaned large-object baseline.
- Owner: forked read-only worker
- Start commit: `0f7e9e4`
- Worktree / branch:
  - none required; read-only evidence gathering only
- Mutable surface:
  - none
- Validator:
  - source-backed explanation of the remaining coordinator/client-visible
    mismatch between `replicant_client_cond_follow("hyperdex", "config", ...)`
    and the point where the client would start daemon traffic for the large
    object put
- Expected artifacts:
  - the next exact coordinator follow/config mismatch on the cleaned baseline
  - concrete producer/consumer pointers in the original HyperDex and current
    Rust code

### Entry `cce-011` - Outcome

- Timestamp: `2026-03-27 21:46Z`
- Kind: `outcome`
- End commit: `6fe08c5`
- Artifact location:
  - `/home/friel/HyperDex/Replicant/client/client.cc`
  - `/home/friel/HyperDex/Replicant/daemon/daemon.cc`
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/lib.rs`
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/hyperdex-admin-protocol/src/lib.rs`
  - `crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - the new harness result on `main` shows the failing path sends and receives
    only repeated bootstrap traffic on the coordinator connection
  - the original Replicant client only accepts bootstrap when the sender token
    identity and decoded bootstrap body satisfy `si == s.id && c.has(s.id)`
  - while bootstrap is not accepted, the original client stays in the
    `start_bootstrap(...)` retry loop and never advances to follow/config or
    daemon traffic
  - current Rust bootstrap handling queues a synthetic bootstrap response but
    does not implement the original sender-token / encoded-`server.id` binding
    the client checks before adopting config
- Conclusion: the remaining exact mismatch is the Replicant bootstrap
  sender-identity contract. Fixing that is the immediate product target.
- Disposition: `advance`
- Next move: hand this exact target to the product worker, then reopen this
  workstream for a narrower read-only implementation map of the current Rust
  patch points and proving tests.

### Entry `cce-012` - Preregistration

- Timestamp: `2026-03-27 21:46Z`
- Kind: `preregister`
- Hypothesis: a final read-only pass focused only on the bootstrap
  sender-identity contract can name the exact current Rust patch points and the
  minimal proving tests for getting the original client past the bootstrap
  retry loop.
- Owner: next forked read-only worker
- Start commit: `6fe08c5`
- Worktree / branch:
  - none required; read-only evidence gathering only
- Mutable surface:
  - none
- Validator:
  - source-backed mapping from the original sender-token / `server.id`
    contract to the exact current Rust implementation sites that must change
  - minimal focused test or probe suggestions that should prove the client
    leaves bootstrap once the contract is fixed
- Expected artifacts:
  - exact Rust patch points for the bootstrap sender-identity fix
  - exact focused tests or probes to prove the fix

### Entry `cce-012` - Outcome

- Timestamp: `2026-03-27 22:35Z`
- Kind: `outcome`
- End commit: `19fc81f`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/lib.rs`
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/hyperdex-admin-protocol/src/lib.rs`
  - `crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - the sender-id plumbing fix landed on `main`
  - the focused bootstrap unit test now proves sender-id consistency across
    identify reply, bootstrap `server.id`, and bootstrap config server list
  - the focused BusyBee proxy still shows only bootstrap traffic on the Hyhac
    path, so the remaining mismatch is later than that wire-visible sender-id
    contract
- Conclusion: the implementation map was sufficient and is no longer the active
  read-only task. The next exact comparison target is the non-wire bootstrap
  acceptance behavior after sender-id consistency.
- Disposition: `advance`
- Next move: reopen this workstream for one narrower read-only pass on the
  original Replicant client's anonymous-channel bootstrap acceptance versus the
  current Rust session behavior.

### Entry `cce-013` - Preregistration

- Timestamp: `2026-03-27 22:35Z`
- Kind: `preregister`
- Hypothesis: a new read-only pass focused on the original Replicant client's
  anonymous-channel bootstrap acceptance can identify the next exact mismatch
  that still keeps the client in bootstrap after sender-id consistency is
  fixed.
- Owner: next forked read-only worker
- Start commit: `19fc81f`
- Worktree / branch:
  - none required; read-only evidence gathering only
- Mutable surface:
  - none
- Validator:
  - source-backed explanation of the next bootstrap acceptance rule after
    sender-id consistency
  - concrete mapping from that rule to the current Rust session behavior
- Expected artifacts:
  - the next exact non-wire bootstrap acceptance mismatch
  - concrete producer/consumer pointers in original Replicant and current Rust

### Entry `cce-013` - Outcome

- Timestamp: `2026-03-27 22:55Z`
- Kind: `outcome`
- End commit: `main after the repeated-identify fix and corrected BusyBee proxy probe`
- Artifact location:
  - `/home/friel/HyperDex/busybee/busybee.cc`
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/lib.rs`
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - the original BusyBee accept path sends an identify reply only on the first
    transition from anonymous to identified; later identify frames on that
    channel are validate-only
  - current Rust `CoordinatorAdminSession` was still replying to every identify
    frame
  - `CoordinatorAdminSession` now tracks whether the channel is already
    identified and suppresses the second identify reply while still validating
    repeated identify payloads
  - the corrected BusyBee proxy probe now shows non-bootstrap `CondWait`
    traffic and `ClientResponse` completions on the coordinator connection,
    proving the bootstrap acceptance path is no longer the active blocker
- Conclusion: `cce-013` is resolved. The next comparison, if needed, belongs
  later in the post-follow path rather than in bootstrap acceptance.
- Disposition: `advance`
- Next move: park this workstream until the corrected post-follow baseline
  needs another read-only comparison.

### Entry `cce-014` - Preregistration

- Timestamp: `2026-03-27 22:17Z`
- Kind: `preregister`
- Hypothesis: now that the corrected BusyBee probe shows post-bootstrap
  `CondWait` and `ClientResponse` traffic, a new read-only pass can identify
  the first exact post-follow mismatch that still keeps the direct Hyhac loop
  from reaching or succeeding on the daemon path.
- Owner: next forked read-only worker
- Start commit: `2b4104b`
- Worktree / branch:
  - none required; read-only evidence gathering only
- Mutable surface:
  - none
- Validator:
  - source-backed explanation of the first exact mismatch after the corrected
    coordinator `CondWait` / `ClientResponse` phase
  - concrete producer/consumer pointers in the original HyperDex/Replicant
    path and the current Rust path
- Expected artifacts:
  - the next exact post-follow mismatch on the corrected baseline
  - concrete pointers for the next product change

### Entry `cce-014` - Outcome

- Timestamp: `2026-03-27 23:18Z`
- Kind: `outcome`
- End commit: `64104e7`
- Artifact location:
  - discarded read-only notes that contradicted the interval-corrected `main`
    baseline
- Evidence summary:
  - the attempted follow-up analysis revisited the already-landed primary-region
    interval mismatch instead of starting from the corrected post-follow probe
  - current `main` already contains `1d6093c`, and the interval-focused tests
    and later proxy evidence show the client now advances past bootstrap into
    `CondWait` plus `ClientResponse`
  - that return did not reduce the remaining post-follow mismatch on the
    corrected baseline
- Conclusion: `cce-014` should not be reused. The next read-only round must
  start from the corrected post-follow probe and avoid re-opening fixed
  bootstrap or region-interval diagnoses unless current `main` evidence
  disproves them.
- Disposition: `retry`
- Next move: preregister a fresh read-only comparison that starts from the
  corrected post-follow trace and names the first exact remaining mismatch.

### Entry `cce-015` - Preregistration

- Timestamp: `2026-03-27 23:18Z`
- Kind: `preregister`
- Hypothesis: a fresh read-only comparison that starts from the corrected
  post-follow probe will identify the first exact mismatch after the observed
  `CondWait` / `ClientResponse` phase, without regressing into already-fixed
  bootstrap or interval analysis.
- Owner: delegated worker `019d316c-58c6-7981-b76e-86a5a507a3a3` (`Nietzsche`)
- Start commit: `64104e7`
- Worktree / branch:
  - none required; read-only evidence gathering only
- Mutable surface:
  - none
- Validator:
  - source-backed explanation of the first exact mismatch after the corrected
    coordinator `CondWait` / `ClientResponse` phase on current `main`
  - concrete producer/consumer pointers in the original HyperDex/Replicant
    path and the current Rust path
  - explicit confirmation that already-fixed bootstrap sender-id, repeated
    identify, and region-interval issues were not reintroduced
- Expected artifacts:
  - the first exact post-follow mismatch on the corrected current baseline
  - concrete pointers for the next product change

### Entry `cce-015` - Outcome

- Timestamp: `2026-03-27 23:46Z`
- Kind: `outcome`
- End commit: `a618ea0`
- Artifact location:
  - `/home/friel/c/aaronfriel/HyperDex/Replicant/client/pending_cond_follow.cc`
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/lib.rs`
  - `/home/friel/c/aaronfriel/hyhac/src/Database/HyperDex/Internal/Ffi/Client.chs`
  - `/home/friel/c/aaronfriel/hyhac/src/Database/HyperDex/Internal/Core.hs`
  - `/home/friel/c/aaronfriel/hyhac/src/Database/HyperDex/Internal/Handle.hs`
- Evidence summary:
  - the observed coordinator post-follow exchange is already consistent with
    original Replicant: upstream consumes `REPLNET_COND_WAIT` through
    `REPLNET_CLIENT_RESPONSE`, and current Rust now produces that same
    completion shape on the corrected baseline
  - `5e2224a` proves the failing large-object Hyhac subset still returns
    `Left ClientGarbage` while daemon capture stays empty after startup noise
    is cleared
  - taken together, those two facts move the remaining boundary away from
    coordinator follow/config behavior and toward the client-handle path before
    any daemon request is emitted
- Conclusion: the next exact product target is the HyperDex client
  handle/completion contract that Hyhac wraps through its deferred path. The
  next product step should verify that differential directly by comparing the
  native HyperDex client path with Hyhac’s `clientDeferred` /
  `wrapDeferred` / `demandHandle` path on the same live Rust cluster.
- Disposition: `advance`
- Next move: hold this workstream at ready and relaunch product work on that
  differential target.
