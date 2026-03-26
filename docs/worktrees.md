# Worktrees

The main branch keeps the trait boundaries, common schema types, and the first
working single-node path.

Dedicated worktrees exist for larger alternatives that should not share a dirty
line of development:

- `worktrees/consensus-openraft`
  Purpose: add an OpenRaft-backed replicated state machine path.
- `worktrees/consensus-omnipaxos`
  Purpose: evaluate a Paxos-family path for the same replicated state machine
  boundary.
- `worktrees/transport-grpc-impl`
  Purpose: replace the in-process transport with a real `prost`/gRPC transport.
- `worktrees/placement-alt`
  Purpose: evaluate alternate placement strategies against the same placement
  trait and simulation harness.

Each worktree starts from commit `cdc633b` and exists to keep large alternative
implementations isolated until they can be compared with targeted tests.
