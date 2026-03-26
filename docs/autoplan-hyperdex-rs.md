# AutoPlan: hyperdex-rs

## Stable Contract

### Goal

Create a new repository at `/home/friel/c/aaronfriel/hyperdex-rs` that reimplements
HyperDex in pure Rust, preserves separate control-plane and data-plane processes,
and can run the `hyhac` test suite against a live cluster.

### Scope

- Implement the HyperDex behavior required by `hyhac` for admin and client
  operations.
- Expose two public protocols:
  - a legacy bit-for-bit HyperDex-compatible frontend for existing clients
  - a modern gRPC frontend for new clients
- Keep consensus, placement, storage, and inter-node transport behind traits.
- Provide multiple viable implementations where the user requested them, managed
  through branches and worktrees once the workspace is bootstrapped.
- Use deterministic simulation testing with `turmoil`, `madsim`, and a property
  testing library once the exact library choice is confirmed.
- Read the HyperDex, Warp, and Consus papers and capture design notes without
  overextending the MVP.

### Non-Goals For The First Useful Cluster

- Reproducing HyperDex's internal C++ node-to-node wire protocol.
- Matching every historical HyperDex feature before `hyhac` compatibility exists.
- Copying HyperDex's internal library naming or C/C++ structure.

### Acceptance Evidence

- `cargo test --workspace` passes in the new repository.
- Deterministic simulation suites exist for placement, replication, and failure
  handling.
- A live `hyperdex-rs` cluster can satisfy the `hyhac` harness without patches to
  `hyhac`'s semantics.
- Paper notes exist and name MVP exclusions explicitly.

### Evaluation Mode

Deterministic

### Mutable Surface

- `/home/friel/c/aaronfriel/hyperdex-rs/**`
- Supporting launch scripts in `/home/friel/c/aaronfriel/hyhac/scripts/**` only
  when needed to point at `hyperdex-rs`
- Watchdog check-ins for this campaign must send an explicit parent-thread
  message on every run.

### Iteration Unit

One bounded design-or-implementation step with validation and a recorded verdict.

### Loop Budget

12 bounded iterations before mandatory review of strategy.

### Dispositions

- `advance`
- `retry`
- `reframe`
- `revert`
- `escalate`
- `stop`

### Pivot Rules

- If a compatibility assumption about `hyhac` proves false, stop and narrow the
  public surface from observed calls.
- If the Rust crate choice for a subsystem blocks progress, keep the trait stable
  and replace only the implementation crate.
- Keep public protocol compatibility separate from internode transport. The
  public side must provide both legacy HyperDex compatibility and a modern gRPC
  protocol, while internode transport stays free to evolve.

### Stop Conditions

- The live `hyhac` suite passes against `hyperdex-rs`.
- Or a hard blocker is proven and documented with evidence.

## Current Hypothesis

The fastest path is to treat `hyhac` compatibility as the external contract, build
the workspace around that contract, expose both legacy and gRPC public frontends,
and keep consensus, internode transport, placement, and storage pluggable behind
stable traits from the start.

## Milestones

1. Bootstrap the repository, workspace layout, and compatibility notes.
2. Prove the exact `hyhac` operation surface and error semantics we need to serve.
3. Land shared domain crates for schema, placement, storage, and protocol types.
4. Land a single-node but trait-correct control plane and data plane.
5. Add replicated control/data paths with at least one concrete consensus backend.
6. Add alternative consensus and transport implementations in dedicated worktrees.
7. Add deterministic simulation harnesses.
8. Run the `hyhac` suite against the live cluster and close semantic gaps.

## Progress

- Done: workspace baseline, trait-based runtime, schema parser, and dual public
  protocol requirement are all recorded and validated in the main branch.
- Done: dedicated `legacy-protocol` crate now owns the legacy HyperDex public
  message numbers, return codes, and request/response header layouts.
- Done: the `server` crate now exposes HyperDex-shaped `coordinator` and
  `daemon` process parsing so the binary can grow toward a real externally
  launchable cluster.
- Done: the main branch now has a real legacy TCP listener that accepts a
  HyperDex-shaped request header and returns a valid `CONFIGMISMATCH` response,
  with `daemon` mode starting that listener.
- Done: the gRPC public frontend now runs a real tonic server over
  `ClusterRuntime` for create-space, put, and get, with an end-to-end test.
- Done: `consensus-core` now has a feature-gated OmniPaxos backend on `main`,
  validated in both default and `omnipaxos`-enabled builds.
- Done: `placement-core` now has a deterministic hyperspace-style token-ring
  placement model on `main`, with explicit partition reporting and tests.
- Done: `consensus-core` now also has a feature-gated OpenRaft backend on
  `main`, validated in both default and `openraft`-enabled builds.
- Done: `server` now selects the consensus backend from `ClusterConfig` at
  runtime, and rejects feature-gated backends when the corresponding server
  feature is not compiled in.
- Done: `server` now also selects placement, storage, and internode transport
  from `ClusterConfig`, with working RocksDB-backed runtime coverage.
- Done: `daemon` startup now instantiates the configured runtime shape and the
  legacy frontend handles a real `REQ_COUNT` request over framed legacy
  messages, instead of only returning `CONFIGMISMATCH`.
- Done: the legacy frontend now handles real `REQ_GET` and a simplified
  `REQ_ATOMIC` path over the configured runtime, covering key-based reads plus
  basic write and delete flows.
- Done: the simplified legacy `REQ_ATOMIC` path now carries attribute checks
  and funcall-style mutation decoding for conditional puts plus scalar numeric
  atomic operations, and the memory engine no longer creates phantom records on
  failed conditional writes.
- Running: worktree lanes for gRPC frontend, hyperspace placement fidelity,
  OpenRaft scaffolding, and OmniPaxos scaffolding.
- Next: integrate the remaining completed worktree results back into `main`
  without letting the compatibility path fragment.

## Next Bounded Iteration

Implement the legacy search request flow that `hyhac` uses, starting with
`REQ_SEARCH_START` plus result streaming that can sit on top of the current
runtime search results without inventing a different public contract.

## Loop Ledger

| Iteration | Hypothesis | Action | Evidence | Verdict | Disposition | Next Move |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | A workspace-first bootstrap will unblock parallel implementation without forcing early subsystem choices. | Create the new repository, write the AutoPlan, pin the `hyhac` compatibility target, and initialize the crate graph. | Workspace root and campaign files exist, the crate graph is in place, and `cargo test --workspace` passes. | Confirmed. | advance | Commit the baseline and use it to seed alternative implementation worktrees. |
| 2 | A committed baseline plus dedicated worktrees will keep alternative consensus, transport, and placement work from colliding with the main compatibility path. | Commit the workspace, create the `consensus-openraft`, `consensus-omnipaxos`, `transport-grpc-impl`, and `placement-alt` worktrees, then add schema and paper notes that clarify the MVP boundary. | Commit `cdc633b` exists, `git worktree list --porcelain` shows four new worktrees, and the workspace still passes `cargo test --workspace`. | Confirmed. | advance | Land the first real admin and client service implementations in the main line. |
| 3 | The main branch needs a concrete admin/client runtime before network compatibility can be attempted. | Add HyperDex DSL parsing, implement `ClusterRuntime` over the trait-based control/data plane, and test admin create/list plus client put/get/count/delete-group behavior. | Commit `f2db73b` exists, `cargo test -p server` passes, and `cargo test --workspace` still passes with the runtime adapter in place. | Confirmed. | advance | Build the external startup and network compatibility path that `hyhac` can actually drive. |
| 4 | The right boundary is dual public protocols, not replacement client libraries. | Record the user decision that the cluster must expose both a bit-for-bit HyperDex-compatible public protocol and a modern gRPC protocol, and reflect that split in configuration and campaign docs. | `ClusterConfig` now distinguishes public protocols from internode transport, and the docs name the legacy-plus-gRPC frontend requirement explicitly. | Confirmed. | advance | Start implementing the legacy HyperDex-compatible frontend as the first real external boundary. |
| 5 | The legacy frontend needs a dedicated code home before sockets or server loops are added. | Add the `legacy-protocol` crate and replace its stub with the HyperDex public message numbers, return codes, and request/response header definitions plus round-trip tests. | `cargo test -p cluster-config -p legacy-protocol` passes, `cargo test --workspace` passes, and the main branch now contains a dedicated legacy protocol crate. | Confirmed. | advance | Build the first callable legacy frontend skeleton on top of those protocol definitions. |
| 6 | A HyperDex-shaped process interface should exist before the external listener is implemented so cluster launch semantics are stable while the network surface grows. | Add `coordinator` and `daemon` process parsing to the `server` crate, test both command forms, and revalidate the workspace. | `cargo test -p server` passes with the new CLI tests, and `cargo test --workspace` still passes after the `server` binary starts parsing process modes. | Confirmed. | advance | Implement the first legacy frontend listener and hang it off the `daemon` mode instead of leaving the protocol crate as definitions only. |
| 7 | Even before full request decoding exists, the main branch should expose a real legacy TCP listener so external compatibility work stops being abstract. | Add the `legacy-frontend` crate, implement a TCP accept path that reads a legacy request header and returns `CONFIGMISMATCH`, wire `daemon` mode to start that listener, and revalidate the workspace. | `cargo test -p legacy-protocol -p legacy-frontend -p server` passes, and `cargo test --workspace` still passes with the new listener crate and daemon wiring in place. | Confirmed. | advance | Integrate the completed gRPC worktree branch, then return to richer legacy request handling. |
| 8 | The gRPC public frontend is independent enough from the legacy listener that it can be integrated immediately as a second public surface without destabilizing the compatibility path. | Replace the placeholder `transport-grpc` crate with a tonic/prost public frontend over `ClusterRuntime`, add a generated protobuf schema plus an end-to-end server test, and revalidate the workspace. | `cargo test -p transport-grpc` passes with the new end-to-end test, and `cargo test --workspace` still passes after the gRPC public surface lands on `main`. | Confirmed. | advance | Land one of the feature-gated consensus backends next, starting with OmniPaxos because its change set is small and isolated. |
| 9 | The OmniPaxos backend is sufficiently isolated behind a feature flag that it can be landed on `main` now without forcing backend selection decisions elsewhere in the stack. | Cherry-pick the `consensus-core` OmniPaxos branch onto `main`, validate the default build plus the `omnipaxos` feature path, and revalidate the full workspace. | `cargo test -p consensus-core` passes, `cargo test -p consensus-core --features omnipaxos` passes, and `cargo test --workspace` still passes after the cherry-pick. | Confirmed. | advance | Review and integrate the placement branch next, then return to feature-gated OpenRaft and runtime-side backend selection. |
| 10 | The placement branch is the next highest-value completed worker result because real hyperspace-style placement is part of the user’s core requirements and the change is already validated in its worktree. | Cherry-pick the placement branch onto `main`, validate placement-specific tests first, and then revalidate the full workspace against the new placement API and behavior. | `cargo test -p placement-core` passes with the new hyperspace ring tests, and `cargo test --workspace` passes on `main` after the cherry-pick. | Confirmed. | advance | Integrate the feature-gated OpenRaft backend next and make the two consensus alternatives coexist on `main`. |
| 11 | The OpenRaft backend can coexist with OmniPaxos in `consensus-core` if both are kept feature-gated and the workspace dependencies are merged cleanly. | Cherry-pick the OpenRaft branch onto `main`, resolve the feature/dependency conflicts so both backends remain available, validate the default build plus the `openraft` feature path, and revalidate the full workspace. | `cargo test -p consensus-core` passes, `cargo test -p consensus-core --features openraft` passes, and `cargo test --workspace` passes after the merged OpenRaft integration. | Confirmed. | advance | Wire runtime-side backend selection next so the consensus alternatives can be exercised from configuration instead of existing only as compile-time options. |
| 12 | The next useful improvement is to make the already-landed consensus alternatives selectable from `ClusterConfig` in `server`, so backend choice becomes part of the runtime shape instead of a compile-time-only detail. | Expand `ConsensusBackend` to name the concrete backends, add server feature forwarding plus runtime backend selection, test both disabled and enabled feature cases in `server`, and revalidate the workspace. | `cargo test -p consensus-core` passes, `cargo test -p consensus-core --features omnipaxos` passes, `cargo test -p consensus-core --features openraft` passes, `cargo test -p server --features omnipaxos` passes, `cargo test -p server --features openraft` passes, and `cargo test --workspace` passes after the selector lands. | Confirmed. | advance | Apply the same configuration-driven selection pattern to placement, storage, and transport next. |
| 13 | The runtime shape is still too hard-coded if placement, storage, and internode transport remain fixed even after consensus became configurable. | Add placement, storage, and internode-transport selection to `server`, back RocksDB selection with an ephemeral runtime directory for tests, add shape-selection tests, and revalidate the workspace plus the feature-enabled server builds. | `cargo test -p server` passes, `cargo test --workspace` passes, `cargo test -p server --features omnipaxos` passes, and `cargo test -p server --features openraft` passes after the new selectors land. | Confirmed. | advance | Start using the configured runtime shape in daemon startup and richer legacy request handling next. |
| 14 | Backend selection is not fully real until daemon startup uses it and the legacy listener serves at least one actual request through the configured runtime. | Build the daemon runtime from the parsed backend flags, keep RocksDB pointed at the daemon data directory, add framed legacy request/response helpers, implement `REQ_COUNT` over the configured runtime, and revalidate the targeted crates plus the full workspace. | `cargo test -p legacy-protocol -p legacy-frontend -p server` passes and `cargo test --workspace` passes after the daemon startup and `REQ_COUNT` handling changes. | Confirmed. | advance | Extend the legacy-compatible public surface with another real operation such as `REQ_GET` or a write path next. |
| 15 | A key-based read path is the next useful compatibility increment because it exercises request decoding, runtime lookup, and typed response encoding without yet committing to the write-path layout. | Extend `legacy-protocol` with `REQ_GET` request and response bodies, route `REQ_GET` through `ClusterRuntime`, share legacy request handling in `server`, and revalidate the targeted crates plus the full workspace. | `cargo test -p legacy-protocol -p legacy-frontend -p server` passes and `cargo test --workspace` passes after the new `REQ_GET` handling lands. | Confirmed. | advance | Implement the first legacy write path next, but follow HyperDex's real `REQ_ATOMIC` message instead of inventing a separate put request. |
| 16 | The first legacy write path should follow HyperDex's real public protocol, which means `REQ_ATOMIC` and `RESP_ATOMIC` with mutation flags, not a made-up put message. | Extend `legacy-protocol` with simplified `REQ_ATOMIC` request and response bodies, route legacy atomic writes and deletes through `ClusterRuntime`, honor the basic fail-if-found and fail-if-not-found flags, add focused protocol/frontend/server tests, and revalidate the targeted crates plus the full workspace. | `cargo test -p legacy-protocol -p legacy-frontend -p server` passes and `cargo test --workspace` passes after the simplified `REQ_ATOMIC` path lands. | Confirmed. | advance | Tighten `REQ_ATOMIC` toward HyperDex's `key_change` layout with checks and funcall-style mutations, starting from conditional put and scalar atomic operations. |
| 17 | The simplified atomic path will only satisfy `hyhac` if it can express conditional puts and at least the scalar numeric atomic operations that the suite uses heavily. | Extend `legacy-protocol` with atomic checks and funcall-style mutation names, route them through `server` as conditional puts and scalar numeric mutations, fix the memory engine so failed conditional writes do not create phantom records, add focused protocol/frontend/server/storage tests, and revalidate the targeted crates plus the full workspace. | `cargo test -p legacy-protocol -p legacy-frontend -p server -p engine-memory` passes and `cargo test --workspace` passes after the atomic tightening step lands. | Confirmed. | advance | Implement the legacy search request flow next, starting with `REQ_SEARCH_START` and streamed result responses over the existing runtime search results. |
