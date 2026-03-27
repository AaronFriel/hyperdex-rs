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
