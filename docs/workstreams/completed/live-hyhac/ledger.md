# Workstream Ledger: live-hyhac

### Entry `hyh-001` - Preregistration

- Timestamp: `2026-03-27 04:22Z`
- Kind: `preregister`
- Hypothesis: the first live `hyhac` failure against `hyperdex-rs` will appear
  in admin `create space` or `waitUntilStable`, before client traffic starts.
- Owner: next forked product worker in
  `worktrees/live-hyhac-large-object`
- Start commit: `98def36`
- Worktree / branch:
  - root checkout on `main`
- Mutable surface:
  - none for the probe itself
  - follow-up implementation surface to be chosen from observed evidence
- Validator:
  - `cargo build -p server --bin server`
  - start `target/debug/server coordinator --data=/tmp/hyperdex-rs-live/coordinator --listen=127.0.0.1 --listen-port=1982`
  - start `target/debug/server daemon --node-id=1 --threads=1 --data=/tmp/hyperdex-rs-live/daemon1 --listen=127.0.0.1 --listen-port=2012 --control-port=3012 --coordinator=127.0.0.1 --coordinator-port=1982 --transport=grpc`
  - `HYPERDEX_ROOT=/home/friel/c/aaronfriel/HyperDex HYPERDEX_COORD_HOST=127.0.0.1 HYPERDEX_COORD_PORT=1982 /home/friel/c/aaronfriel/hyhac/scripts/cabal.sh test -f tests test:tests --test-show-details=direct --test-option=--plain --test-option=--test-seed=1`
- Expected artifacts:
  - observed first failing operation from a real `hyhac` run
  - captured admin or client error surface
  - next bounded compatibility step chosen from that evidence

### Entry `hyh-001` - Outcome

- Timestamp: `2026-03-27 04:33Z`
- Kind: `outcome`
- End commit: `329a469`
- Artifact location:
  - live probe against `/tmp/hyperdex-rs-live`
  - `/tmp/hyperdex-rs-live/coordinator.log`
  - `/tmp/hyperdex-rs-live/daemon.log`
- Evidence summary:
  - `timeout 30s ... hyhac/scripts/cabal.sh test ...` timed out against a live
    `hyperdex-rs` coordinator plus daemon cluster
  - after `329a469`, the coordinator stayed alive instead of crashing on
    malformed admin connections
  - `timeout 5s ... hyperdex-add-space -h 127.0.0.1 -p 1982` also timed out
- Conclusion: the next missing public contract is the legacy coordinator admin
  frontend used by the C admin library, starting with `add_space` and
  `wait_until_stable`.
- Disposition: `advance`
- Next move: preregister and launch a bounded implementation step for the
  legacy coordinator admin frontend.

### Entry `hyh-002` - Preregistration

- Timestamp: `2026-03-27 04:33Z`
- Kind: `preregister`
- Hypothesis: implementing the legacy coordinator admin frontend for
  `add_space` and `wait_until_stable` will unblock the first `hyhac` admin test
  and the matching HyperDex admin tools.
- Owner: dedicated worker in `worktrees/dist-control-plane`
- Start commit: `faa6cb6`
- Worktree / branch:
  - `worktrees/dist-control-plane`
- Mutable surface:
  - `crates/server/**`
  - `crates/hyperdex-admin-protocol/**`
  - any new crate added only if it is strictly for the legacy coordinator admin
    frontend
- Validator:
  - `cargo test -p server coordinator_control_service_ -- --nocapture`
  - `cargo test --workspace`
  - `timeout 5s bash -lc 'printf \"%s\\n\" \"space profiles key username attributes string first, int profile_views tolerate 0 failures\" | LD_LIBRARY_PATH=/home/friel/c/aaronfriel/HyperDex/.libs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH} /home/friel/c/aaronfriel/HyperDex/hyperdex-add-space -h 127.0.0.1 -p 1982'`
  - `timeout 5s bash -lc 'LD_LIBRARY_PATH=/home/friel/c/aaronfriel/HyperDex/.libs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH} /home/friel/c/aaronfriel/HyperDex/hyperdex-wait-until-stable -h 127.0.0.1 -p 1982'`
- Expected artifacts:
  - live legacy admin endpoint that no longer times out on `add_space`
  - live legacy admin endpoint that no longer times out on `wait_until_stable`
  - one bounded commit ready for reconciliation

### Entry `hyh-002` - Outcome

- Timestamp: `2026-03-27 04:39Z`
- Kind: `outcome`
- End commit: `d2c133c`
- Artifact location:
  - no code changes in `worktrees/dist-control-plane`
- Evidence summary:
  - the retired worker reported no file changes
  - the retired worker identified insufficient verified wire detail for the
    Replicant-backed admin path as the exact blocker
- Conclusion: the bounded implementation attempt should not continue until the
  original HyperDex admin framing and completion flow are concrete enough to
  code without guessing.
- Disposition: `retry`
- Next move: preregister a read-only protocol evidence pass, then reopen the
  implementation step with tighter verified scope.

### Entry `hyh-003` - Preregistration

- Timestamp: `2026-03-27 04:39Z`
- Kind: `preregister`
- Hypothesis: a read-only pass over the original HyperDex admin client path
  can recover enough verified framing and completion detail to reopen the
  bounded legacy-admin implementation safely.
- Owner: read-only worker on the original HyperDex and `hyhac` sources
- Start commit: `d2c133c`
- Worktree / branch:
  - root checkout on `main`
  - no `hyperdex-rs` code edits for this pass
- Mutable surface:
  - none; read-only evidence gathering only
- Validator:
  - verified findings tied to concrete source paths in `/home/friel/c/aaronfriel/HyperDex`
  - enough protocol detail to define the next bounded implementation surface
- Expected artifacts:
  - verified transport and completion facts for the original admin client path
  - a tighter implementation target for the replacement legacy-admin worker

### Entry `hyh-003` - Outcome

- Timestamp: `2026-03-27 04:41Z`
- Kind: `outcome`
- End commit: `cd0d58c`
- Artifact location:
  - original HyperDex admin/client sources under `/home/friel/c/aaronfriel/HyperDex`
- Evidence summary:
  - `space_add` is issued through `replicant_client_call(..., "hyperdex", "space_add", packed_space, ...)`
  - `wait_until_stable` is issued through `replicant_client_cond_wait(..., "hyperdex", "stable", m_config.version(), ...)`
  - `coord_rpc_generic` maps a two-byte coordinator return-code body for
    function calls, while condition waits can succeed without such a body
  - `hyperdex_admin_loop` is the completion path that returns the same request
    id originally returned by `add_space` / `wait_until_stable`
- Conclusion: the next implementation step can be bounded safely around a
  Replicant-compatible coordinator admin path for `space_add`,
  `wait_until_stable`, and loop completion.
- Disposition: `advance`
- Next move: preregister and launch the replacement implementation worker on
  that verified scope.

### Entry `hyh-004` - Preregistration

- Timestamp: `2026-03-27 04:41Z`
- Kind: `preregister`
- Hypothesis: implementing the verified Replicant-compatible coordinator admin
  behavior for `space_add`, `wait_until_stable`, and request-id-plus-loop
  completion will unblock the original C admin tools and the first live `hyhac`
  admin test.
- Owner: dedicated worker in `worktrees/dist-control-plane`
- Start commit: `cd0d58c`
- Worktree / branch:
  - `worktrees/dist-control-plane`
- Mutable surface:
  - `crates/server/**`
  - `crates/hyperdex-admin-protocol/**`
  - any small adjacent support code strictly needed for the legacy admin path
- Validator:
  - `cargo test -p server coordinator_control_service_ -- --nocapture`
  - `cargo test --workspace`
  - `timeout 5s bash -lc 'printf \"%s\\n\" \"space profiles key username attributes string first, int profile_views tolerate 0 failures\" | LD_LIBRARY_PATH=/home/friel/c/aaronfriel/HyperDex/.libs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH} /home/friel/c/aaronfriel/HyperDex/hyperdex-add-space -h 127.0.0.1 -p 1982'`
  - `timeout 5s bash -lc 'LD_LIBRARY_PATH=/home/friel/c/aaronfriel/HyperDex/.libs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH} /home/friel/c/aaronfriel/HyperDex/hyperdex-wait-until-stable -h 127.0.0.1 -p 1982'`
- Expected artifacts:
  - live legacy admin endpoint that no longer times out on `add_space`
  - live legacy admin endpoint that no longer times out on `wait_until_stable`
  - one bounded commit ready for reconciliation

### Entry `hyh-004` - Outcome

- Timestamp: `2026-03-27 04:45Z`
- Kind: `outcome`
- End commit: `5b6b614`
- Artifact location:
  - no code changes in `worktrees/dist-control-plane`
- Evidence summary:
  - the replacement worker again reported no file changes
  - the replacement worker confirmed the remaining blocker is concrete
    Replicant transport framing, not the higher-level admin operation flow
- Conclusion: the implementation step should not be retried until the Replicant
  framing is pinned down more concretely.
- Disposition: `retry`
- Next move: preregister two narrower evidence steps, one for Replicant
  framing from source and one for dynamic packet/response capture.

### Entry `hyh-005` - Preregistration

- Timestamp: `2026-03-27 04:45Z`
- Kind: `preregister`
- Hypothesis: a source-focused pass over Replicant client/server code can
  recover enough concrete transport framing detail to constrain the Rust
  compatibility layer.
- Owner: delegated read-only worker
- Start commit: `5b6b614`
- Worktree / branch:
  - none; read-only source pass only
- Mutable surface:
  - none
- Validator:
  - verified findings tied to concrete Replicant and HyperDex source paths
  - explicit framing facts that reduce implementation ambiguity
- Expected artifacts:
  - concrete Replicant framing facts
  - implementation implications for the Rust coordinator path

### Entry `hyh-005` - Outcome

- Timestamp: `2026-03-27 04:46Z`
- Kind: `outcome`
- End commit: `85cf798`
- Artifact location:
  - original BusyBee and Replicant sources under `/home/friel/HyperDex`
  - original HyperDex admin sources under `/home/friel/c/aaronfriel/HyperDex`
- Evidence summary:
  - BusyBee framing uses a 4-byte big-endian size header, with an extended-size
    path for large frames
  - Replicant request and response bodies begin with a one-byte
    `network_msgtype`
  - `call`, `cond_wait`, robust-call setup, and `CLIENT_RESPONSE` body layouts
    are now tied to concrete source paths
  - fixed-width integers are big-endian and slice fields use `e::slice`
    varint-length encoding
- Conclusion: the remaining framing ambiguity is low enough to reopen the Rust
  compatibility implementation safely.
- Disposition: `advance`
- Next move: combine this with the dynamic-capture result and relaunch the
  control-plane implementation worker.

### Entry `hyh-006` - Preregistration

- Timestamp: `2026-03-27 04:45Z`
- Kind: `preregister`
- Hypothesis: a dynamic capture pass using the original HyperDex admin tools
  against a dummy listener can recover the first request bytes and any immediate
  response expectations for `add_space` and `wait_until_stable`.
- Owner: delegated read-only worker
- Start commit: `5b6b614`
- Worktree / branch:
  - none; dynamic capture only
- Mutable surface:
  - none
- Validator:
  - captured byte sequences and concise interpretation
  - concrete difference between `add_space` and `wait_until_stable` startup
    behavior if any
- Expected artifacts:
  - first-packet captures for the legacy admin tools
  - concrete transport facts that reduce implementation ambiguity

### Entry `hyh-006` - Outcome

- Timestamp: `2026-03-27 04:46Z`
- Kind: `outcome`
- End commit: `85cf798`
- Artifact location:
  - dynamic packet captures from the original HyperDex admin tools
- Evidence summary:
  - both `hyperdex-add-space` and `hyperdex-wait-until-stable` send the same
    first 25-byte packet before any server response:
    `80 00 00 14 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 05 1c`
  - both tools block after that first packet until a valid response arrives or
    the outer timeout kills them
  - neither tool waits for a server banner before sending
  - the source path explains the identical first packet: both operations first
    perform Replicant bootstrap
- Conclusion: the replacement frontend must satisfy the initial `config`
  bootstrap before it can ever see operation-specific `space_add` or
  `wait_until_stable` traffic.
- Disposition: `advance`
- Next move: relaunch the implementation worker with this initial handshake
  fact included explicitly in scope.

### Entry `hyh-007` - Preregistration

- Timestamp: `2026-03-27 04:46Z`
- Kind: `preregister`
- Hypothesis: implementing the verified BusyBee-framed, Replicant-compatible
  coordinator admin behavior in one bounded worker will unblock the legacy
  admin tools.
- Owner: dedicated worker in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/dist-control-plane`
- Start commit: `801d20f`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/dist-control-plane`
- Mutable surface:
  - `crates/server/**`
  - `crates/hyperdex-admin-protocol/**`
- Validator:
  - `cargo test -p server coordinator_control_service_ -- --nocapture`
  - `cargo test --workspace`
  - `timeout 5s bash -lc 'printf \"%s\\n\" \"space profiles key username attributes string first, int profile_views tolerate 0 failures\" | LD_LIBRARY_PATH=/home/friel/c/aaronfriel/HyperDex/.libs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH} /home/friel/c/aaronfriel/HyperDex/hyperdex-add-space -h 127.0.0.1 -p 1982'`
  - `timeout 5s bash -lc 'LD_LIBRARY_PATH=/home/friel/c/aaronfriel/HyperDex/.libs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH} /home/friel/c/aaronfriel/HyperDex/hyperdex-wait-until-stable -h 127.0.0.1 -p 1982'`
- Expected artifacts:
  - legacy admin endpoint no longer timing out
  - one bounded commit ready for reconciliation

### Entry `hyh-007` - Outcome

- Timestamp: `2026-03-27 04:56Z`
- Kind: `outcome`
- End commit: `801d20f`
- Artifact location:
  - no code changes in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/dist-control-plane`
- Evidence summary:
  - the worker again reported no file changes
  - the worker named broad implementation design, not missing protocol facts,
    as the blocker
- Conclusion: the implementation must be split into narrower write scopes.
- Disposition: `retry`
- Next move: preregister separate admin-codec and server-integration steps.

### Entry `hyh-008` - Preregistration

- Timestamp: `2026-03-27 04:56Z`
- Kind: `preregister`
- Hypothesis: a dedicated admin-codec worker limited to BusyBee and Replicant
  framing code will produce the reusable parser and encoder pieces without
  getting blocked on server integration.
- Owner: dedicated worker in a new admin-codec worktree
- Start commit: `801d20f`
- Worktree / branch:
  - to be created by root
- Mutable surface:
  - `crates/hyperdex-admin-protocol/**`
  - any tiny adjacent support code strictly needed for codec tests
- Validator:
  - targeted codec tests added by the worker
  - `cargo test -p hyperdex-admin-protocol`
  - `cargo test --workspace`
- Expected artifacts:
  - BusyBee frame reader/writer
  - Replicant request and response codec for the admin path
  - one bounded commit ready for reconciliation

### Entry `hyh-009` - Preregistration

- Timestamp: `2026-03-27 04:56Z`
- Kind: `preregister`
- Hypothesis: a dedicated server-integration worker limited to listener,
  session state, and loop-completion behavior can wire the admin codec into the
  coordinator path once the codec surface is available.
- Owner: dedicated worker in a new admin-server worktree
- Start commit: `801d20f`
- Worktree / branch:
  - to be created by root
- Mutable surface:
  - `crates/server/**`
  - `crates/hyperdex-admin-protocol/**` only for tiny integration glue if
    unavoidable
- Validator:
  - `cargo test -p server coordinator_control_service_ -- --nocapture`
  - `cargo test --workspace`
  - live admin tool probes after codec integration is available
- Expected artifacts:
  - coordinator-side admin session state machine
  - loop-completion behavior
  - one bounded commit ready for reconciliation

### Entry `hyh-008` - Outcome

- Timestamp: `2026-03-27 05:00Z`
- Kind: `outcome`
- End commit: `f2da7e5`
- Artifact location:
  - no code changes in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-codec`
- Evidence summary:
  - the dedicated admin-codec worker was interrupted after the worktree stayed
    clean at `801d20f`
  - there was no salvageable diff in `crates/hyperdex-admin-protocol/**`
- Conclusion: the codec task still needs a tighter contract than "implement
  the codec" before a worker will produce code.
- Disposition: `retry`
- Next move: preregister a pure-codec step with explicit message shapes,
  helper names, and required tests.

### Entry `hyh-009` - Outcome

- Timestamp: `2026-03-27 05:00Z`
- Kind: `outcome`
- End commit: `f2da7e5`
- Artifact location:
  - no code changes in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-server`
- Evidence summary:
  - the dedicated server worker was interrupted after the worktree stayed clean
    at `801d20f`
  - there was no salvageable diff in `crates/server/**`
- Conclusion: the server step should not be phrased as broad integration until
  the exact listener and session hooks are named against a concrete codec
  surface.
- Disposition: `retry`
- Next move: preregister a read-only server-mapping step that names those hooks
  explicitly, then reopen implementation on that narrower target.

### Entry `hyh-010` - Preregistration

- Timestamp: `2026-03-27 05:00Z`
- Kind: `preregister`
- Hypothesis: a worker with an explicit codec contract can land the pure
  BusyBee and Replicant admin frame types, varint slice helpers, and unit tests
  inside `hyperdex-admin-protocol` without depending on server behavior.
- Owner: dedicated worker in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-codec`
- Start commit: `f2da7e5`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-codec`
- Mutable surface:
  - `crates/hyperdex-admin-protocol/**`
- Validator:
  - targeted codec tests for BusyBee framing, varint slice encoding, and
    Replicant message round-trips
  - `cargo test -p hyperdex-admin-protocol`
- Expected artifacts:
  - concrete request and response enums for the legacy admin path
  - BusyBee frame reader/writer helpers
  - `e::slice`-style varint helpers
  - one bounded commit ready for reconciliation

### Entry `hyh-011` - Preregistration

- Timestamp: `2026-03-27 05:00Z`
- Kind: `preregister`
- Hypothesis: a read-only server-mapping pass can name the exact functions,
  types, and tests that must change in `crates/server/**` once the codec lands,
  so the next integration worker no longer has to invent that shape.
- Owner: delegated read-only worker
- Start commit: `f2da7e5`
- Worktree / branch:
  - root checkout on `main`
  - no code edits for this pass
- Mutable surface:
  - none; read-only mapping only
- Validator:
  - concrete path and function list for coordinator listener, session state,
    completion loop, and live-tool probes
- Expected artifacts:
  - exact server insertion points and test hooks
  - one narrower implementation target for the follow-up server worker

### Entry `hyh-011` - Outcome

- Timestamp: `2026-03-27 05:04Z`
- Kind: `outcome`
- End commit: `e3253b4`
- Artifact location:
  - read-only implementation map against
    `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/lib.rs`
    and `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/main.rs`
- Evidence summary:
  - the coordinator listener insertion point is the `coordinator` branch in
    `crates/server/src/main.rs`
  - the existing usable runtime state is `CoordinatorState`,
    `ClusterRuntime::config_view`, `ClusterRuntime::stable_version`, and
    `ClusterRuntime::record_config_change`
  - the current dispatch path already supports `space_add`, but
    `handle_coordinator_admin_request` still rejects `WaitUntilStable` and
    `ConfigGet` as malformed
  - the exact missing state for a legacy coordinator admin session is:
    followed config version, request-id allocation, pending completions keyed
    by request id, and loop polling that removes completions when returned
  - the smallest useful integration tests are coordinator listener tests for
    config-follow, `space_add`, and `wait_until_stable`, plus one
    process-level proof in `dist_multiprocess_harness.rs`
- Conclusion: the server-side shape is now explicit enough to reopen
  implementation once the codec exists.
- Disposition: `advance`
- Next move: reconcile the codec worker result if it lands, then reopen one
  substantial server implementation step on top of that codec and this map.

### Entry `hyh-010` - Outcome

- Timestamp: `2026-03-27 05:07Z`
- Kind: `outcome`
- End commit: `489de25`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/hyperdex-admin-protocol/src/lib.rs`
- Evidence summary:
  - the codec worker produced `ac16953` and root reconciled it as `489de25`
    (`Add legacy admin codec helpers`)
  - the landed code adds BusyBee frame helpers, Replicant admin request and
    response codecs, varint slice helpers, and targeted protocol tests
  - `cargo test -p hyperdex-admin-protocol` passed
  - the captured 25-byte bootstrap request is now covered by an exact byte
    assertion and BusyBee stream round-trip
- Conclusion: the protocol foundation is now strong enough to stop splitting
  prep work and move directly into server implementation.
- Disposition: `advance`
- Next move: preregister one substantial server implementation step on top of
  the landed codec and completed server map.

### Entry `hyh-012` - Preregistration

- Timestamp: `2026-03-27 05:07Z`
- Kind: `preregister`
- Hypothesis: one substantial server worker can implement the full
  coordinator-side legacy admin path now that the codec and the exact server
  insertion points are both available.
- Owner: dedicated worker in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-server`
- Start commit: `489de25`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-server`
- Mutable surface:
  - `crates/server/**`
  - `crates/hyperdex-admin-protocol/**` only for small integration glue if
    strictly necessary
- Validator:
  - `cargo test -p server`
  - `cargo test --workspace`
  - `timeout 5s bash -lc 'printf \"%s\\n\" \"space profiles key username attributes string first, int profile_views tolerate 0 failures\" | LD_LIBRARY_PATH=/home/friel/c/aaronfriel/HyperDex/.libs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH} /home/friel/c/aaronfriel/HyperDex/hyperdex-add-space -h 127.0.0.1 -p 1982'`
  - `timeout 5s bash -lc 'LD_LIBRARY_PATH=/home/friel/c/aaronfriel/HyperDex/.libs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH} /home/friel/c/aaronfriel/HyperDex/hyperdex-wait-until-stable -h 127.0.0.1 -p 1982'`
- Expected artifacts:
  - coordinator-side legacy admin listener
  - session state with request-id allocation and pending completions
  - initial config-follow handling
  - `space_add` and `wait_until_stable` loop completion
  - one bounded commit ready for reconciliation

### Entry `hyh-012` - Outcome

- Timestamp: `2026-03-27 05:11Z`
- Kind: `outcome`
- End commit: `928130e`
- Artifact location:
  - no code changes in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-server`
- Evidence summary:
  - the full server implementation worker was interrupted
  - the `admin-server` worktree remained clean at `928130e`
  - no blocker report was returned before interruption
- Conclusion: the scope is correct, but the worker prompt still allowed too
  much exploration and not enough pressure to patch the identified functions.
- Disposition: `retry`
- Next move: relaunch the same substantial server step with the concrete patch
  targets named explicitly in `crates/server/src/main.rs` and
  `crates/server/src/lib.rs`, and require either a real diff or a precise
  blocker report immediately.

### Entry `hyh-013` - Preregistration

- Timestamp: `2026-03-27 05:11Z`
- Kind: `preregister`
- Hypothesis: a worker told to patch the exact coordinator listener and session
  functions already identified can land the legacy admin server implementation
  without another exploratory stall.
- Owner: dedicated worker in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-server`
- Start commit: `928130e`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-server`
- Mutable surface:
  - `crates/server/**`
  - `crates/hyperdex-admin-protocol/**` only for small integration glue if
    strictly necessary
- Validator:
  - `cargo test -p server`
  - `cargo test --workspace`
  - `timeout 5s bash -lc 'printf \"%s\\n\" \"space profiles key username attributes string first, int profile_views tolerate 0 failures\" | LD_LIBRARY_PATH=/home/friel/c/aaronfriel/HyperDex/.libs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH} /home/friel/c/aaronfriel/HyperDex/hyperdex-add-space -h 127.0.0.1 -p 1982'`
  - `timeout 5s bash -lc 'LD_LIBRARY_PATH=/home/friel/c/aaronfriel/HyperDex/.libs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH} /home/friel/c/aaronfriel/HyperDex/hyperdex-wait-until-stable -h 127.0.0.1 -p 1982'`
- Expected artifacts:
  - coordinator-side legacy admin listener
  - session state with request-id allocation and pending completions
  - initial config-follow handling
  - `space_add` and `wait_until_stable` loop completion
  - one bounded commit ready for reconciliation

### Entry `hyh-013` - Outcome

- Timestamp: `2026-03-27 05:14Z`
- Kind: `outcome`
- End commit: `ee09ee0`
- Artifact location:
  - no code changes in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-server`
- Evidence summary:
  - the explicit-patch-target retry was interrupted
  - the `admin-server` worktree remained clean at `ee09ee0`
  - no blocker report was returned before interruption
- Conclusion: the problem is no longer task definition; it is the execution
  shape of the delegated server step.
- Disposition: `reframe`
- Next move: relaunch the same substantial server implementation with a forked
  implementation worker and a separate read-only reviewer on the session-state
  machine.

### Entry `hyh-014` - Preregistration

- Timestamp: `2026-03-27 05:14Z`
- Kind: `preregister`
- Hypothesis: changing the execution shape will finally produce code. A forked
  implementation worker should inherit enough context to patch the server, and
  a separate reviewer can keep the session-state machine precise without
  touching files.
- Owner: root-coordinated pair
- Start commit: `ee09ee0`
- Worktree / branch:
  - implementation: `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-server`
  - reviewer: read-only, no code edits
- Mutable surface:
  - implementation worker:
    - `crates/server/**`
    - `crates/hyperdex-admin-protocol/**` only for small integration glue if
      strictly necessary
  - reviewer:
    - none; read-only analysis only
- Validator:
  - `cargo test -p server`
  - `cargo test --workspace`
  - `timeout 5s bash -lc 'printf \"%s\\n\" \"space profiles key username attributes string first, int profile_views tolerate 0 failures\" | LD_LIBRARY_PATH=/home/friel/c/aaronfriel/HyperDex/.libs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH} /home/friel/c/aaronfriel/HyperDex/hyperdex-add-space -h 127.0.0.1 -p 1982'`
  - `timeout 5s bash -lc 'LD_LIBRARY_PATH=/home/friel/c/aaronfriel/HyperDex/.libs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH} /home/friel/c/aaronfriel/HyperDex/hyperdex-wait-until-stable -h 127.0.0.1 -p 1982'`
- Expected artifacts:
  - coordinator-side legacy admin listener
  - session state with request-id allocation and pending completions
  - initial config-follow handling
  - `space_add` and `wait_until_stable` loop completion
  - read-only reviewer notes on the session-state machine and risky branches
  - one bounded commit ready for reconciliation

### Entry `hyh-014` - Outcome

- Timestamp: `2026-03-27 05:17Z`
- Kind: `outcome`
- End commit: `2641a75`
- Artifact location:
  - no code changes in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-server`
  - read-only review against
    `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/main.rs`
    and `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/lib.rs`
- Evidence summary:
  - the forked implementation worker still produced no diff
  - the reviewer established that the coordinator currently exposes only the
    JSON `CoordinatorControlService`
  - the existing coordinator control transport encodes `method + JSON body`,
    which is incompatible with the landed BusyBee/Replicant codec
  - no coordinator-side legacy admin listener, config-follow path, nonce
    allocator, or completion queue exists today
- Conclusion: the correct next target is not "wire the current coordinator
  control service"; it is a separate BusyBee/Replicant coordinator admin
  transport and session layer.
- Disposition: `reframe`
- Next move: preregister and launch one substantial implementation step on that
  corrected transport target.

### Entry `hyh-015` - Preregistration

- Timestamp: `2026-03-27 05:17Z`
- Kind: `preregister`
- Hypothesis: implementing a separate BusyBee/Replicant coordinator admin
  transport and session layer will finally unblock the original admin tools,
  because it matches the landed codec instead of trying to reuse the JSON
  control path.
- Owner: dedicated worker in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-server`
- Start commit: `2641a75`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-server`
- Mutable surface:
  - `crates/server/**`
  - `crates/hyperdex-admin-protocol/**` only for small integration glue if
    strictly necessary
- Validator:
  - `cargo test -p server`
  - `cargo test --workspace`
  - `timeout 5s bash -lc 'printf \"%s\\n\" \"space profiles key username attributes string first, int profile_views tolerate 0 failures\" | LD_LIBRARY_PATH=/home/friel/c/aaronfriel/HyperDex/.libs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH} /home/friel/c/aaronfriel/HyperDex/hyperdex-add-space -h 127.0.0.1 -p 1982'`
  - `timeout 5s bash -lc 'LD_LIBRARY_PATH=/home/friel/c/aaronfriel/HyperDex/.libs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH} /home/friel/c/aaronfriel/HyperDex/hyperdex-wait-until-stable -h 127.0.0.1 -p 1982'`
- Expected artifacts:
  - separate coordinator-side BusyBee/Replicant admin listener
  - per-connection session state with nonce allocation and pending completions
  - initial config-follow handling
  - `space_add` and `wait_until_stable` completion frames
  - one bounded commit ready for reconciliation

### Entry `hyh-015` - Outcome

- Timestamp: `2026-03-27 05:21Z`
- Kind: `outcome`
- End commit: `175ed25`
- Artifact location:
  - no code changes in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-server`
- Evidence summary:
  - the corrected-transport worker still produced no diff
  - it returned a precise blocker report matching the reviewer: the current
    server has no coordinator-side legacy admin listener, no BusyBee/Replicant
    transport boundary, no nonce allocator, and no pending-completion store
  - the current JSON coordinator control helpers in `crates/server/src/lib.rs`
    are fundamentally incompatible with the landed codec
- Conclusion: the target is correct, but the next implementation step should be
  phrased around the coordinator service core in `crates/server/src/lib.rs`
  instead of the whole end-to-end listener/wiring stack at once.
- Disposition: `retry`
- Next move: preregister a substantial service-core implementation step in
  `crates/server/src/lib.rs`, then wire startup/tests on top of it.

### Entry `hyh-016` - Preregistration

- Timestamp: `2026-03-27 05:21Z`
- Kind: `preregister`
- Hypothesis: a worker focused on the coordinator BusyBee/Replicant service
  core in `crates/server/src/lib.rs` can land the transport/session machinery
  once the end-to-end framing is no longer bundled with startup wiring.
- Owner: dedicated worker in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-server`
- Start commit: `175ed25`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-server`
- Mutable surface:
  - `crates/server/src/lib.rs`
  - `crates/hyperdex-admin-protocol/**` only for small integration glue if
    strictly necessary
- Validator:
  - focused server tests for the service core if added
  - `cargo test -p server`
- Expected artifacts:
  - coordinator BusyBee/Replicant service core
  - per-connection session state with nonce allocation and pending completions
  - config-follow, `space_add`, and `wait_until_stable` frame handling
  - one bounded commit ready for reconciliation

### Entry `hyh-016` - Outcome

- Timestamp: `2026-03-27 05:24Z`
- Kind: `outcome`
- End commit: `51002a7`
- Artifact location:
  - no code changes in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-server`
- Evidence summary:
  - the service-core worker still produced no diff
  - it returned a precise blocker report: the server has no Rust decoder for
    the original packed `hyperdex::space` payload carried by
    `ReplicantAdminRequestMessage::space_add`
  - `crates/server/src/lib.rs` can only consume `CoordinatorAdminRequest::SpaceAdd(Space)`
    or `LegacyAdminRequest::SpaceAddDsl(String)`
  - `crates/data-model/src/lib.rs` only exposes `parse_hyperdex_space(&str)`,
    not a binary unpacker
- Conclusion: the immediate missing capability is the packed `space_add`
  payload decoder, not the rest of the transport/service stack.
- Disposition: `retry`
- Next move: preregister a substantial decoder implementation step and then
  reconnect it to the coordinator service core.

### Entry `hyh-017` - Preregistration

- Timestamp: `2026-03-27 05:24Z`
- Kind: `preregister`
- Hypothesis: implementing the packed `space_add` payload decoder and the
  matching service-core consumption path will unblock the coordinator
  BusyBee/Replicant service work.
- Owner: dedicated worker in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-server`
- Start commit: `51002a7`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-server`
- Mutable surface:
  - `crates/server/src/lib.rs`
  - `crates/hyperdex-admin-protocol/**` only for small integration glue if
    strictly necessary
  - `crates/data-model/**` only if a decoder helper must live there
- Validator:
  - focused server tests for `space_add` request decoding if added
  - `cargo test -p server`
- Expected artifacts:
  - packed `space_add` payload decoder
  - service-core path that turns decoded bytes into `Space`
  - one bounded commit ready for reconciliation

### Entry `hyh-018` - Preregistration

- Timestamp: `2026-03-27 05:24Z`
- Kind: `preregister`
- Hypothesis: two implementation workers with disjoint write scopes can land
  the missing protocol pieces faster than another single broad attempt.
- Owner: root-coordinated pair
- Start commit: `51002a7`
- Worktree / branch:
  - decoder: dedicated worktree for protocol/data-model decoding work
  - service core: `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-server`
- Mutable surface:
  - decoder worker:
    - `crates/hyperdex-admin-protocol/**`
    - `crates/data-model/**` if the decoder belongs there
  - service-core worker:
    - `crates/server/src/lib.rs`
    - `crates/hyperdex-admin-protocol/**` only for small integration glue if
      strictly necessary
- Validator:
  - decoder worker:
    - focused decoder tests
    - `cargo test -p hyperdex-admin-protocol`
  - service-core worker:
    - focused server tests
    - `cargo test -p server`
- Expected artifacts:
  - packed `space_add` payload decoder from the original HyperDex format
  - coordinator BusyBee/Replicant service core that consumes decoded `Space`
  - two bounded commits ready for reconciliation

### Entry `hyh-018` - Outcome

- Timestamp: `2026-03-27 05:27Z`
- Kind: `outcome`
- End commit: `962c5dd`
- Artifact location:
  - no code changes in
    `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-decoder`
  - no code changes in
    `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-server`
- Evidence summary:
  - both parallel implementation workers were interrupted with clean worktrees
  - the decoder-side blocker remained the missing packed `space_add` decoder
  - the service-core side remained blocked on consuming decoded `Space`
  - root then pinned down the exact binary format from:
    - `/home/friel/c/aaronfriel/HyperDex/common/hyperspace.cc`
    - `/home/friel/c/aaronfriel/HyperDex/admin/admin.cc`
- Conclusion: the split was still right, but the workers lacked the exact
  source-file targets for the original binary format.
- Disposition: `retry`
- Next move: relaunch both implementation steps with direct `hyperspace.cc`
  and `admin.cc` source targets.

### Entry `hyh-019` - Preregistration

- Timestamp: `2026-03-27 06:05Z`
- Kind: `preregister`
- Hypothesis: porting the packed `hyperdex::space` decoder and mapping
  Replicant admin requests into `CoordinatorAdminRequest` values will remove
  the remaining format ambiguity from the coordinator admin service work and
  produce a code result that the next live session layer can consume directly.
- Owner: root checkout on `main`
- Start commit: `afd3f8b`
- Worktree / branch:
  - root checkout on `main`
- Mutable surface:
  - `crates/hyperdex-admin-protocol/**`
  - `crates/server/**` only for small integration glue and focused tests
- Validator:
  - focused protocol tests for packed-space decoding and request mapping
  - `cargo test -p hyperdex-admin-protocol`
  - `cargo test -p server`
- Expected artifacts:
  - packed `hyperdex::space` decoder
  - Replicant admin request to coordinator-request mapping
  - focused tests proving `space_add`, `space_rm`, and `wait_until_stable`
    semantics through the new mapping layer

### Entry `hyh-019` - Outcome

- Timestamp: `2026-03-27 05:39Z`
- Kind: `outcome`
- End commit: `df633ac`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/hyperdex-admin-protocol/src/lib.rs`
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/lib.rs`
- Evidence summary:
  - `decode_packed_hyperdex_space` now ports the original
    `hyperdex::space` binary layout into the Rust `Space` model, including
    key-attribute handling, subspace translation, partition recovery, and
    HyperDex datatype mapping
  - `ReplicantAdminRequestMessage::into_coordinator_request` now maps
    `space_add`, `space_rm`, `wait_until_stable`, and `config` condition waits
    into coordinator-facing request values
  - `handle_replicant_admin_request` now emits real Replicant call and
    condition completions for `space_add`, `space_rm`, and `wait_until_stable`
  - `cargo test -p hyperdex-admin-protocol`, `cargo test -p server`, and
    `cargo test --workspace` all passed
- Conclusion: the packed-space and request-core gap is closed. The remaining
  live admin gap is now the session layer: bootstrap, Replicant persistent
  condition follows, and HyperDex configuration-condition encoding.
- Disposition: `advance`
- Next move: implement the minimal live BusyBee/Replicant coordinator session
  around this new request core, starting with bootstrap and condition-follow
  responses.

### Entry `hyh-020` - Preregistration

- Timestamp: `2026-03-27 05:39Z`
- Kind: `preregister`
- Hypothesis: reconciling the completed `admin-server` worktree result will put
  the missing BusyBee/Replicant session core on `main`, leaving startup wiring
  and payload fidelity as the next live gap rather than session semantics.
- Owner: root reconciliation of the completed `admin-server` worktree
- Start commit: `063d8a1`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/admin-server`
- Mutable surface:
  - `crates/server/src/lib.rs`
  - `crates/hyperdex-admin-protocol/**` only for small integration fixes
- Validator:
  - `cargo test -p server`
  - `cargo test --workspace`
- Expected artifacts:
  - `CoordinatorAdminLegacyService` on `main`
  - focused session-core tests on `main`
  - clean proof that the request core and session core coexist

### Entry `hyh-020` - Outcome

- Timestamp: `2026-03-27 05:48Z`
- Kind: `outcome`
- End commit: `f26d042`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/lib.rs`
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/hyperdex-admin-protocol/src/lib.rs`
- Evidence summary:
  - `78162d5` lands `CoordinatorAdminLegacyService`, per-connection pending
    completion state, BusyBee-framed bootstrap handling, and focused server
    tests for bootstrap, `space_add`, and `wait_until_stable`
  - the interrupted cherry-pick was resolved without losing the earlier
    request-core work on `main`
  - `f26d042` adds the missing `ReplicantAdminRequestMessage::config_follow()`
    helper required by the service-core tests
  - `cargo test -p server` passed
  - `cargo test --workspace` passed
- Conclusion: the session core is now on `main`. The next live gap is no
  longer request decoding or session mechanics in isolation; it is coordinator
  startup wiring plus original-format config-condition payloads.
- Disposition: `advance`
- Next move: run coordinator startup/probe implementation and selective
  decoder hardening in parallel.

### Entry `hyh-021` - Preregistration

- Timestamp: `2026-03-27 05:48Z`
- Kind: `preregister`
- Hypothesis: wiring the legacy admin service into coordinator startup and
  probing it with the original admin tools will expose the next live
  compatibility gap directly, and may already unblock `space_add` /
  `wait_until_stable`.
- Owner: delegated worker `019d2dd3-1489-7153-82b3-b6dc5d937157`
- Start commit: `f26d042`
- Worktree / branch:
  - delegated worker branch from `main`
- Mutable surface:
  - `crates/server/src/main.rs`
  - `crates/server/**` only for startup-facing glue and focused tests
  - probe helper scripts or notes only if strictly needed
- Validator:
  - focused server startup tests
  - bounded `hyperdex-add-space` probe
  - bounded `hyperdex-wait-until-stable` probe
  - `cargo test -p server`
- Expected artifacts:
  - coordinator process hosting the legacy admin listener
  - bounded live admin probe evidence
  - one commit ready for reconciliation

### Entry `hyh-022` - Preregistration

- Timestamp: `2026-03-27 05:48Z`
- Kind: `preregister`
- Hypothesis: selectively porting validation and tests from the richer
  `admin-decoder` worktree will harden packed-space decoding without colliding
  with the newer request-core API already on `main`.
- Owner: delegated worker `019d2dd5-c9cf-7371-b884-147367f7e897`
- Start commit: `f26d042`
- Worktree / branch:
  - delegated worker branch from `main`
- Mutable surface:
  - `crates/hyperdex-admin-protocol/**`
- Validator:
  - `cargo test -p hyperdex-admin-protocol`
  - `cargo test --workspace`
- Expected artifacts:
  - secret, partition, and index validation in packed-space decoding
  - richer truncation and fixture tests
  - one commit ready for reconciliation

### Entry `hyh-023` - Preregistration

- Timestamp: `2026-03-27 05:58Z`
- Kind: `preregister`
- Hypothesis: the startup/probe path now needs to solve one concrete process
  problem, not the whole legacy admin surface: the public coordinator port
  must serve the existing JSON control traffic and the legacy BusyBee/Replicant
  admin traffic on the same listener.
- Owner: delegated worker `019d2ddc-dcd8-7b61-b69f-4aa4b4bd2c2e`
- Start commit: `6bf04d5`
- Worktree / branch:
  - delegated worker branch from `main`
- Mutable surface:
  - `crates/server/src/main.rs`
  - `crates/server/src/lib.rs` only for minimal startup-facing glue
- Validator:
  - `cargo test -p server`
  - `cargo test --workspace`
  - bounded `hyperdex-add-space` probe
  - bounded `hyperdex-wait-until-stable` probe
- Expected artifacts:
  - same-port coordinator startup that reaches the legacy admin path
  - focused startup tests
  - bounded live probe evidence

### Entry `hyh-023` - Outcome

- Timestamp: `2026-03-27 06:02Z`
- Kind: `outcome`
- End commit: `99d3922`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/main.rs`
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/lib.rs`
- Evidence summary:
  - `99d3922` binds one public coordinator listener, accepts connections on
    that port, peeks the first bytes, and dispatches each connection to either
    the JSON control path or the legacy admin path
  - `99d3922` adds a focused test that keeps a legacy `config_follow`
    connection open while a JSON `space_add` request succeeds on the same port
  - `cargo test -p server` passed
  - `cargo test --workspace` passed
  - bounded `hyperdex-add-space` and `hyperdex-wait-until-stable` probes still
    timed out against the live listener afterward
- Conclusion: same-port startup is no longer the main live blocker. The next
  live gap is downstream of accept/dispatch, with the binary `config` payload
  path now the strongest remaining candidate.
- Disposition: `advance`
- Next move: finish the binary `config` follow payload encoder and rerun the
  bounded admin-tool probes.

### Entry `hyh-024` - Preregistration

- Timestamp: `2026-03-27 05:58Z`
- Kind: `preregister`
- Hypothesis: replacing the JSON `config` follow payload with the original
  packed `hyperdex::configuration` binary layout will remove the next concrete
  decoding failure for the C admin client without reopening the session-core
  work.
- Owner: delegated worker `019d2ddc-b498-70c3-98f3-fedc5f07521a`
- Start commit: `6bf04d5`
- Worktree / branch:
  - delegated worker branch from `main`
- Mutable surface:
  - `crates/server/src/lib.rs`
- Validator:
  - `cargo test -p server`
  - `cargo test --workspace`
- Expected artifacts:
  - binary `config` follow payload encoder
  - updated focused server tests that no longer assert JSON
  - one commit ready for reconciliation

### Entry `hyh-024` - Outcome

- Timestamp: `2026-03-27 06:02Z`
- Kind: `outcome`
- End commit: `4ec59fa`
- Artifact location:
  - no code changes from worker `019d2ddc-b498-70c3-98f3-fedc5f07521a`
- Evidence summary:
  - the first binary-config worker was interrupted before returning a code
    result
  - same-port startup landed in parallel as `99d3922`, which makes the binary
    `config` payload path more important rather than less
- Conclusion: keep the scope, replace the worker, and finish the encoder with
  the now-stable process-facing context.
- Disposition: `retry`
- Next move: preregister the replacement binary-config worker on the same
  narrow write scope.

### Entry `hyh-026` - Preregistration

- Timestamp: `2026-03-27 06:02Z`
- Kind: `preregister`
- Hypothesis: with same-port startup now on `main`, a replacement worker can
  finish the packed `hyperdex::configuration` encoder as the main remaining
  live admin blocker.
- Owner: delegated worker `019d2de2-abda-70e3-9c78-3b604c742be1`
- Start commit: `99d3922`
- Worktree / branch:
  - delegated worker branch from `main`
- Mutable surface:
  - `crates/server/src/lib.rs`
- Validator:
  - `cargo test -p server`
  - `cargo test --workspace`
  - bounded `hyperdex-add-space` probe if reasonable after the patch
- Expected artifacts:
  - binary `config` follow payload encoder
  - updated focused server tests
  - the next concrete live admin result

### Entry `hyh-026` - Outcome

- Timestamp: `2026-03-27 06:08Z`
- Kind: `outcome`
- End commit: `0d8d566`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/lib.rs`
- Evidence summary:
  - `0d8d566` replaces the JSON `config` follow payload with packed
    `hyperdex::configuration` bytes in `default_legacy_config_encoder`
  - focused server tests were updated for the binary payload shape
  - `cargo test -p server` passed
  - `cargo test --workspace` passed
  - the worker could not complete a bounded live probe on `127.0.0.1:1982`
    because the port was already in use
- Conclusion: the code-side `config` payload gap is closed. The next useful
  move is a fresh live probe on free ports against the full current stack.
- Disposition: `advance`
- Next move: preregister a probe-only worker against free local ports.

### Entry `hyh-027` - Preregistration

- Timestamp: `2026-03-27 06:08Z`
- Kind: `preregister`
- Hypothesis: with the request core, service core, same-port startup, and
  binary `config` payload now all on `main`, a fresh live probe on free ports
  will either unblock the original admin tools or expose the next concrete
  failing surface for `hyhac`.
- Owner: delegated worker `019d2de8-b84d-75c0-a12f-7d638e84e239`
- Start commit: `0d8d566`
- Worktree / branch:
  - delegated worker branch from `main`
- Mutable surface:
  - none by default; probe-only unless a tiny helper becomes strictly
    necessary
- Validator:
  - bounded `hyperdex-add-space` probe on free local ports
  - bounded `hyperdex-wait-until-stable` probe on free local ports
  - direct `hyhac` Cabal test if those probes advance
- Expected artifacts:
  - exact probe commands and outcomes
  - the next concrete failing surface, if any

### Entry `hyh-027` - Outcome

- Timestamp: `2026-03-27 06:19Z`
- Kind: `outcome`
- End commit: `b4a0648`
- Artifact location:
  - live probe output from worker `019d2df0-9cbb-7720-81f9-d3a957bb178b`
- Evidence summary:
  - the probe selected free ports `coord=19830 daemon=20120 control=30120`
  - `cargo build -p server --bin server` succeeded
  - the coordinator started and stayed alive on `127.0.0.1:19830`
  - the daemon exited immediately on startup with `Error: early eof`
  - because the daemon failed early, `hyperdex-add-space`,
    `hyperdex-wait-until-stable`, and the direct `hyhac` run were not reached
- Conclusion: the next concrete live blocker is daemon registration through the
  public coordinator port. The next step should fix that runtime path before
  spending more time on admin-tool probes.
- Disposition: `advance`
- Next move: preregister a bounded daemon-registration/public-port fix.

### Entry `hyh-028` - Preregistration

- Timestamp: `2026-03-27 06:19Z`
- Kind: `preregister`
- Hypothesis: same-port public coordinator dispatch now serves legacy admin
  and JSON control traffic, but daemon registration still trips the wrong
  branch or response shape; fixing that path will let the daemon join and
  unblock the original admin-tool probes.
- Owner: delegated implementation worker to be launched from `main`
- Start commit: `b4a0648`
- Worktree / branch:
  - delegated worker branch from `main`
- Mutable surface:
  - `crates/server/src/main.rs`
  - `crates/server/src/lib.rs`
- Validator:
  - `cargo test -p server`
  - `cargo test --workspace`
  - free-port coordinator+daemon startup
  - bounded `hyperdex-add-space` and `hyperdex-wait-until-stable` probes if
    daemon startup succeeds
- Expected artifacts:
  - daemon registration that works through the public coordinator port
  - the next concrete live admin result

### Entry `hyh-028` - Outcome

- Timestamp: `2026-03-27 06:24Z`
- Kind: `outcome`
- End commit: `0664ade`
- Artifact location:
  - live probe output from worker `019d2df3-86d2-7100-8406-ac46491c1be8`
- Evidence summary:
  - `cargo test -p server` passed
  - `cargo test --workspace` passed
  - free-port coordinator startup on `19830` succeeded
  - free-port daemon startup on `20120` / `30120` also succeeded
  - `hyperdex-add-space` timed out with exit `124`
  - `hyperdex-wait-until-stable` timed out with exit `124`
- Conclusion: daemon registration through the public coordinator port is not
  the stable blocker on clean `main`. The next blocker is again the original
  admin tools timing out after the cluster is fully up.
- Disposition: `reframe`
- Next move: preregister a bounded admin-timeout investigation on the live
  free-port cluster.

### Entry `hyh-029` - Preregistration

- Timestamp: `2026-03-27 06:24Z`
- Kind: `preregister`
- Hypothesis: with the live cluster now confirmed up on free ports, the next
  missing contract can be isolated by running the original admin tools against
  that cluster and capturing the first concrete timeout surface or missing
  response path in the coordinator logs and protocol behavior.
- Owner: delegated implementation/probe worker to be launched from `main`
- Start commit: `0664ade`
- Worktree / branch:
  - delegated worker branch from `main`
- Mutable surface:
  - probe-only by default
  - `crates/server/**` only if a tiny logging or compatibility fix becomes
    necessary from observed evidence
- Validator:
  - free-port coordinator+daemon startup
  - bounded `hyperdex-add-space` probe
  - bounded `hyperdex-wait-until-stable` probe
  - direct `hyhac` Cabal test if those probes advance
- Expected artifacts:
  - exact first failing admin-tool surface after successful daemon join
  - the next bounded compatibility target

### Entry `hyh-029` - Outcome

- Timestamp: `2026-03-27 06:29Z`
- Kind: `outcome`
- End commit: `46fbb36`
- Artifact location:
  - `/tmp/hyh-029.WZxvnF/coordinator.log`
  - `/tmp/hyh-029.WZxvnF/daemon.log`
  - `/tmp/hyh-029.WZxvnF/admin-proxy.log`
- Evidence summary:
  - `cargo test -p server -- --nocapture` passed on clean `main`
  - a free-port cluster started successfully with:
    - coordinator public port `45035`
    - daemon legacy frontend `44855`
    - daemon gRPC control `45335`
  - both `hyperdex-add-space` and `hyperdex-wait-until-stable` timed out
    with exit `124`
  - the captured wire shows the client sends only the 25-byte Replicant
    bootstrap request, receives one 88-byte Rust completion response, and then
    sends no second request before timing out
- Conclusion: the timeout happens before `space_add` or `wait_until_stable` is
  ever issued. The next bounded compatibility target is the exact mismatch in
  the first `config` follow completion after bootstrap.
- Disposition: `advance`
- Next move: preregister the `config`-follow completion compatibility fix.

### Entry `hyh-030` - Preregistration

- Timestamp: `2026-03-27 06:29Z`
- Kind: `preregister`
- Hypothesis: the original admin client expects a different first `config`
  follow completion shape than the current 88-byte Rust response. Narrowing and
  fixing that exact wire-level mismatch will let the client progress to
  `space_add` / `wait_until-stable`.
- Owner: delegated implementation worker to be launched from `main`
- Start commit: `46fbb36`
- Worktree / branch:
  - delegated worker branch from `main`
- Mutable surface:
  - `crates/server/src/lib.rs`
  - `crates/server/src/main.rs` only if a tiny adjunct is strictly necessary
- Validator:
  - `cargo test -p server`
  - `cargo test --workspace`
  - free-port admin-tool probe with captured wire
- Expected artifacts:
  - exact fix for the first `config` follow completion mismatch
  - a live probe that progresses beyond bootstrap, or the next narrower wire
    mismatch

### Entry `hyh-030` - Outcome

- Timestamp: `2026-03-27 06:35Z`
- Kind: `outcome`
- End commit: `afcfcd8`
- Artifact location:
  - `/tmp/hyh-029.WZxvnF/admin-proxy.log`
  - `/home/friel/HyperDex/Replicant/client/client.cc`
  - `/home/friel/c/aaronfriel/HyperDex/admin/admin.cc`
- Evidence summary:
  - the captured 88-byte Rust reply decodes as a BusyBee-framed
    `REPLNET_CLIENT_RESPONSE` with a success status and 64-byte data payload
  - `/home/friel/HyperDex/Replicant/client/client.cc` expects the first
    successful reply on this path to be `REPLNET_BOOTSTRAP`, not a client
    response
  - `/home/friel/c/aaronfriel/HyperDex/admin/admin.cc` only issues
    `replicant_client_cond_follow(..., "hyperdex", "config", ...)` after that
    Replicant bootstrap state exists
  - the admin client therefore never advances to a second request, so the
    current blocker sits before higher-level HyperDex `config` payload
    compatibility
- Conclusion: the next compatibility target is the exact first Replicant
  bootstrap response from the coordinator, not the later `config`-follow
  completion.
- Disposition: `advance`
- Next move: preregister a bounded implementation step that makes the
  coordinator emit the correct Replicant bootstrap frame.

### Entry `hyh-031` - Preregistration

- Timestamp: `2026-03-27 06:35Z`
- Kind: `preregister`
- Hypothesis: if the coordinator answers the initial 25-byte bootstrap request
  with a proper Replicant `REPLNET_BOOTSTRAP` frame carrying the expected
  bootstrap payload, the original admin client will progress past bootstrap
  and expose the next concrete compatibility gap, or reach `space_add` /
  `wait_until_stable` successfully.
- Owner: delegated implementation worker to be launched from `main`
- Start commit: `afcfcd8`
- Worktree / branch:
  - delegated worker branch from `main`
- Mutable surface:
  - `crates/server/src/lib.rs`
  - `crates/hyperdex-admin-protocol/**` only if small codec support is
    strictly necessary
- Validator:
  - `cargo test -p server`
  - `cargo test --workspace`
  - free-port admin-tool probe with captured wire
- Expected artifacts:
  - exact fix for the first Replicant bootstrap response
  - a live probe that sends a second client request after bootstrap, or the
    next narrower wire mismatch

### Entry `hyh-031` - Outcome

- Timestamp: `2026-03-27 07:10Z`
- Kind: `outcome`
- End commit: `4ccf113`
- Artifact location:
  - root coordination only; no code diff from a worker-owned bootstrap fix
- Evidence summary:
  - the originally intended implementation worker was closed by user request
    before any product-code result was reconciled
  - the durable files now need a larger fork-owned step that carries the
    bootstrap fix through its own verification loop instead of another
    short-lived handoff
- Conclusion: the technical target is unchanged, but the execution shape
  should shift to a larger fork-owned step with explicit fast and strong
  validators.
- Disposition: `reframe`
- Next move: preregister a larger bootstrap-compatibility fork and launch it
  alongside a parallel fast admin-probe harness worker.

### Entry `hyh-032` - Preregistration

- Timestamp: `2026-03-27 07:10Z`
- Kind: `preregister`
- Hypothesis: a forked worker that owns the full bootstrap-compatibility step
  from product-code patching through focused tests and a bounded captured-wire
  admin probe will either make the C admin client send a second request or
  expose the next exact wire mismatch with stronger evidence than another
  narrow substep.
- Owner: forked worker in
  `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-bootstrap`
- Start commit: `4ccf113`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-bootstrap` on
    `live-hyhac-bootstrap`
- Mutable surface:
  - `crates/server/src/lib.rs`
  - `crates/server/src/main.rs` only if a small bootstrap-path adjunct is
    strictly necessary
  - `crates/hyperdex-admin-protocol/**` only if small codec support is
    strictly necessary
- Validator:
  - fastest useful check: focused server bootstrap tests
  - strong checks:
    - `cargo test -p server`
    - `cargo test --workspace`
    - free-port captured-wire admin probe with `hyperdex-add-space` and
      `hyperdex-wait-until-stable`
- Expected artifacts:
  - exact fix for the first Replicant bootstrap response
  - focused tests that keep the fast loop short
  - a live probe that sends a second client request after bootstrap, or the
    next narrower wire mismatch

### Entry `hyh-032` - Outcome

- Timestamp: `2026-03-27 07:35Z`
- Kind: `outcome`
- End commit: `c087f81`
- Artifact location:
  - `crates/hyperdex-admin-protocol/src/lib.rs`
  - `crates/server/src/lib.rs`
  - `crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - `legacy_admin_wait_until_stable_probe_reports_bootstrap_progress` now
    reports `advanced=true`
  - `legacy_admin_add_space_probe_completes_after_bootstrap_and_robust_call`
    passes
  - direct `hyperdex-show-config`, `hyperdex-wait-until-stable`, and
    `hyperdex-add-space` succeed against the live Rust coordinator and daemon
  - `cargo test -p server` passed after integration on `main`
- Conclusion: the coordinator bootstrap/admin barrier is removed. The next live
  blocker is the legacy daemon request/response path, where `hyhac` now fails
  with `ClientGarbage` once it reaches pooled roundtrips and richer client
  operations.
- Disposition: `advance`
- Next move: preregister a larger daemon-data-path worker and a parallel fast
  reproducer worker for the `ClientGarbage` path.

### Entry `hyh-033` - Preregistration

- Timestamp: `2026-03-27 07:35Z`
- Kind: `preregister`
- Hypothesis: a forked worker that owns the legacy daemon data-path
  compatibility step end to end can turn the new `ClientGarbage` failures into
  working `get`, `count`, and richer client operations, or at least return the
  next exact wire mismatch from that path.
- Owner: forked worker in
  `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-data-plane`
- Start commit: `c087f81`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-data-plane` on
    `live-hyhac-data-plane`
- Mutable surface:
  - `crates/legacy-protocol/**`
  - `crates/legacy-frontend/**`
  - `crates/hyperdex-client-protocol/**`
  - `crates/server/src/lib.rs`
  - `crates/server/tests/**` only when focused daemon-path tests are part of
    the worker's own loop
- Validator:
  - fastest useful check: the fastest focused client repro available on the
    branch
  - strong checks:
    - `cargo test -p server`
    - `cargo test --workspace`
    - selected `hyhac` command covering the first pooled roundtrip path
- Expected artifacts:
  - code and tests that move the legacy daemon path past the first
    `ClientGarbage` failure
  - a shorter branch-local loop than the full selected `hyhac` command
  - either broader passing client behavior or the next exact mismatch

### Entry `hyh-033` - Outcome

- Timestamp: `2026-03-27 20:04Z`
- Kind: `outcome`
- End commit: `8871797`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-data-plane`
- Evidence summary:
  - `cargo test -p legacy-frontend -- --nocapture` passed in the worktree
  - `cargo test -p server legacy_ -- --nocapture` passed for the direct
    legacy handler coverage in `crates/server/src/lib.rs`; the remaining
    failure under that filter is the unrelated multiprocess test
    `legacy_atomic_routes_numeric_update_to_remote_primary_process` with
    `Error: early eof`
  - a manual live cluster with the full `profiles` schema successfully added
    the space through `hyperdex-add-space`
  - the exact focused `hyhac` selection for `*Can store a large object*`
    still reproduced `Left ClientGarbage`
  - temporary instrumentation on `handle_legacy_request` for `ReqAtomic`
    never fired during that failing run, so the daemon did not receive the
    first atomic write on this path
- Conclusion: the remaining mismatch is earlier than the daemon request
  decoder and response path. The next exact target is the packed coordinator
  config and client-side request-preparation contract for the full `profiles`
  schema, especially container and map datatype encoding.
- Disposition: `reframe`
- Next move: preregister a new product-owned step on the same worktree for the
  packed coordinator config and client-side request-preparation contract, and
  give it the new harness evidence showing the first captured exchange is still
  on the coordinator connection.

### Entry `hyh-034` - Preregistration

- Timestamp: `2026-03-27 20:04Z`
- Kind: `preregister`
- Hypothesis: a forked worker that owns the packed coordinator config and
  client-side request-preparation contract for the full `profiles` schema can
  move the focused large-object path past the pre-daemon `ClientGarbage`
  failure, or return the next exact coordinator-side mismatch with code and
  validators.
- Owner: next forked worker in
  `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-data-plane`
- Start commit: `8871797`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-data-plane` on
    `live-hyhac-data-plane`
- Mutable surface:
  - `crates/legacy-protocol/**`
  - `crates/legacy-frontend/**`
  - `crates/hyperdex-admin-protocol/**`
  - `crates/hyperdex-client-protocol/**`
  - `crates/server/**`
  - `/home/friel/c/aaronfriel/hyhac/scripts/**` only if launcher wiring is
    strictly necessary for the focused probe
- Validator:
  - fastest useful check:
    - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_hits_clientgarbage_fast -- --nocapture`
  - strong checks:
    - `cargo test -p server`
    - `cargo test --workspace`
    - focused manual cluster probe for `*Can store a large object*`
- Expected artifacts:
  - code and tests that move the large-object path past the pre-daemon
    `ClientGarbage` failure
  - or a tighter coordinator-side mismatch tied to concrete packet or source
    evidence
  - a shorter branch-local loop than the broader selected `hyhac` command

### Entry `hyh-034` - Outcome

- Timestamp: `2026-03-27 20:16Z`
- Kind: `outcome`
- End commit: `be0cb38`
- Artifact location:
  - `crates/legacy-frontend/src/lib.rs`
  - `crates/legacy-protocol/src/lib.rs`
  - `crates/server/src/lib.rs`
- Evidence summary:
  - `default_legacy_config_encoder` now packs `space.name`, attribute names,
    and index extras as HyperDex-style varint slices instead of fixed-width
    `u32` lengths
  - local packed-config decoder and server tests now prove the full `profiles`
    schema survives that encoding with the expected legacy datatype codes,
    including list, set, and map forms
  - the legacy daemon and frontend tests are now aligned to the actual
    nonce-prefixed BusyBee body shape rather than the earlier simplified body
  - `cargo test -p legacy-frontend -- --nocapture` passed
  - `cargo test -p server legacy_config_encoder_preserves_profiles_attribute_names_and_types -- --nocapture` passed
  - `cargo test -p server replicant_config_get_maps_to_packed_condition_completion -- --nocapture` passed
  - `cargo test -p server coordinator_admin_legacy_service_space_add_triggers_follow_update -- --nocapture` passed
  - on integrated `main`, `cargo test --workspace` passed
  - the shared fast public loop still reproduces `Left ClientGarbage`
- Conclusion: the packed coordinator config contract advanced materially, and
  the original client now sees the correct legacy datatype codes across the
  full `profiles` schema. The next mismatch is deeper inside the packed
  `hyperdex::configuration` / `hyperdex::space` body after string-slice
  encoding, not in bootstrap and not in the daemon request path.
- Disposition: `advance`
- Next move: preregister the next product-owned step on the same worktree to
  compare and fix the remaining `configuration` / `space` body mismatch after
  string-slice encoding, with the fast large-object repro still as the public
  loop.

### Entry `hyh-035` - Preregistration

- Timestamp: `2026-03-27 20:16Z`
- Kind: `preregister`
- Hypothesis: a follow-up worker on the same `live-hyhac-data-plane` worktree
  can use the now-correct string-slice and datatype encoding as a base to find
  and fix the next mismatch inside the packed `hyperdex::configuration` /
  `hyperdex::space` body for the full `profiles` schema. That next mismatch is
  now concrete: replace singleton primary-subspace region bounds with the
  original contiguous `hyperdex::partition(...)` hash intervals so the client
  can route ordinary keys before it prepares the first atomic write.
- Owner: resumed forked worker in
  `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-data-plane`
- Start commit: `be0cb38`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-data-plane` on
    `live-hyhac-data-plane`
- Mutable surface:
  - `crates/legacy-protocol/**`
  - `crates/legacy-frontend/**`
  - `crates/hyperdex-admin-protocol/**`
  - `crates/hyperdex-client-protocol/**`
  - `crates/server/**`
  - `/home/friel/c/aaronfriel/hyhac/scripts/**` only if launcher wiring is
    strictly necessary for the focused probe
- Validator:
  - fastest useful check:
    - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_hits_clientgarbage_fast -- --nocapture`
  - strong checks:
    - `cargo test -p legacy-frontend -- --nocapture`
    - `cargo test -p server`
    - `cargo test --workspace`
    - focused manual cluster probe for `*Can store a large object*`
- Expected artifacts:
  - code and tests that move the focused large-object path past the
    primary-subspace region-bounds mismatch
  - or a tighter `configuration` / `space` body mismatch tied to concrete
    packet or source evidence
  - a shorter branch-local loop than the broader selected `hyhac` command

### Entry `hyh-035` - Outcome

- Timestamp: `2026-03-27 20:28Z`
- Kind: `outcome`
- End commit: `1d6093c`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/lib.rs`
- Evidence summary:
  - `1d6093c` replaces singleton primary-region bounds with the original
    HyperDex partition intervals in the legacy config encoder
  - `cargo test -p server legacy_partition_regions_cover_full_u64_space_for_single_dimension -- --nocapture`
    passed
  - `cargo test -p server coordinator_admin_legacy_service_space_add_triggers_follow_update -- --nocapture`
    passed
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_hits_clientgarbage_fast -- --nocapture`
    still reproduces `Left ClientGarbage`
- Conclusion: the primary-region interval mismatch was real and is now fixed
  on `main`, but it was not the last mismatch inside the packed
  `hyperdex::configuration` / `hyperdex::space` body for the focused
  large-object path.
- Disposition: `advance`
- Next move: preregister the next product-owned step on a clean worktree and
  keep the same fast public loop while identifying and fixing the next
  coordinator-side packed-config/body mismatch.

### Entry `hyh-036` - Preregistration

- Timestamp: `2026-03-27 20:28Z`
- Kind: `preregister`
- Hypothesis: a forked worker that owns the next packed
  `hyperdex::configuration` / `hyperdex::space` body mismatch after the region
  fix can move the focused large-object path past the remaining pre-daemon
  `ClientGarbage` failure, or return the next exact coordinator-side mismatch
  with code and validators.
- Owner: next forked worker in
  `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-config-body`
- Start commit: `1d6093c`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-config-body` on
    `live-hyhac-config-body`
- Mutable surface:
  - `crates/legacy-protocol/**`
  - `crates/legacy-frontend/**`
  - `crates/hyperdex-admin-protocol/**`
  - `crates/hyperdex-client-protocol/**`
  - `crates/server/**`
  - `/home/friel/c/aaronfriel/hyhac/scripts/**` only if launcher wiring is
    strictly necessary for the focused probe
- Validator:
  - fastest useful check:
    - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_hits_clientgarbage_fast -- --nocapture`
  - strong checks:
    - `cargo test -p server legacy_partition_regions_cover_full_u64_space_for_single_dimension -- --nocapture`
    - `cargo test -p server`
    - `cargo test --workspace`
    - focused manual cluster probe for `*Can store a large object*`
- Expected artifacts:
  - code and tests that move the focused large-object path past the next
    remaining packed-config/body mismatch after region intervals
  - or a tighter coordinator-side mismatch tied to concrete packet or source
    evidence
  - a shorter branch-local loop than the broader selected `hyhac` command

### Entry `hyh-036` - Outcome

- Timestamp: `2026-03-27 21:05Z`
- Kind: `outcome`
- End commit: `475f4eb`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/lib.rs`
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/main.rs`
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/legacy-frontend/src/lib.rs`
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/legacy-protocol/src/lib.rs`
- Evidence summary:
  - added upstream-style post-decode atomic validation with explicit
    `RespAtomic/BadDimensionSpec` replies
  - added focused tests for schema-mismatched `FUNC_SET` and erase-with-funcalls
  - integrated BusyBee identify handling, persistent legacy connections, config
    ID allocation fixes, map datatype fix, and daemon config-refresh retry
  - `cargo test -p server legacy_atomic_returns_bad_dim_spec_for_schema_mismatched_set -- --nocapture`
    passed
  - `cargo test -p server legacy_atomic_returns_bad_dim_spec_for_erase_with_funcalls -- --nocapture`
    passed
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_hits_clientgarbage_fast -- --nocapture`
    still reproduces `Left ClientGarbage`
  - `cargo test -p server --test dist_multiprocess_harness coordinator_space_add_reaches_multiple_daemon_processes -- --nocapture`
    fails with `Error: early eof`
- Conclusion: the missing atomic validation-and-explicit-error contract was
  real and is now implemented on `main`, but it did not clear the focused
  large-object failure. The next concrete failing surface is the multiprocess
  process-level `early eof` path.
- Disposition: `reframe`
- Next move: preregister a fresh product-owned step from current `main` on the
  multiprocess `early eof` path.

### Entry `hyh-037` - Preregistration

- Timestamp: `2026-03-27 21:05Z`
- Kind: `preregister`
- Hypothesis: a fresh forked worker starting from current `main` can fix the
  multiprocess `early eof` process-level path exposed after `hyh-036`, which is
  now the next concrete blocker for trustworthy live-cluster validation.
- Owner: next forked worker in
  `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-process-eof`
- Start commit: `acfdcdc`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-process-eof` on
    `live-hyhac-process-eof`
- Mutable surface:
  - `crates/server/**`
  - `crates/legacy-frontend/**`
  - `crates/legacy-protocol/**`
  - `crates/transport-grpc/**` only if strictly required by the failing
    process-level path
- Validator:
  - fastest useful check:
    - `cargo test -p server --test dist_multiprocess_harness coordinator_space_add_reaches_multiple_daemon_processes -- --nocapture`
  - strong checks:
    - `cargo test -p server --test dist_multiprocess_harness legacy_atomic_routes_numeric_update_to_remote_primary_process -- --nocapture`
    - `cargo test -p server --test dist_multiprocess_harness degraded_search_and_count_survive_one_daemon_process_shutdown -- --nocapture`
    - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_hits_clientgarbage_fast -- --nocapture`
    - `cargo test -p server`
    - `cargo test --workspace`
- Expected artifacts:
  - code and tests that remove the `early eof` multiprocess failures
  - a clean next live-cluster baseline for the remaining large-object failure
  - or a tighter process-level mismatch with code and evidence

### Entry `hyh-037` - Outcome

- Timestamp: `2026-03-27 21:20Z`
- Kind: `outcome`
- End commit: `9afb11a`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/lib.rs`
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - taught the legacy daemon frontend to accept both newer protocol bodies and
    older named-body requests on the public data path
  - added compatibility for named-body `ReqCount`, `ReqGet`, `ReqAtomic`, and
    `ReqSearchStart` plus named get/search response bodies
  - `cargo test -p server --test dist_multiprocess_harness coordinator_space_add_reaches_multiple_daemon_processes -- --nocapture`
    passed on integrated `main`
  - `cargo test -p server --test dist_multiprocess_harness legacy_atomic_routes_numeric_update_to_remote_primary_process -- --nocapture`
    passed on integrated `main`
  - `cargo test -p server --test dist_multiprocess_harness degraded_search_and_count_survive_one_daemon_process_shutdown -- --nocapture`
    passed on integrated `main`
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_hits_clientgarbage_fast -- --nocapture`
    still reproduces `Left ClientGarbage`
- Conclusion: the multiprocess `early eof` process-level failures are removed
  from `main`. The next active blocker is once again the focused large-object
  `ClientGarbage` failure, now on a cleaner live-cluster baseline.
- Disposition: `advance`
- Next move: preregister a fresh current-main product step for the large-object
  failure and relaunch a parallel read-only narrowing pass on that cleaner
  baseline.

### Entry `hyh-038` - Preregistration

- Timestamp: `2026-03-27 21:20Z`
- Kind: `preregister`
- Hypothesis: a fresh forked worker on current `main` can move the focused
  large-object `ClientGarbage` path forward now that the multiprocess `early
  eof` failures are gone, or reduce it to the next exact daemon-side mismatch
  with code and validators.
- Owner: next forked worker in
  `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-large-object`
- Start commit: `5879fab`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-large-object` on
    `live-hyhac-large-object`
- Mutable surface:
  - `crates/server/**`
  - `crates/legacy-frontend/**`
  - `crates/legacy-protocol/**`
  - `crates/transport-grpc/**` only if strictly required by the remaining live
    path
- Validator:
  - fastest useful check:
    - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_hits_clientgarbage_fast -- --nocapture`
  - strong checks:
    - `cargo test -p server --test dist_multiprocess_harness coordinator_space_add_reaches_multiple_daemon_processes -- --nocapture`
    - `cargo test -p server --test dist_multiprocess_harness legacy_atomic_routes_numeric_update_to_remote_primary_process -- --nocapture`
    - `cargo test -p server --test dist_multiprocess_harness degraded_search_and_count_survive_one_daemon_process_shutdown -- --nocapture`
    - `cargo test -p server`
    - `cargo test --workspace`
- Expected artifacts:
  - code and tests that move the focused large-object path past `Left ClientGarbage`
  - or the next exact remaining mismatch with code and evidence
  - a clean current-main branch-local loop for the remaining live failure

### Entry `hyh-038` - Outcome

- Timestamp: `2026-03-27 22:30Z`
- Kind: `outcome`
- End commit: `live-hyhac-large-object` worktree after `57a23a0`, `5a8eac4`, and the
  session-owned sender-id edits
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-large-object/crates/server/src/lib.rs`
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-large-object/crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - the coordinator admin session now owns one sender id and reuses it across
    BusyBee identify, bootstrap `server.id`, and the bootstrap config server
    list
  - `coordinator_admin_legacy_service_bootstrap_sends_bootstrap_reply` now
    proves a non-anonymous identify request keeps one chosen sender id
    consistent across the identify reply and bootstrap body
  - `legacy_bootstrap_response_matches_replicant_sender_identity_contract`
    encodes the original Replicant acceptance rule directly and passes
  - `legacy_hyhac_large_object_probe_reports_coordinator_busybee_sequence`
    still shows only identify plus bootstrap traffic on the coordinator
    connection, with no non-bootstrap Replicant message from Hyhac
  - `legacy_hyhac_large_object_probe_hits_clientgarbage_fast` still reports
    `Left ClientGarbage`
  - `cargo test -p server` passed
- Conclusion: the wire-visible sender-id mismatch is narrowed and the
  coordinator is now internally consistent on that contract, but the focused
  Hyhac path still fails before follow/config. The next exact target is the
  non-wire bootstrap acceptance behavior on the original Replicant client side,
  not the daemon path.
- Disposition: `reframe`
- Next move: compare the original Replicant client's anonymous-channel
  bootstrap acceptance against the handcrafted Rust BusyBee session behavior
  and isolate the next remaining acceptance mismatch.

### Entry `hyh-039` - Preregistration

- Timestamp: `2026-03-27 22:35Z`
- Kind: `preregister`
- Hypothesis: after the sender-id plumbing fix, the remaining focused Hyhac
  failure is the next non-wire bootstrap acceptance mismatch between the
  original Replicant client and the handcrafted Rust BusyBee session behavior.
- Owner: next forked product worker in
  `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-large-object`
- Start commit: `19fc81f`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-large-object`
    on `live-hyhac-large-object`
- Mutable surface:
  - `crates/server/**`
  - `crates/hyperdex-admin-protocol/**`
  - `crates/server/tests/**`
  - `/home/friel/c/aaronfriel/hyhac/scripts/**` only if a tiny focused helper
    is strictly necessary
- Validator:
  - fastest useful check:
    `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reports_coordinator_busybee_sequence -- --nocapture`
  - strong checks:
    - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_hits_clientgarbage_fast -- --nocapture`
    - `cargo test -p server`
- Expected artifacts:
  - code or an exact reduction of the next non-wire bootstrap acceptance
    mismatch
  - focused validator evidence that the client either leaves bootstrap or that
  the next exact acceptance rule is isolated

### Entry `hyh-039` - Outcome

- Timestamp: `2026-03-27 23:05Z`
- Kind: `outcome`
- End commit: `working tree on main after the repeated-identify fix and probe corrections`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/lib.rs`
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - `cce-013` identified repeated server identify handling on an already-
    identified anonymous BusyBee channel as the next exact mismatch after
    sender-id consistency
  - `CoordinatorAdminSession` now tracks whether the legacy admin channel is
    already identified and treats later identify frames as validation-only
    instead of replying again
  - `coordinator_admin_legacy_service_repeated_identify_is_validate_only`
    proves the Rust coordinator now emits only one identify reply for the
    anonymous-to-identified transition
  - `legacy_hyhac_large_object_probe_reports_coordinator_busybee_sequence`
    now captures non-bootstrap `CondWait` requests and `ClientResponse`
    completions on the coordinator connection once the bootstrap address is
    forced through the proxy
  - `legacy_hyhac_large_object_probe_hits_clientgarbage_fast` still reports
    `Left ClientGarbage`
  - `cargo test -p server` passed
- Conclusion: the remaining large-object failure is no longer a bootstrap
  acceptance problem. The coordinator path now advances through follow traffic,
  so the next exact target is the first post-follow mismatch on the corrected
  baseline, most likely at the daemon path or the remaining coordinator state
  consumed just before the daemon request.
- Disposition: `advance`
- Next move: capture the first daemon-side request/response or reduce the
  remaining post-follow mismatch exactly enough to explain the still-failing
  direct Hyhac loop.

### Entry `hyh-040` - Preregistration

- Timestamp: `2026-03-27 23:05Z`
- Kind: `preregister`
- Hypothesis: now that the corrected proxy proves the focused Hyhac path
  advances beyond bootstrap into coordinator `CondWait` traffic, the remaining
  large-object `ClientGarbage` failure can be reduced by capturing the first
  daemon-side request/response or by isolating the exact post-follow mismatch
  that still prevents that request from succeeding.
- Owner: delegated worker `019d316c-56ed-7b83-ade6-f5f83c32c7d9` (`Pauli`)
- Start commit: `64104e7`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-post-follow`
    on `live-hyhac-post-follow`
- Mutable surface:
  - `crates/server/**`
  - `crates/server/tests/**`
  - `crates/legacy-frontend/**` only if a tiny focused probe helper is needed
- Validator:
  - fastest useful check:
    `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reports_coordinator_busybee_sequence -- --nocapture`
  - strong checks:
    - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_hits_clientgarbage_fast -- --nocapture`
    - `cargo test -p server`
- Expected artifacts:
  - the first daemon-side request/response evidence on the corrected baseline,
    or the next exact post-follow mismatch
  - focused tests or probes that lock in the new reduction

### Entry `hyh-040` - Outcome

- Timestamp: `2026-03-27 23:28Z`
- Kind: `outcome`
- End commit: `2fb432c`
- Artifact location:
  - no code changes in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-post-follow`
- Evidence summary:
  - the delegated product fork returned only a root-status restatement
  - the fresh post-follow worktree remained clean
  - no probe result, code change, or new reduction was produced
- Conclusion: the problem is the execution shape of the relaunch, not the
  product surface. The next product worker must be told explicitly to do repo
  work or return one precise blocker tied to current code and the corrected
  post-follow probe.
- Disposition: `retry`
- Next move: preregister a stricter product relaunch on the same clean
  worktree while the read-only comparison continues.

### Entry `hyh-041` - Preregistration

- Timestamp: `2026-03-27 23:28Z`
- Kind: `preregister`
- Hypothesis: a stricter relaunch that forbids root-status narration and
  requires either repo work or one precise blocker will move the corrected
  post-follow large-object failure forward on the clean `live-hyhac-post-follow`
  worktree.
- Owner: delegated worker `019d316f-5ffb-7f62-885c-e7eddcc6345f` (`Confucius`)
- Start commit: `2fb432c`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-post-follow`
    on `live-hyhac-post-follow`
- Mutable surface:
  - `crates/server/**`
  - `crates/server/tests/**`
  - `crates/legacy-frontend/**` only if a small focused helper is needed
- Validator:
  - fastest useful check:
    `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reports_coordinator_busybee_sequence -- --nocapture`
  - strong checks:
    - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_hits_clientgarbage_fast -- --nocapture`
    - `cargo test -p server`
- Expected artifacts:
  - a commit that materially advances the remaining large-object failure, or
    one precise blocker tied to current code and the corrected post-follow
    probe
  - focused validator evidence for the new reduction or fix

### Entry `hyh-041` - Outcome

- Timestamp: `2026-03-27 23:41Z`
- Kind: `outcome`
- End commit: `5e2224a`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - `5e2224a` adds a focused daemon-capture harness for the failing large-object
    Hyhac subset and clears startup readiness probe noise before asserting on
    daemon traffic
  - `legacy_hyhac_large_object_probe_reports_no_daemon_traffic_after_startup`
    passes on integrated `main` and proves the daemon capture is empty while
    the large-object subset still returns `Left ClientGarbage`
  - `legacy_hyhac_large_object_probe_hits_clientgarbage_fast` still reproduces
    the same public failure
  - `cargo test -p server` passes on integrated `main`
- Conclusion: the corrected post-follow failure is still before the first
  daemon legacy request. The next product target must remain on the
  coordinator-side post-follow behavior rather than daemon request handling.
- Disposition: `advance`
- Next move: wait for `cce-015` to name the exact remaining coordinator-side
  mismatch, then relaunch product work on that narrower target.

### Entry `hyh-042` - Preregistration

- Timestamp: `2026-03-27 23:46Z`
- Kind: `preregister`
- Hypothesis: a product pass that verifies the native HyperDex client path
  against the same Rust cluster and compares it with Hyhac’s deferred
  handle/completion path will expose the remaining public contract mismatch
  before the first daemon request, and that differential will either produce a
  fix or one exact blocker.
- Owner: delegated worker `019d3180-ba84-7443-9a62-8de2faaeb9c1` (`Parfit`)
- Start commit: `a618ea0`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-post-follow`
    on `live-hyhac-post-follow`
- Mutable surface:
  - `crates/server/**`
  - `crates/server/tests/**`
  - `/home/friel/c/aaronfriel/hyhac/**` only if a tiny focused probe helper is
    strictly necessary and does not change Hyhac semantics
- Validator:
  - fastest useful checks:
    - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reports_no_daemon_traffic_after_startup -- --nocapture`
    - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_hits_clientgarbage_fast -- --nocapture`
  - strong checks:
    - `cargo test -p server`
    - any tighter differential probe added for native client versus Hyhac
- Expected artifacts:
  - a verified differential between native HyperDex client behavior and Hyhac’s
    deferred-handle path against the same live Rust cluster
  - a commit that fixes the remaining public contract or one exact blocker

### Entry `hyh-042` - Outcome

- Timestamp: `2026-03-27 23:58Z`
- Kind: `outcome`
- End commit: `eb6d093`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - `eb6d093` adds a tighter differential probe that captures the immediate
    `hyperdex_client_put` handle and status from both Hyhac and a native C
    client against the same live Rust cluster
  - both paths report the same immediate failure on the old fast validator:
    `put handle=-1 status=8512`, followed by `loop handle=-1 status=8523`
  - the new probe shows the old fast path was missing the `profiles` space
    prerequisite, so it was proving `UnknownSpace` rather than the later bug
  - `legacy_hyhac_large_object_probe_reports_immediate_unknownspace_before_deferred_loop`,
    `legacy_hyhac_large_object_probe_reports_no_daemon_traffic_after_startup`,
    and `cargo test -p server` all pass on integrated `main`
- Conclusion: the old fast large-object validator is flawed. The next product
  step must preserve schema setup and then capture the real later failure.
- Disposition: `reframe`
- Next move: launch a schema-created fast path and a matching read-only setup
  map under the same live-hyhac workstream.

### Entry `hyh-043` - Preregistration

- Timestamp: `2026-03-27 23:49Z`
- Kind: `preregister`
- Hypothesis: a parallel read-only pass over the original HyperDex client loop
  and Hyhac’s `clientDeferred` / `wrapDeferred` / `demandHandle` path will map
  the exact handle/completion contract the product worker must satisfy or
  probe, without waiting for another broad implementation attempt.
- Owner: delegated worker `019d3180-bc6b-7800-aa9e-552c9c2c1853` (`Laplace`)
- Start commit: `a618ea0`
- Worktree / branch:
  - none required; read-only evidence gathering only
- Mutable surface:
  - none
- Validator:
  - concrete mapping between original HyperDex client handle lifecycle and
    Hyhac’s deferred wrapper path on the failing large-object operation
  - exact file/function pointers for the first point where Hyhac can surface
    `ClientGarbage` before any daemon request is emitted
- Expected artifacts:
  - a precise handle/completion map for native client versus Hyhac
  - a tighter next target for `hyh-042`

### Entry `hyh-043` - Outcome

- Timestamp: `2026-03-27 23:58Z`
- Kind: `outcome`
- End commit: `eb6d093`
- Artifact location:
  - `/home/friel/c/aaronfriel/HyperDex/client/c.cc`
  - `/home/friel/c/aaronfriel/HyperDex/client/client.cc`
  - `/home/friel/c/aaronfriel/hyhac/src/Database/HyperDex/Internal/Ffi/Client.chs`
  - `/home/friel/c/aaronfriel/hyhac/src/Database/HyperDex/Internal/Core.hs`
  - `/home/friel/c/aaronfriel/hyhac/src/Database/HyperDex/Internal/Handle.hs`
- Evidence summary:
  - the original HyperDex client returns a negative immediate handle when
    `hyperdex_client_put` fails before queueing any request
  - Hyhac stores that negative handle as deferred work and later rewrites the
    resulting `NonePending` loop result into `ClientGarbage`
  - this exactly matches the integrated no-daemon-traffic probe once the new
    immediate-handle differential is added
- Conclusion: the first exact point where Hyhac can surface `ClientGarbage`
  before any daemon request is a negative immediate handle, not a later daemon
  reply. The old fast validator hit that path because `profiles` had never
  been created there.
- Disposition: `advance`
- Next move: use a schema-created baseline for the next live-hyhac product
  pass and keep the focus on the later failure after setup.

### Entry `hyh-044` - Preregistration

- Timestamp: `2026-03-27 23:58Z`
- Kind: `preregister`
- Hypothesis: a new product pass that preserves the `profiles` space setup
  before running the focused large-object subset will expose the real later
  failure and replace the flawed old fast validator.
- Owner: delegated worker `019d318e-8775-74a0-864e-a67fdd23eb49` (`Fermat`)
- Start commit: `eb6d093`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-post-follow`
    on `live-hyhac-post-follow`
- Mutable surface:
  - `crates/server/**`
  - `crates/server/tests/**`
  - `/home/friel/c/aaronfriel/hyhac/**` only if a tiny focused probe helper is
    strictly necessary and does not change Hyhac semantics
- Validator:
  - current flaw proof:
    `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reports_immediate_unknownspace_before_deferred_loop -- --nocapture`
  - target replacement:
    a new schema-created focused probe on the same large-object subset
  - strong check:
    `cargo test -p server`
- Expected artifacts:
  - a schema-created focused large-object probe that reaches the real later
    failure
  - a commit that moves the live-hyhac path forward on that corrected baseline

### Entry `hyh-045` - Preregistration

- Timestamp: `2026-03-27 23:58Z`
- Kind: `preregister`
- Hypothesis: a parallel read-only pass over `hyhac` test ordering and setup
  code can pin down the smallest prerequisite sequence needed to make the
  focused large-object probe honest before the product worker rewires it.
- Owner: delegated worker `019d318e-895a-7462-9396-ddf6791d417a` (`Harvey`)
- Start commit: `eb6d093`
- Worktree / branch:
  - none required; read-only evidence gathering only
- Mutable surface:
  - none
- Validator:
  - exact source-backed description of the minimal `profiles` setup required
    before the large-object subset is meaningful
  - concrete file/function pointers in `hyhac` test code for that setup path
- Expected artifacts:
  - the smallest honest setup sequence for the focused large-object probe
  - concrete guidance for `hyh-044`

### Entry `hyh-045` - Outcome

- Timestamp: `2026-03-28 00:17Z`
- Kind: `outcome`
- End commit: `589ce4f`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyhac/test/Test/HyperDex/Space.hs`
  - `/home/friel/c/aaronfriel/hyhac/test/Test/HyperDex/Shared.hs`
  - `/home/friel/c/aaronfriel/hyhac/test/Test/HyperDex/Pool.hs`
- Evidence summary:
  - `defaultSpaceDesc` and `makeSpaceDesc` define the full 19-attribute
    `profiles` schema rather than the old two-column shortcut
  - the smallest honest prerequisite sequence is: create `profiles` with that
    full schema, wait until stable, then run only the selected large-object
    tests from `Shared.hs` and `Pool.hs`
  - using anything smaller only reproves the earlier `UnknownSpace` path and
    does not reach the later bug
- Conclusion: `hyh-044` must preserve the full `defaultSpaceDesc` setup before
  any focused large-object reduction is trustworthy.
- Disposition: `advance`
- Next move: land the full-schema probe on `main` and use it to capture the
  first later failure after setup.

### Entry `hyh-044` - Outcome

- Timestamp: `2026-03-28 00:17Z`
- Kind: `outcome`
- End commit: `589ce4f`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - `589ce4f` adds
    `legacy_hyhac_large_object_probe_reaches_daemon_after_full_profiles_setup`
  - that probe creates `profiles` with the full 19-attribute schema, waits
    until stable, runs a native C large-object write first, then runs the
    selected Hyhac large-object subset with client tracing
  - native C succeeds with `put handle=1 status=8575` followed by
    `loop handle=1 status=8448`
  - Hyhac also advances past setup and completes one successful
    `put handle=1 status=8575` plus `loop handle=1 status=8448`, then reaches
    a later second `put handle=1 status=8575` before timing out
  - the daemon capture is non-empty on the corrected baseline, so the bug is
    no longer the old `UnknownSpace` / no-daemon-traffic path
- Conclusion: the old fast validator is replaced. The remaining live blocker is
  a later post-success operation in the selected large-object subset, not
  missing schema setup.
- Disposition: `advance`
- Next move: preregister the next focused reduction around the second
  large-object operation after setup.

### Entry `hyh-046` - Preregistration

- Timestamp: `2026-03-28 00:17Z`
- Kind: `preregister`
- Hypothesis: splitting the corrected full-schema baseline into the smallest
  later-failure probes will isolate which selected large-object operation
  stalls after the first successful Hyhac round-trip and expose the exact
  daemon/client contract that still differs from HyperDex.
- Owner: delegated worker `019d319b-e6f9-72a3-87ad-58542da51ba1` (`Hume`)
- Start commit: `589ce4f`
- Worktree / branch:
  - root checkout on `main`
- Mutable surface:
  - `crates/server/**`
  - `crates/server/tests/**`
- Validator:
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reaches_daemon_after_full_profiles_setup -- --nocapture`
  - one or more tighter full-schema probes that isolate the next later failure
  - `cargo test -p server`
- Expected artifacts:
  - a narrower post-success large-object probe
  - either a Rust-side fix for the later stall or one precise blocker tied to
    the corrected baseline

### Entry `hyh-047` - Preregistration

- Timestamp: `2026-03-28 00:19Z`
- Kind: `preregister`
- Hypothesis: a parallel read-only pass over the selected large-object tests
  can identify the exact operation order after the first successful `put` plus
  `loop`, so the product worker can split the corrected baseline without
  guessing which post-success step is actually hanging.
- Owner: delegated worker `019d319b-e8a2-7fa1-a8bb-41d50791b9f2` (`Zeno`)
- Start commit: `589ce4f`
- Worktree / branch:
  - none required; read-only evidence gathering only
- Mutable surface:
  - none
- Validator:
  - exact source-backed ordering of the selected large-object operations in
    `Shared.hs` and `Pool.hs`
  - a concrete recommendation for the first narrower post-success probe to add
- Expected artifacts:
  - the exact next operation after the first successful large-object round-trip
  - concrete guidance for `hyh-046`

### Entry `hyh-025` - Preregistration

- Timestamp: `2026-03-27 05:58Z`
- Kind: `preregister`
- Hypothesis: a clean retry of the decoder-hardening step with strict file
  ownership will land the missing validation and tests without repeating the
  earlier drift across unrelated crates.
- Owner: delegated worker `019d2ddc-89a0-7000-af8f-8683597f4a89`
- Start commit: `6bf04d5`
- Worktree / branch:
  - delegated worker branch from `main`
- Mutable surface:
  - `crates/hyperdex-admin-protocol/**`
- Validator:
  - `cargo test -p hyperdex-admin-protocol`
  - `cargo test --workspace`
- Expected artifacts:
  - secret, partition, and index validation in packed-space decoding
  - richer truncation and fixture tests
  - one commit ready for reconciliation

### Entry `hyh-025` - Outcome

- Timestamp: `2026-03-27 05:58Z`
- Kind: `outcome`
- End commit: `007bdf1`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/hyperdex-admin-protocol/src/lib.rs`
- Evidence summary:
  - `007bdf1` restores secret-attribute validation for `__secret` and rejects
    secret key attributes
  - `007bdf1` enforces consistent partition counts across packed subspaces
  - `007bdf1` validates packed index types and packed index attribute
    references
  - `007bdf1` switches packed-space decoding to contextual truncation checks
  - `007bdf1` replaces the minimal fixture with richer coverage for maps,
    timestamps, secret attributes, indices, and rejection paths
  - `cargo test -p hyperdex-admin-protocol` passed
  - `cargo test --workspace` passed
- Conclusion: the packed-space decoder is no longer the main live blocker. The
  remaining live admin gap is now same-port coordinator startup plus original
  binary `config` follow payloads.
- Disposition: `advance`
- Next move: reconcile the startup and binary-config workers, then rerun the
  bounded admin-tool probes.

### Entry `hyh-046` - Outcome

- Timestamp: `2026-03-28 00:35Z`
- Kind: `outcome`
- End commit: `30227c3`
- Artifact location:
  - no reconciled code result
  - stale worktree state in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-post-follow`
- Evidence summary:
  - all subagents were stopped before the work completed
  - the old `live-hyhac-post-follow` worktree is not a clean basis for the
    next product pass because it carries an uncommitted harness edit
  - the root package was tightened to reduce planning churn and reset the next
    product pass around a fresh clean worktree
- Conclusion: the intended product direction still stands, but the old worker
  result is not trustworthy enough to continue from directly.
- Disposition: `reframe`
- Next move: relaunch the product step from a fresh current-main worktree with
  the same honest full-schema validator.

### Entry `hyh-047` - Outcome

- Timestamp: `2026-03-28 00:35Z`
- Kind: `outcome`
- End commit: `30227c3`
- Artifact location:
  - no reconciled code or durable evidence result
- Evidence summary:
  - the read-only step was interrupted by the user-directed pause before a
    durable outcome was integrated
  - the root plan no longer needs a default parallel read-only step because the
    next move is a product-owned fix on the honest live baseline
- Conclusion: the next useful work is a real product pass, not another
  operation-order note.
- Disposition: `stop`
- Next move: preregister the new product-owned pass and only reopen read-only
  comparison if the product step proves it is necessary.

### Entry `hyh-048` - Preregistration

- Timestamp: `2026-03-28 00:35Z`
- Kind: `preregister`
- Hypothesis: a product-owned pass on a fresh worktree can reduce the
  full-schema post-success Hyhac failure to the first later divergent
  operation, patch the responsible server or protocol behavior, and move the
  honest live baseline forward.
- Owner: delegated worker `019d31bc-e8da-7af3-b40a-bfa04fd8ec4b` (`Gauss`)
- Start commit: `ace4050`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-roundtrip-fix`
    on `live-hyhac-roundtrip-fix`
- Mutable surface:
  - `crates/legacy-protocol/**`
  - `crates/legacy-frontend/**`
  - `crates/hyperdex-client-protocol/**`
  - `crates/server/**`
  - `crates/server/tests/**`
  - `/home/friel/c/aaronfriel/hyhac/scripts/**` only if launcher wiring must
    point at `hyperdex-rs`
- Validator:
  - fastest useful check:
    `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reaches_daemon_after_full_profiles_setup -- --nocapture`
  - expected narrower truthful checks added by the worker if they materially
    shorten the loop
  - strong checks:
    - `cargo test -p server`
    - `cargo test --workspace`
- Expected artifacts:
  - at least one material code change in `crates/**`
  - a reduced truthful repro for the remaining later Hyhac failure, if needed
  - either a live-baseline improvement or one precise blocker tied to current
    code and observed output

### Entry `hyh-048` - Outcome

- Timestamp: `2026-03-28 00:48Z`
- Kind: `outcome`
- End commit: `3c72516`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/legacy-frontend/src/lib.rs`
- Evidence summary:
  - `3c72516` changes `LegacyFrontend::serve_forever_with` so each accepted
    client connection is handled in its own task instead of blocking accept
    until one long-lived client disconnects
  - `cargo test -p legacy-frontend serve_forever_with_accepts_second_connection_while_first_stays_open -- --nocapture`
    passed on integrated `main`
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reaches_daemon_after_full_profiles_setup -- --nocapture`
    passed on integrated `main`
  - `cargo test -p server` passed on integrated `main`
  - the honest full-schema Hyhac large-object probe now completes both pooled
    and shared writes successfully
- Conclusion: the first post-success large-object blocker is cleared. The next
  live-hyhac step is no longer to fix that boundary, but to identify the next
  failing Hyhac operation beyond it.
- Disposition: `advance`
- Next move: find the next truthful failing Hyhac operation after the now-
  passing full-schema large-object boundary, then launch the next product-owned
  fix pass from that observed failure.

### Entry `hyh-049` - Preregistration

- Timestamp: `2026-03-28 01:02Z`
- Kind: `preregister`
- Hypothesis: a product-owned pass on a fresh worktree can clear the first
  full-schema post-large-object pooled failure, where `roundtrip` returns
  `ClientReconfigure`, and move the honest live baseline forward again.
- Owner: delegated worker `019d31ce-097e-7e51-bc7d-03b86e2996f6` (`Descartes`)
  in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-get-reconfigure`
- Start commit: `94b13c5`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-get-reconfigure`
    on `live-hyhac-get-reconfigure`
- Mutable surface:
  - `crates/legacy-protocol/**`
  - `crates/legacy-frontend/**`
  - `crates/hyperdex-client-protocol/**`
  - `crates/server/**`
  - `crates/server/tests/**` only when a focused validator is needed for this
    exact failure
  - `/home/friel/c/aaronfriel/hyhac/scripts/**` only if launcher wiring must
    point at `hyperdex-rs`
- Validator:
  - fastest useful check:
    a focused truthful full-schema pooled repro for the first
    `ClientReconfigure` failure, if the supporting harness worker lands it
  - current honest check:
    a live full-schema `--select-tests='*pooled*'` run that shows
    `Can store a large object: [OK]` and then fails first at `roundtrip` with
    `ClientReconfigure`
  - strong checks:
    - `cargo test -p server`
    - `cargo test --workspace`
- Expected artifacts:
  - at least one material code change in `crates/**`
  - a real move forward on the full-schema pooled compatibility path
  - either a greener honest live baseline or one precise blocker tied to
    current code and observed output

### Entry `hyh-049` - Outcome

- Timestamp: `2026-03-28 01:18Z`
- Kind: `outcome`
- End commit: `b23458c`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/lib.rs`
- Evidence summary:
  - `b23458c` fills legacy default values for missing attributes when encoding
    sparse records back through the legacy `get` path
  - `cargo test -p server legacy_get_fills_defaults_for_sparse_record_attributes -- --nocapture`
    passed
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reaches_daemon_after_full_profiles_setup -- --nocapture`
    passed
  - `cargo test -p server` passed
  - `cargo test --workspace` passed
  - the honest live full-schema pooled run now reports:
    - `Can store a large object: [OK]`
    - `roundtrip: [OK, passed 100 tests]`
    - `conditional: [OK, passed 100 tests]`
  - the next visible pooled failures are later `search`, `count`, and several
    atomic operations
- Conclusion: one real cause of the pooled `ClientReconfigure` path is fixed.
  The live compatibility boundary moved forward without regressing the
  large-object guard.
- Disposition: `advance`
- Next move: launch the next product pass on the first remaining truthful
  pooled atomic failure, while keeping the active harness workstream focused on
  shortening that later boundary if it can.

### Entry `hyh-050` - Preregistration

- Timestamp: `2026-03-28 01:32Z`
- Kind: `preregister`
- Hypothesis: a product-owned pass on a fresh worktree can clear the first
  remaining truthful pooled atomic failure on the full-schema live path and
  move the honest compatibility boundary forward again.
- Owner: delegated worker `019d31dc-cffe-7840-83a8-73e01c839261` (`Archimedes`)
  in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-atomic-fix`
- Start commit: `1e12978`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-atomic-fix` on
    `live-hyhac-atomic-fix`
- Mutable surface:
  - `crates/legacy-protocol/**`
  - `crates/legacy-frontend/**`
  - `crates/hyperdex-client-protocol/**`
  - `crates/server/**`
  - `crates/server/tests/**` only when a focused validator is needed for this
    exact failure
  - `/home/friel/c/aaronfriel/hyhac/scripts/**` only if launcher wiring must
    point at `hyperdex-rs`
- Validator:
  - fastest useful check:
    the honest live full-schema pooled check on the real cluster, narrowed
    further if the worker can do so truthfully inside its scope
  - strong checks:
    - `cargo test -p server`
    - `cargo test --workspace`
- Expected artifacts:
  - at least one material code change in `crates/**`
  - a real move forward on the first remaining truthful pooled atomic failure
  - either a greener honest live baseline or one precise blocker tied to
    current code and observed output

### Entry `hyh-050` - Outcome

- Timestamp: `2026-03-28 00:58Z`
- Kind: `outcome`
- End commit: `83e6003`
- Artifact location:
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/src/lib.rs`
  - `/home/friel/c/aaronfriel/hyperdex-rs/crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - `83e6003` is on `main`
  - `cargo test -p server legacy_atomic_integer_div_and_mod_follow_hyperdex_signed_semantics -- --nocapture`
    passed
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_integer_div_probe_turns_green_after_full_profiles_setup -- --nocapture`
    passed
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_map_int_int_add_probe_fails_after_full_profiles_setup -- --nocapture`
    passed
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_pooled_probe_reaches_map_atomic_failure_after_integer_boundary -- --nocapture`
    passed
  - `cargo test -p server` passed
  - `cargo test --workspace` passed on the current tree
  - the honest pooled live boundary now stays green through integer and float
    atomic sections and fails next in map-valued atomic mutation with
    `ClientServererror`
- Conclusion: the first remaining pooled atomic failure moved forward
  materially. Integer `div` and `mod` are fixed, the harness is truthful
  again, and the next product target is map-valued atomic mutation.
- Disposition: `advance`
- Next move: split the next product ownership into numeric map mutation and
  string map mutation on clean worktrees from `83e6003`.

### Entry `hyh-051` - Preregistration

- Timestamp: `2026-03-28 00:58Z`
- Kind: `preregister`
- Hypothesis: one product-owned pass can move the live pooled path through the
  failing numeric map mutations by teaching the legacy atomic path to apply
  map-valued numeric and bitwise updates instead of rejecting them with
  `ClientServererror`.
- Owner: delegated worker `019d31f4-8bce-7b72-a250-9526b3b31743` (`Pascal`)
  in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/map-atomic-numeric`
- Start commit: `83e6003`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/map-atomic-numeric` on
    `map-atomic-numeric`
- Mutable surface:
  - `crates/server/**`
  - `crates/legacy-protocol/**` only if wire decoding must change for this
    exact map-mutation contract
  - `crates/server/tests/**` only for focused truthful validators
- Validator:
  - fastest useful check:
    `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_map_int_int_add_probe_fails_after_full_profiles_setup -- --nocapture`
  - strong checks:
    - `cargo test -p server`
    - `cargo test --workspace`
- Expected artifacts:
  - at least one material code change in `crates/**`
  - a greener live pooled boundary on numeric map mutation
  - either a focused next failing map-numeric operation or a clear exact
    blocker tied to current code and observed output

### Entry `hyh-052` - Preregistration

- Timestamp: `2026-03-28 00:58Z`
- Kind: `preregister`
- Hypothesis: one product-owned pass can move the live pooled path through the
  failing string-keyed or string-valued map mutations by teaching the legacy
  atomic path to apply map string prepend/append and other string-map updates
  instead of rejecting them with `ClientServererror`.
- Owner: delegated worker `019d31f4-8e23-7b81-8879-c089630de0dc` (`Franklin`)
  in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/map-atomic-string`
- Start commit: `83e6003`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/map-atomic-string` on
    `map-atomic-string`
- Mutable surface:
  - `crates/server/**`
  - `crates/legacy-protocol/**` only if wire decoding must change for this
    exact map-mutation contract
  - `crates/server/tests/**` only for focused truthful validators
- Validator:
  - fastest useful check:
    the current broad truthful pooled probe, narrowed further inside the
    worker only if it preserves the map-string failure honestly
  - strong checks:
    - `cargo test -p server`
    - `cargo test --workspace`
- Expected artifacts:
  - at least one material code change in `crates/**`
  - a greener live pooled boundary on string map mutation
  - either a focused next failing map-string operation or a clear exact
    blocker tied to current code and observed output

### Entry `hyh-051` - Outcome

- Timestamp: `2026-03-28 03:10Z`
- Kind: `outcome`
- End state:
  - integrated on `main` in the current root reconciliation pass
- Artifacts:
  - `77885e7` (`Advance legacy numeric map atomic compatibility`)
  - `crates/server/src/lib.rs`
  - `crates/server/tests/dist_multiprocess_harness.rs`
- Evidence:
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_map_int_int_add_probe_turns_green_after_full_profiles_setup -- --nocapture`
    passed
  - `cargo test -p server`
    passed
  - `cargo test --workspace`
    passed
  - the broader pooled live path now keeps the numeric map families green
- Conclusion: the legacy atomic path now applies map-valued numeric updates
  instead of rejecting them. The old numeric-map boundary is cleared.
- Disposition: `advance`
- Next move: keep the greener pooled boundary honest while reconciling the
  remaining string-map work and broader live acceptance proof.

### Entry `hyh-052` - Outcome

- Timestamp: `2026-03-28 03:10Z`
- Kind: `outcome`
- End state:
  - integrated on `main` in the current root reconciliation pass
- Artifacts:
  - `8a33f55` (`Support legacy map string atomic funcalls`)
  - `crates/server/src/lib.rs`
  - `crates/server/tests/dist_multiprocess_harness.rs`
- Evidence:
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_map_string_string_prepend_probe_turns_green_after_full_profiles_setup -- --nocapture`
    passed
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_map_int_string_prepend_probe_turns_green_after_numeric_map_boundary -- --nocapture`
    passed
  - `cargo test -p server`
    passed
  - `cargo test --workspace`
    passed
- Conclusion: the legacy atomic path now applies the string-map prepend and
  append families that Hyhac exercises. The old string-map boundary is
  cleared too.
- Disposition: `advance`
- Next move: keep the pooled live path green and replace the stale
  failure-oriented probes with an honest live acceptance proof.

### Entry `hyh-053` - Outcome

- Timestamp: `2026-03-28 03:10Z`
- Kind: `outcome`
- End state:
  - integrated on `main` in the current root reconciliation pass
- Artifacts:
  - `crates/server/tests/dist_multiprocess_harness.rs`
  - `legacy_hyhac_pooled_probe_turns_green_after_map_atomic_compatibility`
  - `legacy_hyhac_split_acceptance_suite_passes_live_cluster`
- Evidence:
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_pooled_probe_turns_green_after_map_atomic_compatibility -- --nocapture`
    passed
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_split_acceptance_suite_passes_live_cluster -- --nocapture`
    passed
  - `cargo test -p server`
    passed
  - `cargo test --workspace`
    passed
  - the split live Hyhac acceptance now proves:
    - `Can add a space`
    - `Can remove a space`
    - `*pooled*`
    - `*shared*`
    - `*CBString*`
- Conclusion: the single-daemon live Hyhac surface is green on a real
  `hyperdex-rs` cluster when exercised in the suite’s correct live phases.
- Disposition: `advance`
- Next move: broaden that public proof toward a more distributed live
  acceptance path or a reusable verifier without regressing the current green
  surface.

### Entry `hyh-054` - Preregistration

- Timestamp: `2026-03-28 03:18Z`
- Kind: `preregister`
- Hypothesis: one substantial pass in the multiprocess harness can move the
  live public proof beyond the split single-daemon cluster by proving the same
  Hyhac-facing surface against a real two-daemon cluster.
- Owner: delegated worker on
  `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-two-daemon`
- Start commit: `8db4d81`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-hyhac-two-daemon` on
    `live-hyhac-two-daemon`
- Mutable surface:
  - `crates/server/tests/**`
  - `crates/server/**` only if the live two-daemon proof exposes a real public
    compatibility bug
- Validator:
  - fastest useful check:
    `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_split_acceptance_suite_passes_live_cluster -- --nocapture`
  - strong checks:
    - a new focused two-daemon live Hyhac acceptance test
    - `cargo test -p server`
- Expected artifacts:
  - a substantive harness or product change that proves or clears the next
    live two-daemon public acceptance gap
  - either a green two-daemon Hyhac-facing proof or an exact next failing
    public boundary

### Entry `hyh-055` - Preregistration

- Timestamp: `2026-03-28 03:18Z`
- Kind: `preregister`
- Hypothesis: one substantial pass can package the current live Hyhac proof
  into a reusable repository-local verifier so the acceptance path is runnable
  without remembering cargo-test filters or ad hoc shell commands.
- Owner: delegated worker on
  `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-acceptance-script`
- Start commit: `8db4d81`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/live-acceptance-script` on
    `live-acceptance-script`
- Mutable surface:
  - `scripts/**`
  - `crates/server/tests/**` only if the script needs a tighter fast check to
    stay truthful
- Validator:
  - fastest useful check:
    the script itself on a bounded acceptance mode
  - strong checks:
    - the full script on the current green live acceptance path
    - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_split_acceptance_suite_passes_live_cluster -- --nocapture`
- Expected artifacts:
  - a reusable verifier under `scripts/**`
  - a documented command surface in the script usage itself
  - proof that the script reproduces the current green live acceptance result

### Entry `hyh-054` - Outcome

- Timestamp: `2026-03-28 03:27Z`
- Kind: `outcome`
- End state:
  - integrated on `main` in `281b8cb`
- Artifacts:
  - `281b8cb` (`Fix legacy two-daemon routing and add live Hyhac proof`)
  - `crates/server/tests/dist_multiprocess_harness.rs`
  - `crates/server/src/lib.rs`
- Evidence:
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_split_acceptance_suite_passes_two_daemon_live_cluster -- --nocapture`
    passed
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_split_acceptance_suite_passes_live_cluster -- --nocapture`
    passed
  - `cargo test -p server`
    passed
- Conclusion: the Hyhac-facing public surface is now proven on a real
  two-daemon cluster as well as on the earlier single-daemon split acceptance
  path.
- Disposition: `advance`
- Next move: keep both live proofs green and use them as the public baseline.

### Entry `hyh-055` - Outcome

- Timestamp: `2026-03-28 03:27Z`
- Kind: `outcome`
- End state:
  - integrated on `main` via the landed verifier content and root
    reconciliation at `40b5d4f`
- Artifacts:
  - `scripts/verify-live-acceptance.sh`
  - `40b5d4f` (`Add reusable live acceptance verifier`)
- Evidence:
  - `scripts/verify-live-acceptance.sh --quick`
    passed
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_split_acceptance_suite_passes_live_cluster -- --nocapture`
    passed
- Conclusion: the current green live acceptance path is now packaged as a
  repository-local verifier instead of only as cargo-test filters and ad hoc
  shell commands.
- Disposition: `advance`
- Next move: keep the verifier green and use it as the default public check for
  the current Hyhac-facing surface.
