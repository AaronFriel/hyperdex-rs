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
