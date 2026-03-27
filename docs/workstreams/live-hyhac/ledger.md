# Workstream Ledger: live-hyhac

### Entry `hyh-001` - Preregistration

- Timestamp: `2026-03-27 04:22Z`
- Kind: `preregister`
- Hypothesis: the first live `hyhac` failure against `hyperdex-rs` will appear
  in admin `create space` or `waitUntilStable`, before client traffic starts.
- Owner: root
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
- Owner: dedicated worker in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/dist-control-plane`
- Start commit: `faa6cb6`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/dist-control-plane`
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
  - no code changes in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/dist-control-plane`
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
- Owner: dedicated worker in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/dist-control-plane`
- Start commit: `cd0d58c`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/dist-control-plane`
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
  - no code changes in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/dist-control-plane`
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
    follow coordinator `config`
- Conclusion: the replacement frontend must satisfy the initial `config`
  follow before it can ever see operation-specific `space_add` or
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
