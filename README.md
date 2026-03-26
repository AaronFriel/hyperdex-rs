# hyperdex-rs

`hyperdex-rs` is a pure-Rust reimplementation of HyperDex with explicit control-plane
and data-plane separation. The immediate acceptance target is simple:

1. Stand up a cluster that exposes the public HyperDex admin and client surface
   needed by `hyhac`.
2. Keep consensus, placement, storage, and inter-node transport behind traits so
   multiple implementations can be validated side by side.
3. Drive confidence with deterministic simulation first, then run the real Haskell
   client test suite against the live system.

The campaign state lives in [docs/autoplan-hyperdex-rs.md](docs/autoplan-hyperdex-rs.md).
