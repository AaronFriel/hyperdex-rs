# hyhac Compatibility Surface

This note describes the observed public surface that `hyperdex-rs` must satisfy
to make `hyhac` useful as an end-to-end compatibility check.

## Admin

- create space
- remove space
- list spaces

## Client

- `put`
- `get`
- `delete`
- conditional `put`
- numeric atomic operations
- map atomic operations
- `search`
- `search_describe`
- `delete_group`
- `count`

## First Compatibility Boundary

The first useful cluster does not need every HyperDex feature. It does need the
observable semantics that the `hyhac` tests expect for these operations, plus
stable process management so the harness can start and stop a cluster reliably.
