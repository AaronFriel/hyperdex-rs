# Future Directions

This document records larger directions that are worth pursuing after or
alongside the current active workstreams. It is intentionally separate from the
active root AutoPlan board so the repository can preserve useful ideas without
pretending they are all in flight at once.

## User-Prioritized Directions

These are the explicit next-phase directions requested for `hyperdex-rs`.

### Warp-Style Transactions

- Add transaction coordination in the style of HyperDex Warp.
- Start with a bounded implementation over the current coordinator, placement,
  and replication model.
- Keep validation tied to real distributed behavior, not only local unit tests.

### Consus-Style Georeplication

- Add region-aware grouping for spaces, keys, or both.
- Make region and cluster placement explicit in configuration, control-plane,
  and replication terms.
- Treat this as a real distributed feature, not only a documentation idea.

### Repository-Grade Validation

- Add GitHub Actions workflows that exercise formatting, linting, tests, and
  bounded acceptance checks.
- Keep the workflow set close to the local developer loops already proven on
  `main`.

### Stronger Failure-Oriented Testing

- Add more adversarial Turmoil and Madsim tests.
- Use forked workers to build tests that intentionally break routing,
  replication, failover, and recovery assumptions.
- Prefer tests that expose or prevent real product regressions.

### Fuzzing

- Add fuzz targets for critical parsers, protocol decoders, and API handlers.
- Start with the legacy compatibility boundary and other malformed-input risks.

### Native Rust Async Traits

- Remove `async_trait` where modern Rust async traits are sufficient.
- Prefer coherent cross-crate conversions over scattered local edits.

## Additional Directions Worth Tracking

These are not above the user-prioritized list, but they are worth recording as
future areas of interest.

### Changefeeds And CDC

- Export ordered change streams from the distributed runtime.
- Keep this tied to real replication and durability semantics instead of a
  best-effort side channel.

### Failure-Domain-Aware Placement

- Model racks, zones, regions, and related failure domains directly in
  placement and repair logic.
- Treat this as a natural extension of the georeplication direction.

### Distributed Request Tracing

- Add request and replication tracing that can explain why a distributed
  operation took the path it did.
- Keep tracing useful for debugging live failures, not just for demos.

### Graph Or Vector Layers Above Spaces

- Explore whether typed spaces can support graph-shaped access patterns or
  vector-assisted search as an upper layer.
- Keep this above the current key-value and search substrate rather than
  turning it into a near-term core requirement.

## Positioning

- The active root AutoPlan should continue to prioritize material code
  delivery.
- This document is a backlog of directions with architectural value.
- A direction graduates from here into an active workstream when it has:
  - a bounded first implementation step
  - a clear mutable surface
  - a main validator
  - a fastest useful check
