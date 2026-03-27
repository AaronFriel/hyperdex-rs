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
- A real multi-daemon `hyperdex-rs` deployment forms a cluster through a live
  coordinator instead of each daemon constructing an isolated local runtime.
- Real internode replication and request forwarding occur between separate
  daemon processes.
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

The runtime now preserves logical degraded reads in fixed integration tests, in
one deterministic `turmoil` simulation, and in one deterministic `madsim`
simulation. The requested Hegel path is now viable and integrated for one real
memory-engine sequence model and the single-node runtime client surface, so the
next limiting gap is distributed mutation diversity: Hegel now exercises both
healthy and degraded multi-runtime reads, but it still does not cover generated
delete or conditional-write behavior across runtimes.

## Milestones

1. Bootstrap the repository, workspace layout, and compatibility notes.
2. Prove the exact `hyhac` operation surface and error semantics we need to serve.
3. Land shared domain crates for schema, placement, storage, and protocol types.
4. Land a single-node but trait-correct control plane and data plane.
5. Land real distributed control-plane cluster formation through the coordinator.
6. Land real distributed data-plane replication and forwarding between daemons.
7. Add alternative consensus and transport implementations in dedicated worktrees.
8. Add deterministic simulation harnesses plus multiprocess validation.
9. Run the `hyhac` suite against the live cluster and close semantic gaps.

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
- Done: the legacy frontend now handles `REQ_SEARCH_START` and `REQ_SEARCH_NEXT`
  over the configured runtime, with cursor-backed result streaming and focused
  compatibility tests.
- Done: the deterministic test harness now includes a `turmoil` session test
  plus property-based model checking for the memory engine, instead of only a
  basic in-process round-trip.
- Done: the admin runtime now exposes a versioned config view plus stable wait
  semantics over space lifecycle changes, so coordinator-facing admin mutations
  are observable instead of being anonymous local edits.
- Done: the admin protocol now carries exact HyperDex coordinator return codes,
  exact admin-status mapping, and a server-side `space_add` / `space_rm`
  dispatcher that returns the real 2-byte coordinator reply payload.
- Done: coordinator mode now starts a live control service on the control port,
  serving `space_add` and `space_rm` over TCP on top of the existing dispatcher
  and exact 2-byte coordinator replies.
- Done: the live coordinator control service now serves `wait_until_stable`
  with a stable-version body and `config_get` with a structured config snapshot,
  while preserving the exact 2-byte reply path for `space_add` and `space_rm`.
- Done: coordinator mode now accepts `daemon_register`, tracks live daemon
  identity in `ConfigView.cluster.nodes`, and updates placement layout metadata
  as registrations arrive.
- Done: daemon startup now registers with the coordinator, fetches
  coordinator-published config on startup, and keeps polling for later config
  changes while serving the legacy frontend.
- Done: a new multiprocess harness proves one coordinator process plus two
  daemon processes share remotely-created space state instead of behaving like
  isolated local runtimes.
- Done: the transport abstraction now carries addressed internode data-plane
  requests, and the gRPC transport can route `put` and `get` to a remote
  primary between separate runtimes.
- Done: the addressed internode path now also routes delete to the remote
  primary, with a focused cross-runtime delete proof.
- Done: the addressed internode path now also routes `ConditionalPut` to the
  remote primary, with a focused cross-runtime compare-and-write proof.
- Done: the addressed `Put` path is now proven to carry scalar numeric mutation
  to the remote primary, matching the mutation shape the legacy atomic flow
  already uses.
- Done: daemon startup now hosts the internode gRPC service on the daemon
  control port when `--transport=grpc` is selected, and the multiprocess
  harness now proves that a legacy `REQ_ATOMIC` sent to one daemon is routed
  to the remote primary and observed with `REQ_GET` on the other daemon.
- Done: committed `Put` and `ConditionalPut` operations now fan out to the
  placement replica set over internode gRPC, and the transport-grpc harness
  proves a secondary runtime stores replicated state after a public legacy
  atomic write.
- Done: committed delete now fans out to the placement replica set as well, and
  the transport-grpc harness proves a replicated record disappears from both
  runtimes after a distributed delete.
- Done: distributed delete-group now converges across replicas too, with a
  focused proof that matching records disappear from every replica while
  non-matching records survive.
- Done: `simulation-harness` now includes a first Hegel-backed property test
  for latest-write-wins behavior in the memory engine, and this host satisfies
  Hegel's `uv` requirement.
- Done: `simulation-harness` now includes a Hegel-backed stateful operation
  sequence over put/delete/get transitions in the memory engine.
- Done: `simulation-harness` now includes a Hegel-backed single-node
  `ClusterRuntime` sequence model for put/get/delete/count behavior.
- Done: `simulation-harness` now includes a Hegel-backed distributed routing
  property for a healthy two-runtime pair.
- Done: `simulation-harness` now includes a Hegel-backed degraded distributed
  read property for replica-backed `Get` and logical `Count`.
- Known gap: Hegel now covers distributed delete, but it still does not cover
  distributed conditional-write semantics, so generated cross-runtime mutation
  diversity remains incomplete.
- Active: dedicated worktrees are now producing distributed control-plane,
  distributed data-plane, and multiprocess validation changes in parallel.
- Next: extend Hegel coverage from distributed delete to distributed
  conditional-write behavior in `simulation-harness`.

## Next Bounded Iteration

Add a Hegel-backed distributed conditional-write property in
`simulation-harness` so generated tests cover a routed compare-and-write
mutation across runtimes, not just writes, deletes, and degraded reads.

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
| 18 | The next missing client-facing behavior for the active compatibility path is HyperDex-style search start and cursor-driven result delivery, because `hyhac` exercises search heavily and the runtime already has enough search semantics to back it. | Extend `legacy-protocol` with search start, search continue, search item, and search done bodies; route `REQ_SEARCH_START` and `REQ_SEARCH_NEXT` through `ClusterRuntime` with stored cursors; add focused protocol, frontend, and server tests; and revalidate the workspace. | `cargo test -p legacy-protocol -p legacy-frontend -p server` passes in the search worktree, and `cargo test --workspace` passes there and again on `main` after the cherry-pick as commit `837e412`. | Confirmed. | advance | Strengthen deterministic confidence next so the growing compatibility layer has model-backed regression protection instead of only example tests. |
| 19 | The public compatibility layer is growing quickly enough that a stronger deterministic test harness will pay off now by checking behavior against a simple model before the live `hyhac` run starts exposing harder semantic gaps. | Expand `simulation-harness` with a deterministic `turmoil` data-plane session test and a property-based model comparison for the memory engine, then revalidate the full workspace and integrate the result on `main`. | `cargo test -p simulation-harness` passes in the simulation worktree, and `cargo test --workspace` passes there and again on `main` after the cherry-pick as commit `013df66`. | Confirmed. | advance | Move to the coordinator/admin compatibility path next, starting from config view and space lifecycle operations required to boot a live `hyhac` run. |
| 20 | Before adding the real coordinator listener, the runtime needs explicit admin-visible config state and stable semantics so later wire-compatibility work has something concrete to expose. | Extend `hyperdex-admin-protocol` with config-view and stable-wait requests, add versioned coordinator state to `ClusterRuntime`, make space create and drop advance that state, add a focused lifecycle test, and revalidate the full workspace. | `cargo test -p hyperdex-admin-protocol -p server` passes with the new lifecycle test, and `cargo test --workspace` passes after the versioned config-view and stable-wait changes land locally on `main`. | Confirmed. | advance | Implement the first coordinator-side legacy admin boundary next, starting with `space_add` and `space_rm` dispatch plus HyperDex return-code mapping. |
| 21 | Once the runtime has versioned admin state, the next useful compatibility increment is exact coordinator reply handling for `space_add` and `space_rm`, because that is the smallest real HyperDex admin wire contract and it can be validated without full Replicant or full config-payload compatibility. | Extend `hyperdex-admin-protocol` with exact HyperDex coordinator return codes, exact admin-status mapping, and typed admin request forms; add server-side `space_add` / `space_rm` dispatch plus a method-based handler that returns the exact 2-byte coordinator reply bytes; add focused protocol and server tests; and revalidate the full workspace. | `cargo test -p hyperdex-admin-protocol -p server` passes with the new coordinator-code and dispatch tests, and `cargo test --workspace` passes after the new admin boundary lands locally on `main`. | Confirmed. | advance | Build the first live coordinator control service next, serving `space_add` and `space_rm` on the control port with the new 2-byte replies while full `config` payload compatibility remains deferred. |
| 22 | The next useful step after in-process admin dispatch is a live coordinator control service, because it turns the existing method handler into an actual network boundary without yet forcing full Replicant compatibility. | Add a TCP control listener for coordinator mode, frame control requests as method plus typed body, serve `space_add` and `space_rm` through `handle_coordinator_admin_method`, add focused TCP listener tests, and revalidate the full workspace. | `cargo test -p server` passes with the new control-service tests, and `cargo test --workspace` passes after coordinator mode begins serving the control port on `main`. | Confirmed. | advance | Extend the live coordinator control path next with `wait_until_stable` and a minimal config-follow path that a compatibility client can actually consume. |
| 23 | Once the live control service exists, the next useful compatibility increment is to add `wait_until_stable` and a minimal config-follow path so a client can observe coordinator state instead of only issuing space mutations. | Extend `CoordinatorAdminRequest` with `WaitUntilStable` and `ConfigGet`, add an optional-body control response path, serve stable-version and config-snapshot bodies from coordinator mode, add focused TCP tests for both operations, and revalidate the full workspace. | `cargo test -p server` passes with the new stable/config control-service tests, and `cargo test --workspace` passes after the live control service grows those two operations on `main`. | Confirmed. | advance | Build the first legacy-admin client compatibility path next, starting with deferred-call and event-loop semantics that can back `hyperdex_admin_loop`, `add_space`, `rm_space`, and `wait_until_stable`. |
| 24 | Real distributed control-plane progress starts with the coordinator owning daemon membership instead of every daemon keeping a fixed local node list. | Cherry-pick the daemon-registration worktree onto `main`, add coordinator-side daemon registration plumbing plus daemon identity parsing, validate control-plane protocol/runtime behavior with focused tests, and reframe the next move around daemon startup consuming shared coordinator state. | Commit `7c864a6` lands `daemon_register` on `main`; `cargo test -p control-plane -p hyperdex-admin-protocol -p server` passes with new registration tests; the coordinator now updates both `ConfigView.cluster.nodes` and placement layout membership as daemons register. | Confirmed. | advance | Make daemon startup register and synchronize against coordinator state so real multi-daemon formation exists outside the coordinator process. |
| 25 | Once the coordinator owns daemon membership, the next control-plane proof is to make daemon processes consume shared coordinator state during startup and after later config changes. | Add coordinator config synchronization into daemon startup, keep a background refresh loop alive while the daemon serves requests, add a multiprocess harness that boots one coordinator plus two daemon processes, creates a space through the coordinator, waits for daemon sync, and verifies both daemons can serve `REQ_COUNT` for that space, then revalidate the workspace. | `cargo test -p server --test dist_multiprocess_harness -- --nocapture` passes on `main`; `cargo test -p server` passes with the new process harness included; `cargo test --workspace` passes after daemon startup begins synchronizing config from the coordinator. | Confirmed. | advance | Land the first real cross-daemon data path next, starting with routed `put` / `get` between separate daemons over the transport abstraction. |
| 26 | With shared coordinator state in place, the next bounded distributed proof is a real cross-daemon data path that routes requests to the primary instead of keeping every client request local. | Cherry-pick the data-plane worktree onto `main`, resolve the runtime merge against the newer coordinator-sync code, preserve coordinator runtimes with empty node lists, validate the new gRPC internode forwarding path plus the full workspace, and reframe the next step around broadening distributed operations beyond `put` / `get`. | Commit `02420f5` lands addressed internode forwarding on `main`; the follow-up coordinator-runtime fix keeps empty-node coordinator runtimes valid; `cargo test -p transport-grpc --test public_frontend` passes with `grpc_forwards_data_plane_requests_between_two_runtimes`; `cargo test -p server` and `cargo test --workspace` both pass after the merge. | Confirmed. | advance | Extend the addressed internode path to the next highest-value legacy operation and add tighter deterministic coverage for the distributed behavior. |
| 27 | After routed `put` / `get`, delete is the smallest next distributed legacy operation because it uses the same primary-routing mechanism while changing remote record state. | Extend `DataPlaneRequest` with delete, route `ClientRequest::Delete` through the addressed transport abstraction, add a focused cross-runtime delete proof in the gRPC transport test, and revalidate the transport tests, `server`, and the full workspace. | `cargo test -p transport-grpc --test public_frontend` passes with `grpc_forwards_delete_requests_between_two_runtimes`; `cargo test -p server` passes; `cargo test --workspace` passes after routed delete lands on `main`. | Confirmed. | advance | Extend the addressed transport path to remote `ConditionalPut` next so distributed mutation semantics cover the legacy atomic compare-and-write flow. |
| 28 | After routed delete, `ConditionalPut` is the next highest-value distributed mutation because the legacy atomic request flow already relies on compare-and-write semantics and status mapping. | Extend `DataPlaneRequest` with `ConditionalPut`, route `ClientRequest::ConditionalPut` through the addressed transport abstraction, add a focused cross-runtime compare-and-write proof in the gRPC transport test, and revalidate the transport tests, `server`, and the full workspace. | `cargo test -p transport-grpc --test public_frontend` passes with `grpc_forwards_conditional_put_requests_between_two_runtimes`; `cargo test -p server` passes; `cargo test --workspace` passes after routed conditional write lands on `main`. | Confirmed. | advance | Extend the addressed transport path to the next legacy atomic mutation shape, starting with scalar numeric mutation. |
| 29 | After routed `ConditionalPut`, the next distributed legacy mutation shape to verify is scalar numeric mutation, because the legacy atomic path already emits it through the existing `Put` mutation vector. | Add a focused cross-runtime numeric-mutation proof in the gRPC transport test, revalidate the transport tests, `server`, and the full workspace, and use the result to decide whether another transport variant is needed. | `cargo test -p transport-grpc --test public_frontend` passes with `grpc_forwards_numeric_mutation_requests_between_two_runtimes`; `cargo test -p server` passes; `cargo test --workspace` passes, confirming the existing routed `Put` path already carries scalar numeric mutation correctly to the remote primary. | Confirmed. | advance | Move up one layer and prove the distributed mutation path through the legacy atomic frontend itself across daemon boundaries. |
| 30 | After the numeric mutation proof, the next useful check is to drive that same distributed mutation path through the public legacy atomic request surface instead of only the typed internal API. | Add a focused legacy-TCP proof in the gRPC transport test that sends `REQ_ATOMIC` to one runtime, forwards the write to the remote primary over real gRPC internode transport, verifies the result with `REQ_GET` on the remote runtime, remove the overreaching process-level test that assumed daemon gRPC hosting already existed, and revalidate the transport tests, `server`, and the full workspace. | `cargo test -p transport-grpc --test public_frontend` passes with `legacy_atomic_public_path_forwards_to_remote_primary_runtime`; `cargo test -p server` passes after removing the invalid process-level test; `cargo test --workspace` passes, proving the legacy public mutation path across real networked runtimes while also showing that daemon process startup still lacks internode gRPC hosting. | Confirmed, with a narrower boundary than first attempted. | advance | Make daemon startup host and use the internode gRPC service so the same legacy atomic proof can move from the multi-runtime harness into the real multiprocess daemon harness. |
| 31 | The next limiting gap after the multi-runtime legacy proof is daemon-process internode hosting, because without it the same distributed mutation path cannot be proven across the real coordinator-plus-daemons harness. | Add a server-local tonic/prost build path for the internode RPC, make daemon startup install a gRPC transport adapter and host the internode service on `control_port` when `--transport=grpc` is selected, route remote gRPC dials to `control_port`, restore the multiprocess legacy atomic test, and revalidate `server` plus the full workspace. | `cargo test -p server --test dist_multiprocess_harness -- --nocapture` passes with `legacy_atomic_routes_numeric_update_to_remote_primary_process`; `cargo test -p server` passes; `cargo test --workspace` passes, proving a legacy `REQ_ATOMIC` can now cross real daemon processes through the coordinator-published cluster layout and the daemon-hosted internode gRPC service. | Confirmed. | advance | Add first real replication fanout beyond the primary and prove a secondary daemon stores replicated state after a distributed write. |
| 32 | Once daemon-process forwarding works, the next missing distributed-system property is replica fanout, because a primary-only write still leaves secondaries stale even though placement already computes a replica set. | Add an explicit replica-apply internode request, make successful primary `Put` and `ConditionalPut` operations fan out to the placement replicas, add a focused public-path test in `transport-grpc` with `replicas = 2`, and revalidate the transport tests, `server`, and the full workspace. | `cargo test -p transport-grpc --test public_frontend` passes with `legacy_atomic_replicates_to_secondary_runtime`; `cargo test -p server` passes; `cargo test --workspace` passes, proving a public legacy atomic write now leaves both the primary and the secondary runtime with stored state. | Confirmed. | advance | Extend the same replica-fanout path to delete and prove replicated records disappear from both runtimes after a distributed delete. |
| 33 | After replica fanout for writes, delete is the next missing convergence property because a secondary that keeps a deleted record is an immediate correctness bug. | Add an explicit replicated-delete internode request, make successful primary delete fan out to the placement replicas, add a focused distributed delete proof in `transport-grpc`, and revalidate the transport tests, `server`, and the full workspace. | `cargo test -p transport-grpc --test public_frontend` passes with `distributed_delete_removes_replicated_state_from_secondary_runtime`; `cargo test -p server` passes; `cargo test --workspace` passes, proving a distributed delete now removes replicated state from both the primary and the secondary runtime. | Confirmed. | advance | Extend replica convergence to delete-group and prove matching records disappear from all replicas after a distributed group delete. |
| 34 | After single-key delete convergence, delete-group is the next missing multi-record mutation property because group deletion that only affects one replica would leave immediately visible divergence. | Add an explicit replicated-delete-group internode request, make distributed `DeleteGroup` fan out across the cluster and normalize the logical deleted count by replica factor, add a focused distributed delete-group proof in `transport-grpc`, and revalidate the transport tests, `server`, and the full workspace. | `cargo test -p transport-grpc --test public_frontend` passes with `distributed_delete_group_removes_matching_records_from_all_replicas`; `cargo test -p server` passes; `cargo test --workspace` passes, proving matching records now disappear from every replica while a non-matching replicated record remains. | Confirmed. | advance | Make distributed search converge across replicas and prove a multi-record query returns the same logical result set regardless of which replica handles it. |
| 35 | After delete-group convergence, distributed search is the next missing multi-record read property because every matching record now exists on multiple replicas and an uncoordinated fanout would overcount or duplicate logical rows. | Add an internode `Search` request and response shape, make `ClientRequest::Search` fan out across cluster nodes and dedupe logical records by key, add a focused distributed search proof in `transport-grpc`, and revalidate the transport tests, `server`, and the full workspace. | `cargo test -p transport-grpc --test public_frontend` passes with `distributed_search_dedupes_replicated_records_across_runtimes`; `cargo test -p server` passes; `cargo test --workspace` passes, proving replicated search now returns one logical result per matching key regardless of which runtime handles the request. | Confirmed. | advance | Serve `Get` from the replica set and prove a replica can satisfy a key lookup when the primary is unavailable. |
| 36 | After distributed search convergence, the most immediate remaining healthy-cluster read bug is `Count`, because it still reads only local state and therefore undercounts across primaries while any naive fanout would also risk double-counting replicas. | Reframe the next bounded step from primary-failure `Get` fallback to logical distributed `Count`, implement `Count` as a thin wrapper over the existing distributed search fanout and dedupe path, add a focused three-runtime proof in `transport-grpc`, correct the multiprocess harness to use real gRPC internode transport for multi-process distributed reads, and revalidate the transport tests, `server`, and the full workspace. | A scout review showed `ClientRequest::Count` still used only `self.data_plane.count(...)` while distributed search was already logical and deduped; `cargo test -p transport-grpc --test public_frontend` passes with `distributed_count_returns_logical_matches_from_any_runtime`; `cargo test -p server --test dist_multiprocess_harness coordinator_space_add_reaches_multiple_daemon_processes -- --nocapture` passes after switching that harness to `--transport=grpc`; `cargo test -p server` passes; `cargo test --workspace` passes. | Confirmed. | reframe | Return to replica-serving `Get` fallback and prove a replica can satisfy a key lookup when the primary is unavailable. |
| 37 | After logical distributed `Count`, the next missing single-key availability property is primary-failure `Get` behavior, because a replicated record should remain readable from another daemon when the primary transport is down. | Add ordered replica fallback for `ClientRequest::Get` using the existing internode `Get` path, keep the primary-first behavior when healthy, add a focused public gRPC proof that shuts down the primary runtime and reads the replicated record through the surviving daemon, and revalidate the transport tests, `server`, and the full workspace. | `cargo test -p transport-grpc --test public_frontend` passes with `distributed_get_falls_back_to_local_replica_when_primary_grpc_is_down`; `cargo test -p server` passes; `cargo test --workspace` passes, proving a public `Get` can still return the replicated record after the primary daemon's gRPC service is shut down. | Confirmed. | advance | Make distributed `Search` and `Count` tolerate one unavailable daemon while preserving logical results across surviving replicas. |
| 38 | After degraded `Get` fallback, the next missing read-availability property is degraded multi-record reads, because `Search` and `Count` still aborted on one unreachable daemon even when replica coverage meant the logical answer was available from survivors. | Teach distributed `Search` to skip remote unavailability while still failing on real protocol errors or total replica loss, let `Count` inherit that behavior through the existing distributed search path, add a focused degraded-read proof in `transport-grpc` that shuts down one daemon and verifies the surviving runtime still returns the correct logical search results and count, and revalidate the transport tests, `server`, and the full workspace. | `cargo test -p transport-grpc --test public_frontend` passes with `distributed_search_and_count_survive_one_daemon_shutdown`; `cargo test -p server` passes; `cargo test --workspace` passes, proving logical search results and logical counts now survive one daemon shutdown when replica coverage remains. | Confirmed. | advance | Carry degraded `Search` and `Count` proof into the real coordinator-plus-daemons harness. |
| 39 | After degraded multi-record reads work in the gRPC runtime harness, the next missing proof is the real coordinator-plus-daemons process surface, because the user asked for a real distributed system rather than only in-process correctness. | Add a coordinator-plus-daemons harness test that writes replicated data, shuts down one daemon process, and verifies the surviving daemon still returns the expected logical legacy search results and total count, then revalidate `server` and the full workspace. | `cargo test -p server --test dist_multiprocess_harness degraded_search_and_count_survive_one_daemon_process_shutdown -- --nocapture` passes; `cargo test -p server` passes; `cargo test --workspace` passes, proving degraded search and count through the real daemon-process harness. | Confirmed. | advance | Add deterministic simulation coverage for degraded `Get`, `Search`, and `Count`. |
| 40 | After real-process degraded-read proof, the next missing coverage is deterministic failure simulation, because integration tests alone do not vary node-loss timing cheaply enough to harden the behavior. | Add a `turmoil` simulation with a shared fake transport and two real `ClusterRuntime` instances, inject one node failure after replicated writes, prove degraded `Get`, `Search`, and `Count` still return the expected logical results, add the minimal simulation-harness dependencies needed for that runtime-level proof, harden the process harness readiness check to wait on the daemon control port instead of a log line, and revalidate `simulation-harness`, `server`, and the full workspace. | `cargo test -p simulation-harness` passes with `turmoil_preserves_degraded_read_correctness_after_one_node_loss`; `cargo test -p server` passes after replacing the flaky daemon gRPC log wait with a control-port readiness probe; `cargo test --workspace` passes. | Confirmed. | advance | Add `madsim` degraded-read coverage and compare it against the existing `turmoil` and real-process proofs. |
| 41 | After the `turmoil` proof, the next missing simulation surface is `madsim`, because the user explicitly asked for both deterministic schedulers and the degraded-read proof should survive the second runtime as well. | Add a cfg-gated `madsim` degraded-read proof to `simulation-harness`, keep default workspace builds clean by registering `cfg(madsim)` with Cargo check-cfg, generalize the degraded-read target selection so it follows actual placement instead of assuming one fixed primary, and revalidate the default harness, the explicit `madsim` path, `server`, and the full workspace. | `cargo test -p simulation-harness` passes; `RUSTFLAGS='--cfg madsim' cargo test -p simulation-harness madsim_preserves_degraded_read_correctness_after_one_node_loss -- --nocapture` passes; `cargo test -p server` passes; `cargo test --workspace` passes, proving the degraded `Get`, `Search`, and `Count` path under both deterministic simulation runtimes plus the existing process harness. | Confirmed. | advance | Confirm the requested Hegel property-testing path and either land a first property check with it or record an evidence-backed reframe if no practical Rust Hegel crate exists. |
| 42 | After the `madsim` proof, the next open testing request is the Hegel property path, because the user asked for it explicitly and the harness still relied on `proptest` alone for property-level confidence. | Confirm the practical Rust Hegel crate and host prerequisites, add `hegeltest` as a dev dependency under the exported crate name `hegel`, land one generated latest-write-wins property in `simulation-harness`, and revalidate the targeted Hegel test, the full harness crate, and the full workspace. | `cargo search hegel --limit 10` shows `hegeltest` as the Rust property-testing crate; `cargo info hegeltest` reports version `0.2.6`; `uv --version` returns `uv 0.6.6`; `cargo test -p simulation-harness hegel_memory_engine_tracks_latest_write_per_key -- --nocapture` passes; `cargo test -p simulation-harness` passes; `cargo test --workspace` passes. | Confirmed. | advance | Extend Hegel coverage to a stateful operation-sequence model so it overlaps the existing `proptest` model instead of remaining a single-property smoke test. |
| 43 | After the first Hegel smoke test, the next missing depth is a real stateful sequence model, because one generated write property still left the richer evolving-key behavior to `proptest` alone. | Replace the write-only Hegel check with a stateful put/delete/get sequence model, make Hegel use an explicit `HEGEL_SERVER_COMMAND` pinned to a temp-installed `hegel-core` binary instead of relying on the crate-local bootstrap path, revalidate `simulation-harness`, and rerun the full workspace. | `cargo test -p simulation-harness` passes with `hegel_memory_engine_matches_stateful_sequence_model`; `cargo test --workspace` passes; the Hegel server bootstrap now installs to `/tmp/hyperdex-rs-hegel-core-0.2.3/venv` and no longer depends on a fragile crate-local `.hegel/venv` bootstrap. | Confirmed. | advance | Extend Hegel coverage to the single-node `ClusterRuntime` client surface so property checks move one layer above the raw memory engine. |
| 44 | After the Hegel memory-engine sequence model, the next missing scope is the single-node runtime surface, because property checks should also cover the client API layer that maps requests into the runtime and storage engine. | Add a single-node runtime fixture to `simulation-harness`, land a Hegel-backed put/get/delete/count sequence property over `HyperdexClientService::handle`, revalidate `simulation-harness`, and rerun the full workspace. | `cargo test -p simulation-harness` passes with `hegel_single_node_runtime_matches_sequence_model`; `cargo test --workspace` passes; the Hegel-backed sequence now covers both the raw memory engine and the single-node runtime client surface. | Confirmed. | advance | Extend Hegel coverage to a small distributed multi-runtime routing property so routed client behavior is also covered by generated tests. |
| 45 | After the single-node runtime property, the next missing scope is healthy distributed routing, because Hegel should also cover the two-runtime client path that forwards to the primary and reads the routed result back. | Add a two-runtime fixture to `simulation-harness`, land a Hegel-backed distributed put/get routing property over `HyperdexClientService::handle`, revalidate `simulation-harness`, and rerun the full workspace. | `cargo test -p simulation-harness` passes with `hegel_distributed_runtime_routes_put_and_get`; `cargo test --workspace` passes; the Hegel-backed generated checks now cover the raw memory engine, the single-node runtime client surface, and the healthy two-runtime routed path. | Confirmed. | advance | Extend Hegel coverage to degraded distributed reads so generated tests also cover replica-backed availability after one runtime becomes unavailable. |
| 46 | After healthy distributed routing, the next missing generated property is degraded distributed reads, because Hegel should also cover replica-backed availability after one runtime becomes unavailable. | Extend the distributed fixture to expose the shared simulated transport, add a Hegel-backed degraded distributed read property over routed `Get` and logical `Count`, revalidate `simulation-harness`, rerun the full workspace, and confirm the existing daemon-process degraded-read harness still passes. | `cargo test -p simulation-harness` passes with `hegel_distributed_runtime_preserves_degraded_get_and_count`; `timeout 120s cargo test -p server --test dist_multiprocess_harness degraded_search_and_count_survive_one_daemon_process_shutdown -- --nocapture` passes; `cargo test --workspace` passes, proving the new generated degraded-read property without regressing the existing distributed harness. | Confirmed. | advance | Extend Hegel coverage to a distributed delete property so generated tests cover a routed removal mutation across runtimes. |
| 47 | After degraded distributed reads, the next missing generated mutation property is routed delete, because Hegel should also cover a cross-runtime operation that removes previously replicated state. | Add a Hegel-backed distributed delete property over the existing two-runtime fixture in `simulation-harness`, verify that a routed delete clears the record through both the routed client path and the primary runtime, revalidate `simulation-harness`, and rerun the full workspace. | `cargo test -p simulation-harness` passes with `hegel_distributed_runtime_routes_delete`; `cargo test --workspace` passes on a fresh run with the new generated delete property included. | Confirmed. | advance | Extend Hegel coverage to a distributed conditional-write property so generated tests cover routed compare-and-write behavior across runtimes. |
