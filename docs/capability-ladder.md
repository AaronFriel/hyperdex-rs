# Capability Ladder

This document groups larger `hyperdex-rs` feature areas by dependency and
implementation order.

It is not the active root AutoPlan. It is the architectural grouping that the
active root AutoPlan and backlog should draw from.

## Why This Exists

`hyperdex-rs` has a growing set of worthwhile directions:

- validation and proof hardening
- transactions
- georeplication
- programmability on the nodes
- relational, graph, vector, and temporal features
- object-store-like and erasure-coded storage features

They should not be prioritized as a flat list. Some of them make later work
materially easier. Some should start as bounded spikes because there are
multiple real implementation options. Some are mostly direct implementation
once the lower layers are strong enough.

## Group 1: Foundation

This group improves engineering leverage and lowers risk.

### Included

- GitHub Actions validation
- stronger deterministic failure testing
- fuzzing of critical parsers and protocol handlers
- native Rust async traits instead of `async_trait`
- tracing, metrics, and repair-oriented diagnostics

### Why First

- It shortens the path from change to evidence.
- It reduces regression risk.
- It makes every later feature cheaper to land and easier to debug.

### Typical Work Shape

- Usually direct implementation work
- Sometimes harness work, but only when it directly improves a product loop

## Group 2: Distributed Semantics

This group defines the real distributed behavior that higher layers depend on.

### Included

- Warp-style transactions
- Consus-style georeplication
- failure-domain-aware placement
- durability ladders
- locality-aware replication and recovery

### Why Second

- Transactions, region semantics, and failure domains shape how higher layers
  behave.
- Graph, temporal, server-side programs, and object features become cleaner if
  these semantics are explicit first.

### Typical Work Shape

- Often spike-first, because multiple implementation paths are plausible
- Should produce bounded design notes and then real code paths

## Group 3: Programmability

This group makes the database itself an execution environment.

### Included

- WASM runtime on coordinator or data nodes
- stored programs, triggers, and server-side query helpers
- CDC and changefeeds
- programmable metadata or policy hooks

### Why Third

- It is powerful but dangerous.
- It needs transaction semantics, resource isolation, and observability to be
  credible.

### Typical Work Shape

- Spike-first
- Requires explicit safety, scheduling, and resource boundaries

## Group 4: Data Models And Query Layers

This group exposes richer ways to model and query data on top of the shared
substrate.

### Included

- relational or record layers
- graph database features and abstractions
- vector database features
- temporal database features

### Relational And Record Layers

- Inspired by the idea of a record layer over a transactional substrate
- Best treated as a layer over the shared runtime instead of a competing engine

### Graph Features

- adjacency-heavy traversal support
- graph-shaped indexing and query abstractions
- graph-aware query decomposition if later justified by the substrate

### Vector Features

- HNSW is the obvious first candidate
- alternate ANN architectures should stay open when update cost, rebuild cost,
  recall, or distributed partitioning tradeoffs differ materially
- vector features should stay tied to the same placement, durability, and
  transaction semantics as the rest of the system

### Temporal Features

- basic temporal encoding may emerge from explicit `created_at` and
  `deleted_at` fields
- real time-travel queries and historical visibility need explicit semantics
- this likely intersects with transaction and storage-version design

### Why Fourth

- These are important product surfaces, but they are easier to implement well
  once the distributed semantics are already clear.

### Typical Work Shape

- Some direct implementation work
- some spike-first work where indexing or query-model choices are open

## Group 5: Storage Products

This group treats the cluster as a broader storage substrate.

### Included

- object-store-like features
- erasure-coded storage
- filesystem or metadata layers
- JuiceFS-like out-of-the-box storage products

### Why Fifth

- These features want strong placement, durability, region semantics, and
  observability first.
- They can easily become a distracting parallel product line if introduced too
  early.

### Typical Work Shape

- Usually spike-first
- likely requires a clean split between metadata placement and bulk data
  placement

## Prioritization Rule

Default order:

1. Foundation
2. Distributed semantics
3. Programmability
4. Data models and query layers
5. Storage products

This order is not absolute. A later-group spike is acceptable when:

- it clarifies a real architectural choice
- it stays bounded
- it produces reusable evidence
- it does not displace critical foundation work

## Good Spike Candidates

- Warp-style transaction coordination choices
- georeplication policy at space level versus key or region level
- WASM execution model and isolation
- vector indexing strategy beyond a first HNSW baseline
- object-store metadata versus data placement split
- temporal visibility semantics beyond plain timestamp fields

## Work That Usually Does Not Need A Spike

- GitHub Actions workflows
- fuzz harness setup
- `async_trait` removal
- straightforward deterministic failure tests

## How To Use This Document

- The root AutoPlan should pull active priorities from here.
- `future-directions.md` should remain the broad backlog note.
- Workstreams should be promoted in dependency order unless there is a strong
  reason to do otherwise.
