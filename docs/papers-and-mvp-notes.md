# Papers And MVP Notes

This repository is being built with three paper streams in mind:

1. HyperDex for searchable placement and value-dependent replication.
2. Warp for transaction ideas that can sit above the base key-value substrate.
3. Consus for geo-replicated transaction commit and cleaner internal layering.

## What Stays In The MVP

- HyperDex-style spaces and typed attributes.
- Hyperspace-style placement as a first-class strategy, with alternative
  placement implementations behind the same trait.
- Separate control-plane and data-plane processes.
- Two public frontends:
  - a legacy frontend that is bit-for-bit compatible with the public HyperDex
    admin/client protocol used by existing clients such as `hyhac`
  - a modern gRPC frontend for new clients
- Strongly-ordered per-key behavior as the first correctness target.
- Search, count, delete-group, conditional updates, and atomic updates needed by
  `hyhac`.

## What We Intentionally Defer

- Warp's multi-key transaction protocol.
- Consus's geo-replicated transaction commit path.
- Full cross-data-center replication.
- Any internal protocol compatibility with the original HyperDex nodes.

## Design Direction

- The `placement-core` crate owns strategy selection so the original
  hyperspace-based design and alternate placement approaches can coexist.
- The `consensus-core` crate owns the replicated state machine boundary so
  single-node, Raft-like, and Paxos-like paths can be compared without
  rewriting the control plane.
- The `transport-core` crate owns internode messaging so the main line can use
  a simple in-process transport while the `transport-grpc-impl` worktree grows a
  `prost`/gRPC alternative.
- Public protocol compatibility is a separate concern from internode transport.
  The cluster should expose both a legacy HyperDex-compatible frontend and a
  modern gRPC frontend while remaining free to use a different Rust-native
  internode protocol.
- The schema model already carries a `SchemaFormat` enum so HyperDex DSL can be
  the first parser without preventing future protobuf-based schemas.
