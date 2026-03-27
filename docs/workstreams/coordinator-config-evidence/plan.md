# Workstream Plan: coordinator-config-evidence

This workstream plan is a living document. The sections `Progress`,
`Current Hypothesis`, `Next Bounded Step`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept current as work
proceeds.

This repository does not contain its own `PLANS.md`, so this file follows the
fallback rules at `/home/friel/.codex/skills/autoplan/references/PLANS.md`.

## Purpose / Big Picture

This workstream exists to keep the product worker from guessing at the
coordinator-side contract once the large-object `ClientGarbage` failure moved
earlier than the daemon request path. Its job is to turn the harness capture
and the original HyperDex sources into exact protocol evidence about packed
coordinator config and the client-side request-preparation contract for the
full `profiles` schema.

## Goal

Identify the exact coordinator-side packet or schema-contract mismatch that
prevents the focused large-object path from ever reaching `REQ_ATOMIC`.

## Acceptance Evidence

- The workstream names the exact coordinator-side exchange and the specific
  schema/config contract the original client expects.
- The result is tied to the harness capture, the original HyperDex sources,
  and the current Rust implementation rather than guesswork.
- The product worker gets a concrete target for the next code change.

## Mutable Surface

- none for the first bounded step; this is a read-only evidence pass
- if a later bounded step needs a tiny helper or diagnostic test, root must
  preregister that separately before any code changes begin

## Dependencies / Blockers

- Depends on the harness capture now on `main` in
  `legacy_hyhac_large_object_probe_reports_first_coordinator_frame_pair`.
- Should not overlap with the active product worker’s write surface.

## Plan Of Work

Use the focused harness capture, the original HyperDex coordinator/client
sources, and the current Rust packed-config path to decode what the client is
doing on the coordinator connection before the large-object path fails. The
first bounded step is read-only and should return the exact meaning of the
captured partial BusyBee-style frames and the likely packed config or schema
contract the original client is consuming before it decides whether to send the
first atomic write.

## Progress

- [x] (2026-03-27 20:04Z) Created this workstream after the harness and product
  worker both showed that the large-object failure still occurs before the
  daemon sees `REQ_ATOMIC`.
- [x] (2026-03-27 20:43Z) Decoded the captured coordinator bytes as BusyBee
  identify plus Replicant bootstrap traffic and tied the next product target
  to the packed `hyperdex::configuration` body behind the `hyperdex/config`
  follow reply.
- [x] (2026-03-27 20:20Z) Identified the first concrete mismatch inside that
  packed config body: Rust is emitting singleton primary-region bounds instead
  of HyperDex partition hash intervals.
- [x] (2026-03-27 20:24Z) Reopened this workstream for a third read-only step
  that turns the original HyperDex partition logic into exact expected region
  intervals and packed bytes for the live `profiles` config body.
- [x] (2026-03-27 20:28Z) Kept this workstream active after `1d6093c` landed,
  because the region-interval fix was necessary but not sufficient and the
  next read-only job is now to name the next exact packed-config mismatch.
- [x] (2026-03-27 20:31Z) Finished `cce-004` and identified the next concrete
  packed-config mismatch: zero-based ID allocation, especially
  `virtual_server_id=0`, where the original coordinator uses nonzero IDs from
  a shared counter.
- [x] (2026-03-27 20:36Z) Finished `cce-005` and proved that the concrete
  failing key `"large"` already routes to a non-null replica tuple on current
  `main`, so the remaining blocker lies beyond coordinator route selection.
- [x] (2026-03-27 20:41Z) Finished `cce-006` and identified the next exact
  pre-daemon contract after route selection: the client-to-daemon routing
  header and its `virtual_server_id -> server_id -> address` mapping plus
  version acceptance.
- [x] (2026-03-27 20:46Z) Finished `cce-007` and ruled out the routing-header
  contract for the concrete failing key `"large"`, so the remaining blocker is
  later than the daemon header gate.
- [x] (2026-03-27 20:51Z) Finished `cce-008` and ruled out the first atomic
  body contract as well: current `hyperdex-rs` is structurally aligned with the
  original `key_change` and `FUNC_SET` packing for the large-object put.
- [x] (2026-03-27 20:56Z) Finished `cce-009` and identified the first exact
  daemon-side divergence after a structurally valid atomic request: missing
  `kc->validate(schema)` handling and missing explicit
  `RESP_ATOMIC/NET_BADDIMSPEC` response semantics.
- [x] (2026-03-27 20:14Z) Reopened this workstream for a second read-only step
  that compares the Rust `default_legacy_config_encoder` output against the
  original HyperDex `configuration` / `space` packing rules on a live
  `profiles` config body.
- [x] (2026-03-27 21:34Z) Rechecked the cleaned post-`5879fab` baseline and
  confirmed that the focused large-object failure still does not reach daemon
  request handling; the active mismatch is back in the coordinator follow/config
  path the original client must complete before daemon traffic starts.
- [x] (2026-03-27 21:46Z) Finished `cce-011` and named the remaining exact
  mismatch: the Replicant bootstrap sender-identity contract still differs from
  the original, so the client never leaves the bootstrap retry loop.
- [x] (2026-03-27 22:35Z) Finished `cce-012` and turned that exact mismatch
  into a concrete Rust patch/test map; after the fix landed, the remaining
  comparison target moved one step later into non-wire bootstrap acceptance.

## Current Hypothesis

The sender-id mismatch and the repeated-identify mismatch are both fixed, and
the corrected BusyBee proxy now shows the focused large-object path advancing
into coordinator `CondWait` traffic. This workstream no longer owns the active
blocker. It should stay read-only and ready for the next exact comparison once
the post-follow path needs another source-backed reduction.

## Next Bounded Step

Keep this workstream read-only and parked until the post-follow path needs
another exact comparison. The next bounded step, when reopened, is to compare
the original HyperDex/Replicant behavior against the corrected coordinator
baseline after the first non-bootstrap `CondWait` traffic, not to revisit
bootstrap acceptance again.

## Surprises & Discoveries

- Observation: the harness can now prove that the failing path never reaches a
  decodable legacy daemon frame.
  Evidence: `853e290` captures only partial BusyBee-style coordinator frames
  on the focused large-object repro.
- Observation: the captured `trailing_bytes=45` and `trailing_bytes=100` values
  are not malformed single frames.
  Evidence: they decompose cleanly into BusyBee identify and Replicant
  bootstrap traffic sizes from the original BusyBee and Replicant sources.
- Observation: the HyperDex client cannot prepare the first atomic write until
  it has successfully unpacked the coordinator's `hyperdex/config` follow
  payload as `hyperdex::configuration`.
  Evidence: `client::maintain_coord_connection` blocks on
  `replicant_client_cond_follow("hyperdex", "config", ...)`, then unpacks
  `m_config_data` into `configuration` before `get_schema`, `point_leader`,
  and `prepare_funcs` are used.
- Observation: the first concrete mismatch is in the primary-subspace region
  bounds, not in bootstrap or string/datatype metadata.
  Evidence: the original HyperDex builder fills regions with
  `hyperdex::partition(...)`, yielding contiguous hash intervals such as
  `upper=0x03ffffffffffffff` for the first primary region, while Rust currently
  writes `lower=partition` and `upper=partition` in
  `default_legacy_config_encoder`.
- Observation: after `1d6093c`, the region-bound mismatch is no longer the
  active product target.
  Evidence: the interval fix is integrated on `main`, the focused interval
  test passes, and the fast large-object public loop still reports
  `Left ClientGarbage`.
- Observation: the next concrete packed-config mismatch is the ID-allocation
  contract, especially `virtual_server_id`.
  Evidence: Rust starts all packed IDs at `0`, while the original coordinator
  allocates `space`, `subspace`, `region`, and `virtual_server` IDs from one
  shared counter seeded at `1`; the original client treats
  `virtual_server_id()` as the null sentinel and refuses to send if the chosen
  replica returns that default value.
- Observation: the concrete failing key `"large"` does not hit the null
  `virtual_server_id` path.
  Evidence: `CityHash64("large") = 0xe2d4d8f959c0215c`, which maps to primary
  region index `56`, and the current Rust config already gives that region a
  non-null replica tuple `(server_id=1, virtual_server_id=56)`.
- Observation: the next exact pre-daemon contract is the routing header, not
  the atomic body.
  Evidence: after `point_leader`, the original client only allocates a nonce
  and packs `(mt, flags=0, version, vidt, nonce)` before BusyBee send, and the
  original daemon only reaches `process_req_atomic` if that header passes its
  `vidt` and version checks first.
- Observation: the routing-header contract appears internally consistent for
  the concrete failing key on current `main`.
  Evidence: `"large"` already maps to a non-null replica tuple, the current
  Rust config emits matching server-table and replica data from one source, and
  the original daemon-side acceptance checks would accept that header shape for
  a single-daemon cluster.
- Observation: the first atomic-body contract also appears structurally
  consistent on current `main`.
  Evidence: both the original path and Rust lower `put` to `FUNC_SET`
  funcalls, pack `nonce >> key_change`, and use compatible funcall and
  `key_change` field orderings for the large-object put.
- Observation: the first exact daemon-side divergence is missing validation and
  explicit atomic error response semantics.
  Evidence: upstream `replication_manager::client_atomic` validates
  `key_change` against schema and emits `RESP_ATOMIC/NET_BADDIMSPEC` on
  failure, while current Rust goes straight from decode into translation and
  execution without an upstream-equivalent gate.
- Observation: on the cleaned post-`5879fab` baseline, the focused large-object
  failure still does not reach daemon request handling at all.
  Evidence: a manual live cluster with daemon-side capture cleared after
  startup still reproduced `Left ClientGarbage` while recording no daemon
  request frames for the failing path.
- Observation: one exact daemon mismatch is already known but is downstream of
  the current blocker.
  Evidence: current Rust falls through to `ConfigMismatch` for unhandled legacy
  message types and still lacks `ReqGetPartial -> RespGetPartial`, while the
  original producer/consumer contract is `hyperdex_client_get_partial` plus
  `pending_get_partial::handle_message`.
- Observation: the remaining exact mismatch is the Replicant bootstrap
  sender-identity contract, not a later `hyperdex/config` body field.
  Evidence: the new harness probe shows only repeated bootstrap traffic, and
  the original client only leaves bootstrap when the sender token identity and
  encoded `server.id` satisfy the `si == s.id && c.has(s.id)` acceptance check.

## Decision Log

- Decision: keep the first bounded step read-only.
  Rationale: the highest-value missing information is exact coordinator-side
  evidence, and the active product worker already owns the code surface.
  Date/Author: 2026-03-27 / root
- Decision: keep the second bounded step read-only as well.
  Rationale: the next question is still contract comparison, not product-code
  ownership, and the product worker already owns the code surface where fixes
  will land.
  Date/Author: 2026-03-27 / root
- Decision: park this workstream after the region-bound mismatch was identified.
  Rationale: the active product worker now has a precise fix target, so another
  read-only pass is unnecessary until that fix lands and a new mismatch remains.
  Date/Author: 2026-03-27 / root
- Decision: reopen the workstream immediately for one narrower read-only step.
  Rationale: the product target is now precise enough that a source-backed
  interval-and-bytes fixture can materially shorten the next code pass instead
  of leaving the worker to reconstruct the original partition contract alone.
  Date/Author: 2026-03-27 / root
- Decision: keep the workstream active for one more read-only pass after
  `cce-004`.
  Rationale: the ID-allocation mismatch is the next proven divergence, but the
  focused public failure is still keyed to `"large"`, so one more tie-off pass
  is useful while the product worker patches IDs.
  Date/Author: 2026-03-27 / root
- Decision: keep the workstream active for one more step beyond `cce-005`.
  Rationale: the tie-off ruled out route selection for the concrete key, so
  the next high-value read-only question is the next client-side contract after
  route selection, not another packed-config comparison.
  Date/Author: 2026-03-27 / root
- Decision: keep the workstream active for one more focused comparison after
  `cce-006`.
  Rationale: the next contract is now narrow enough to inspect directly in the
  Rust implementation without broadening into daemon-body speculation.
  Date/Author: 2026-03-27 / root
- Decision: move the read-only path one step later after `cce-007`.
  Rationale: the header contract is no longer the most likely blocker for the
  concrete failing key, so the next useful read-only pass is the first body
  contract after an accepted header, not more header analysis.
  Date/Author: 2026-03-27 / root
- Decision: move the read-only path one step later again after `cce-008`.
  Rationale: the large-object put now looks structurally valid through the
  first atomic-body contract, so the next useful read-only question is the
  first daemon-side processing or response contract after `REQ_ATOMIC`.
  Date/Author: 2026-03-27 / root
- Decision: keep the read-only path available for one narrower follow-up only
  if the product fix needs exact validation-rule coverage or response-body
  details.
  Rationale: `cce-009` already names a concrete implementation target, so the
  next root priority is product code rather than another broad read-only pass.
  Date/Author: 2026-03-27 / root

## Outcomes & Retrospective

- The first bounded read-only step finished with a concrete source-backed
  target: the product worker should stop treating the coordinator bootstrap as
  the active mismatch and should instead verify the packed
  `hyperdex::configuration` body returned by the `hyperdex/config` follow reply
  for the full `profiles` schema.
- The second bounded step is now narrower and more useful than generic packet
  decoding: directly compare the Rust packed-config bytes against the original
  HyperDex unpacking rules for the same live `profiles` body.
- That second bounded step produced the first concrete byte-level mismatch:
  primary-region upper bounds are encoded as singleton partition ids instead of
  contiguous partition hash intervals. This workstream can now pause until the
  product fix lands or another packed-config mismatch needs source-backed
  narrowing.
- The third bounded step did its job once the region-interval fix landed. The
  next read-only pass should stay equally narrow, but it now needs to identify
  the next exact packed-config mismatch after the interval correction rather
  than restating the interval contract.
- The fourth bounded step identified that next exact mismatch: zero-based ID
  allocation in the packed config body, with `virtual_server_id=0` as the most
  likely route-preparation blocker. The next read-only pass should not repeat
  general comparison; it should tie that mismatch directly to the failing
  large-object key or move one field deeper if needed.
- The fifth bounded step tied the concrete key to the route-selection path and
  ruled that path out: `"large"` already maps to a non-null replica tuple on
  current `main`. The next read-only pass should therefore move one step later
  in the client path and identify the next exact pre-daemon contract before
  `REQ_ATOMIC` is sent.
- The sixth bounded step identified that next exact contract: the routing
  header and its `virtual_server_id -> server_id -> address` reverse mapping,
  plus the stamped config version that the daemon checks before it will
  classify the frame as `REQ_ATOMIC`.
- The seventh bounded step ruled that routing-header contract out for the
  concrete failing key. The next read-only pass should therefore inspect the
  first body contract after an accepted header rather than revisit config or
  header mechanics.
- The eighth bounded step ruled out that first body contract as well. The next
  read-only pass should inspect the first daemon-side processing or response
  contract after a structurally valid atomic request.
- The ninth bounded step found that contract: missing schema validation and
  missing explicit `RESP_ATOMIC/NET_BADDIMSPEC` handling after atomic decode.
- The product work that followed has now landed both that validation contract
  and the multiprocess `early eof` cleanup on `main`. This workstream can be
  reopened on the remaining large-object failure if product work needs another
  narrow comparison, but it is no longer the active path right now.
