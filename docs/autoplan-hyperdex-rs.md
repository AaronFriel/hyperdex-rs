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
- Running: worktree lanes for gRPC frontend, hyperspace placement fidelity,
  OpenRaft scaffolding, and OmniPaxos scaffolding.
- Next: turn the legacy protocol definitions into a real external listener that
  `hyhac` can target.

## Next Bounded Iteration

Build the first real legacy frontend skeleton on top of the new
`legacy-protocol` crate so the main branch stops at definitions only and starts
growing a callable external boundary.

## Loop Ledger

| Iteration | Hypothesis | Action | Evidence | Verdict | Disposition | Next Move |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | A workspace-first bootstrap will unblock parallel implementation without forcing early subsystem choices. | Create the new repository, write the AutoPlan, pin the `hyhac` compatibility target, and initialize the crate graph. | Workspace root and campaign files exist, the crate graph is in place, and `cargo test --workspace` passes. | Confirmed. | advance | Commit the baseline and use it to seed alternative implementation worktrees. |
| 2 | A committed baseline plus dedicated worktrees will keep alternative consensus, transport, and placement work from colliding with the main compatibility path. | Commit the workspace, create the `consensus-openraft`, `consensus-omnipaxos`, `transport-grpc-impl`, and `placement-alt` worktrees, then add schema and paper notes that clarify the MVP boundary. | Commit `cdc633b` exists, `git worktree list --porcelain` shows four new worktrees, and the workspace still passes `cargo test --workspace`. | Confirmed. | advance | Land the first real admin and client service implementations in the main line. |
| 3 | The main branch needs a concrete admin/client runtime before network compatibility can be attempted. | Add HyperDex DSL parsing, implement `ClusterRuntime` over the trait-based control/data plane, and test admin create/list plus client put/get/count/delete-group behavior. | Commit `f2db73b` exists, `cargo test -p server` passes, and `cargo test --workspace` still passes with the runtime adapter in place. | Confirmed. | advance | Build the external startup and network compatibility path that `hyhac` can actually drive. |
| 4 | The right boundary is dual public protocols, not replacement client libraries. | Record the user decision that the cluster must expose both a bit-for-bit HyperDex-compatible public protocol and a modern gRPC protocol, and reflect that split in configuration and campaign docs. | `ClusterConfig` now distinguishes public protocols from internode transport, and the docs name the legacy-plus-gRPC frontend requirement explicitly. | Confirmed. | advance | Start implementing the legacy HyperDex-compatible frontend as the first real external boundary. |
| 5 | The legacy frontend needs a dedicated code home before sockets or server loops are added. | Add the `legacy-protocol` crate and replace its stub with the HyperDex public message numbers, return codes, and request/response header definitions plus round-trip tests. | `cargo test -p cluster-config -p legacy-protocol` passes, `cargo test --workspace` passes, and the main branch now contains a dedicated legacy protocol crate. | Confirmed. | advance | Build the first callable legacy frontend skeleton on top of those protocol definitions. |
