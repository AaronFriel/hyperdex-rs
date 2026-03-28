# Workstream Ledger: multiprocess-harness

### Entry `mph-001` - Preregistration

- Timestamp: `2026-03-27 04:19Z`
- Kind: `preregister`
- Hypothesis: serializing the three process-spawning multiprocess-harness tests
  will remove the current workspace false failure caused by same-process port
  collisions.
- Owner: `root`; matching isolated worktree result available from paused worker
- Start commit: `2e6490e`
- Worktree / branch:
  - root checkout dirty state
  - `worktrees/dist-multiprocess-harness`
- Mutable surface:
  - `Cargo.toml`
  - `crates/server/Cargo.toml`
  - `crates/server/tests/dist_multiprocess_harness.rs`
- Validator:
  - `cargo test -p server --test dist_multiprocess_harness -- --nocapture`
  - `cargo test --workspace`
- Expected artifacts:
  - green multiprocess harness
  - green workspace
  - one bounded commit on `main`

### Entry `mph-001` - Outcome

- Timestamp: `2026-03-27 04:22Z`
- Kind: `outcome`
- End commit: `98def36`
- Artifact location:
  - `Cargo.toml`
  - `crates/server/Cargo.toml`
  - `crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - `cargo test -p server --test dist_multiprocess_harness -- --nocapture` passed
  - `cargo test --workspace` passed
- Conclusion: the immediate same-process harness collision is contained and the
  workspace is green again.
- Disposition: `advance`
- Next move: preregister the next bounded readiness cleanup in the dedicated
  multiprocess worktree.

### Entry `mph-002` - Preregistration

- Timestamp: `2026-03-27 04:22Z`
- Kind: `preregister`
- Hypothesis: replacing ephemeral port reuse and log-text waits with
  protocol-based readiness checks will keep the multiprocess harness stable
  without further broad serialization.
- Owner: dedicated worker in `worktrees/dist-multiprocess-harness`
- Start commit: `98def36`
- Worktree / branch:
  - `worktrees/dist-multiprocess-harness`
- Mutable surface:
  - `crates/server/tests/dist_multiprocess_harness.rs`
  - `crates/server/src/main.rs` only if the harness truly needs a small startup
    signal change
- Validator:
  - `cargo test -p server --test dist_multiprocess_harness -- --nocapture`
  - `cargo test --workspace`
- Expected artifacts:
  - no same-process port reuse inside multiprocess tests
  - readiness based on observable protocol state rather than log text
  - green multiprocess harness
  - green workspace
  - one bounded commit ready for reconciliation

### Entry `mph-002` - Outcome

- Timestamp: `2026-03-27 04:33Z`
- Kind: `outcome`
- End commit: `faa6cb6`
- Artifact location:
  - `crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - `cargo test -p server --test dist_multiprocess_harness -- --nocapture` passed
  - `cargo test --workspace` passed
- Conclusion: the multiprocess harness now uses held port reservations and
  protocol-based readiness checks, so it no longer depends on ephemeral port
  reuse or log-text polling.
- Disposition: `advance`
- Next move: hold until a new real-cluster failure requires another harness
  change.

### Entry `mph-003` - Preregistration

- Timestamp: `2026-03-27 07:10Z`
- Kind: `preregister`
- Hypothesis: a targeted coordinator-plus-daemon admin probe harness test can
  give the live compatibility workstream a much faster and more repeatable
  signal than the current manual free-port probe sequence, without touching
  product code.
- Owner: forked worker in
  `worktrees/admin-probe-harness`
- Start commit: `4ccf113`
- Worktree / branch:
  - `worktrees/admin-probe-harness` on
    `admin-probe-harness`
- Mutable surface:
  - `Cargo.toml`
  - `crates/server/Cargo.toml`
  - `crates/server/tests/**`
  - `crates/server/src/main.rs` only if the harness truly needs a small
    readiness adjunct
- Validator:
  - fastest useful check: focused `dist_multiprocess_harness` target for the
    new admin probe
  - strong checks:
    - `cargo test -p server --test dist_multiprocess_harness -- --nocapture`
    - `cargo test --workspace`
- Expected artifacts:
  - a fast coordinator-plus-daemon admin probe harness
  - captured evidence about whether the C admin client advances beyond
    bootstrap
  - one bounded commit ready for reconciliation

### Entry `mph-003` - Outcome

- Timestamp: `2026-03-27 07:20Z`
- Kind: `outcome`
- End commit: `6f061b3`
- Artifact location:
  - `crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - `cargo test -p server --test dist_multiprocess_harness legacy_admin_wait_until_stable_probe_reports_bootstrap_progress -- --nocapture` passed
  - the focused test reports `advanced=false` on current `main`
  - the captured frame summary from that test shows the first meaningful
    server reply is still `ClientResponse`
  - `cargo test -p server --test dist_multiprocess_harness -- --nocapture`
    passed
  - `cargo test --workspace` passed
- Conclusion: the fast admin-probe harness is now on `main`, and it provides
  the shortest trustworthy check for whether the original C admin client
  progresses beyond bootstrap.
- Disposition: `advance`
- Next move: hold until the product worker or another real-cluster failure
  needs more harness work.

### Entry `mph-004` - Preregistration

- Timestamp: `2026-03-27 07:35Z`
- Kind: `preregister`
- Hypothesis: a focused process-level or selected-client-path reproducer for
  the new daemon-side `ClientGarbage` failure can shorten the feedback loop
  materially compared with the current selected `hyhac` command.
- Owner: forked worker in
  `worktrees/admin-probe-harness`
- Start commit: `c087f81`
- Worktree / branch:
  - `worktrees/admin-probe-harness` on
    `admin-probe-harness`
- Mutable surface:
  - `Cargo.toml`
  - `crates/server/Cargo.toml`
  - `crates/server/tests/**`
  - `/home/friel/c/aaronfriel/hyhac/scripts/**` only if a tiny focused helper
    is strictly necessary for the shorter repro
- Validator:
  - fastest useful check: focused repro target for the first `ClientGarbage`
    path
  - strong checks:
    - `cargo test -p server --test dist_multiprocess_harness -- --nocapture`
    - `cargo test --workspace`
- Expected artifacts:
  - a faster reproducer for the first daemon-path `ClientGarbage` failure
  - clear evidence about the first bad request/response pair on that path
  - one bounded commit ready for reconciliation

### Entry `mph-004` - Outcome

- Timestamp: `2026-03-27 07:45Z`
- Kind: `outcome`
- End commit: `0b2379d`
- Artifact location:
  - `crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_hits_clientgarbage_fast -- --nocapture` passed
  - that focused repro reaches `Left ClientGarbage` in about `107ms`
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_pooled_probe_reports_large_object_failure_first -- --nocapture` passed
  - the broader pooled probe shows the large-object case fails before later
    pooled failures
  - `cargo test -p server --test dist_multiprocess_harness -- --nocapture`
    passed
  - `cargo test --workspace` passed
- Conclusion: the first daemon-path public failure is now reproducible with a
  much shorter check than the earlier selected `hyhac` command.
- Disposition: `advance`
- Next move: hold until the product worker or another real-cluster failure
  needs a tighter repro.

### Entry `mph-005` - Preregistration

- Timestamp: `2026-03-27 07:50Z`
- Kind: `preregister`
- Hypothesis: extending the new large-object `ClientGarbage` repro to capture
  the first bad legacy daemon request/response pair will give the product
  worker a stronger target than the public failure string alone.
- Owner: forked worker in
  `worktrees/clientgarbage-probe`
- Start commit: `0b2379d`
- Worktree / branch:
  - `worktrees/clientgarbage-probe` on
    `clientgarbage-probe`
- Mutable surface:
  - `Cargo.toml`
  - `crates/server/Cargo.toml`
  - `crates/server/tests/**`
  - `/home/friel/c/aaronfriel/hyhac/scripts/**` only if a tiny focused helper
    is strictly necessary
- Validator:
  - fastest useful check: focused large-object repro with added capture or
    summary
  - strong checks:
    - `cargo test -p server --test dist_multiprocess_harness -- --nocapture`
    - `cargo test --workspace`
- Expected artifacts:
  - a fast repro that also reports the first bad request/response edge
  - clearer evidence for the daemon-path product worker
  - one bounded commit ready for reconciliation

### Entry `mph-005` - Outcome

- Timestamp: `2026-03-27 19:49Z`
- Kind: `outcome`
- End commit: `d12c23c`
- Artifact location:
  - `worktrees/clientgarbage-probe`
- Evidence summary:
  - the interrupted worker is no longer active
  - `git status --short --branch` in the old worktree shows unrelated edits in
    `crates/consensus-core/src/lib.rs`, `crates/data-model/src/lib.rs`,
    `crates/engine-memory/src/lib.rs`, `crates/hyperdex-admin-protocol/src/lib.rs`,
    `crates/legacy-frontend/src/lib.rs`, `crates/legacy-protocol/src/lib.rs`,
    `crates/server/src/lib.rs`, and `crates/simulation-harness/src/lib.rs`
  - no bounded harness-only commit was produced
- Conclusion: the first wire-capture retry did not stay inside its owned
  harness surface, so it cannot be reconciled safely and must be replaced on a
  clean worktree.
- Disposition: `retry`
- Next move: preregister and launch the same wire-capture goal on a fresh
  `clientgarbage-wire` worktree from clean `main`.

### Entry `mph-006` - Preregistration

- Timestamp: `2026-03-27 19:49Z`
- Kind: `preregister`
- Hypothesis: repeating the same large-object wire-capture goal on a fresh
  clean worktree will produce bounded harness evidence without polluting
  product files outside the harness-owned surface.
- Owner: forked worker in
  `worktrees/clientgarbage-wire`
- Start commit: `d12c23c`
- Worktree / branch:
  - `worktrees/clientgarbage-wire` on
    `clientgarbage-wire`
- Mutable surface:
  - `Cargo.toml`
  - `crates/server/Cargo.toml`
  - `crates/server/tests/**`
  - `/home/friel/c/aaronfriel/hyhac/scripts/**` only if a tiny focused helper
    is strictly necessary
- Validator:
  - fastest useful check: `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_hits_clientgarbage_fast -- --nocapture`
  - strong checks:
    - `cargo test -p server --test dist_multiprocess_harness -- --nocapture`
    - `cargo test --workspace`
- Expected artifacts:
  - a fast repro that also reports the first bad request/response edge
  - clearer daemon-path evidence for the product worker
  - one bounded harness-only commit ready for reconciliation

### Entry `mph-006` - Outcome

- Timestamp: `2026-03-27 19:54Z`
- Kind: `outcome`
- End commit: `ad458f1`
- Artifact location:
  - no code changes in `worktrees/clientgarbage-wire`
- Evidence summary:
  - the completed worker verified that `main` is at `ad458f1`
  - the completed worker rechecked the fast validator and confirmed the same
    `Left ClientGarbage` failure still reproduces
  - `git status --short --branch` in the worktree remained clean
  - no harness code or new daemon-path evidence was produced
- Conclusion: the clean replacement worktree removed drift risk, but this
  attempt still stopped at baseline verification rather than the owned wire
  capture goal.
- Disposition: `retry`
- Next move: relaunch the harness workstream on the same clean worktree with a
  stricter requirement to expose or decode the first bad daemon frame.

### Entry `mph-007` - Preregistration

- Timestamp: `2026-03-27 19:54Z`
- Kind: `preregister`
- Hypothesis: a sharper harness-only attempt that must either expose the first
  bad daemon frame directly in test output or land a harness commit that
  decodes it will give the product worker actionable wire evidence without
  touching product code.
- Owner: next forked worker in
  `worktrees/clientgarbage-wire`
- Start commit: `ad458f1`
- Worktree / branch:
  - `worktrees/clientgarbage-wire` on
    `clientgarbage-wire`
- Mutable surface:
  - `Cargo.toml`
  - `crates/server/Cargo.toml`
  - `crates/server/tests/**`
  - `/home/friel/c/aaronfriel/hyhac/scripts/**` only if a tiny focused helper
    is strictly necessary
- Validator:
  - fastest useful check: `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_hits_clientgarbage_fast -- --nocapture`
  - strong checks:
    - `cargo test -p server --test dist_multiprocess_harness -- --nocapture`
    - `cargo test --workspace`
- Expected artifacts:
  - either a harness commit that exposes or decodes the first bad daemon frame,
    or a clean proof tied to test output that identifies the bad edge exactly
  - clearer daemon-path evidence for the product worker
  - one bounded harness-only commit ready for reconciliation

### Entry `mph-007` - Outcome

- Timestamp: `2026-03-27 19:54Z`
- Kind: `outcome`
- End commit: `853e290`
- Artifact location:
  - `crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - `853e290` adds a coordinator proxy capture path and the focused test
    `legacy_hyhac_large_object_probe_reports_first_coordinator_frame_pair`
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_hits_clientgarbage_fast -- --nocapture` passed
  - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reports_first_coordinator_frame_pair -- --nocapture` passed
  - `cargo test -p server --test dist_multiprocess_harness -- --nocapture` passed
  - `cargo test --workspace` passed in the worktree
  - the focused large-object repro never reaches a decodable legacy daemon
    frame; the first captured exchange is on the coordinator connection, where
    both directions are partial BusyBee-style frames
  - client-to-coordinator frames end with `trailing_bytes=45` and raw prefix
    `80 00 00 14 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 05`
  - coordinator-to-client frames end with `trailing_bytes=100` and raw prefix
    `80 00 00 14 00 00 00 00 00 00 00 01 00 00 00 00 00 00 00 00 00 00 00 3c`
- Conclusion: the first bad edge on the large-object `ClientGarbage` path is
  earlier than the legacy daemon request/response layer the harness can decode.
  The next useful product diagnosis is coordinator-side BusyBee/Replicant
  framing, not more daemon-frame capture.
- Disposition: `advance`
- Next move: hand the coordinator-frame evidence to the product worker and hold
  this workstream until a new harness change is needed.

### Entry `mph-008` - Preregistration

- Timestamp: `2026-03-27 21:30Z`
- Kind: `preregister`
- Hypothesis: on the cleaned post-`5879fab` baseline, a new harness-only pass
  can expose or decode the first bad client-visible response or wire edge for
  `legacy_hyhac_large_object_probe_hits_clientgarbage_fast` without touching
  product code.
- Owner: next forked worker in
  `worktrees/clientgarbage-wire`
- Start commit: `4902f03`
- Worktree / branch:
  - `worktrees/clientgarbage-wire` on
    `clientgarbage-wire`
- Mutable surface:
  - `Cargo.toml`
  - `crates/server/Cargo.toml`
  - `crates/server/tests/**`
  - `/home/friel/c/aaronfriel/hyhac/scripts/**` only if a tiny focused helper
    is strictly necessary
- Validator:
  - fastest useful check:
    `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_hits_clientgarbage_fast -- --nocapture`
  - strong checks:
    - `cargo test -p server --test dist_multiprocess_harness -- --nocapture`
    - `cargo test --workspace`
- Expected artifacts:
  - a harness-only commit that exposes or decodes the next bad client-visible
    response or wire edge on the large-object repro, or a precise clean proof
    tied to test output that names that edge exactly
  - clearer failure evidence for the active product and read-only workers
  - one bounded harness-only commit ready for reconciliation

### Entry `mph-008` - Outcome

- Timestamp: `2026-03-27 21:42Z`
- Kind: `outcome`
- End commit: `69d5918`
- Artifact location:
  - `crates/server/tests/dist_multiprocess_harness.rs`
- Evidence summary:
  - `69d5918` adds
    `legacy_hyhac_large_object_probe_reports_coordinator_busybee_sequence`
  - root verified on integrated `main`:
    - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reports_coordinator_busybee_sequence -- --nocapture`
    - `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_hits_clientgarbage_fast -- --nocapture`
  - the fast Hyhac failure still reports `Left ClientGarbage`
  - the new harness probe shows only repeated coordinator identify/bootstrap
    traffic on that path and no non-bootstrap coordinator or daemon messages
  - the worker report mentioned a raw-protocol live-cluster test, but that test
    did not land in `69d5918`; root reconciled only the commit contents and the
    validators actually present on `main`
- Conclusion: the cleaned-baseline Hyhac failure is now harness-proven to stop
  inside the coordinator/client-visible bootstrap-follow loop. The immediate
  next debugging work belongs to product code and read-only coordinator
  comparison, not more harness instrumentation.
- Disposition: `advance`
- Next move: hand this evidence to the product and read-only workers and hold
  this workstream until another harness change is justified.

### Entry `mph-009` - Preregistration

- Timestamp: `2026-03-28 00:35Z`
- Kind: `preregister`
- Hypothesis: a harness-owned repro-reduction pass can shorten the honest
  full-schema Hyhac failure into a smaller truthful post-success probe without
  drifting into product implementation.
- Owner: delegated worker `019d31bc-eb2a-7f40-b735-2f35a49b2c12` (`Russell`)
- Start commit: `ace4050`
- Worktree / branch:
  - `worktrees/post-success-repro` on
    `post-success-repro`
- Mutable surface:
  - `Cargo.toml`
  - `crates/server/Cargo.toml`
  - `crates/server/tests/**`
  - `/home/friel/c/aaronfriel/hyhac/scripts/**` only if a tiny focused helper
    is strictly necessary for the repro
- Validator:
  - fastest useful check:
    `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reaches_daemon_after_full_profiles_setup -- --nocapture`
  - expected narrower truthful checks added by the worker if they materially
    shorten the loop
  - strong checks:
    - `cargo test -p server --test dist_multiprocess_harness -- --nocapture`
    - `cargo test --workspace`
- Expected artifacts:
  - a smaller truthful post-success probe for the active live failure
  - no product-code drift outside the harness-owned surface
  - one bounded harness commit ready for reconciliation if the shorter repro is
    worth keeping

### Entry `mph-009` - Outcome

- Timestamp: `2026-03-28 00:46Z`
- Kind: `outcome`
- End commit: `df25106`
- Artifact location:
  - no reconciled code result
  - clean worktree at `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/post-success-repro`
- Evidence summary:
  - the worker returned root-status narration instead of performing harness work
  - `git status` in the worktree remained clean
  - there is no new repro reduction or proof from this attempt
- Conclusion: the assignment is still useful, but the execution shape was bad.
  The retry should use a fresh-context worker with a self-contained prompt so
  it cannot confuse root state with harness ownership.
- Disposition: `retry`
- Next move: relaunch the same bounded harness step from the same clean
  worktree with a fresh-context prompt.

### Entry `mph-010` - Preregistration

- Timestamp: `2026-03-28 00:46Z`
- Kind: `preregister`
- Hypothesis: the harness step will make progress if it is relaunched as a
  fresh-context worker with a self-contained prompt and the same clean
  worktree.
- Owner: next fresh-context worker in
  `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/post-success-repro`
- Start commit: `df25106`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/post-success-repro` on
    `post-success-repro`
- Mutable surface:
  - `Cargo.toml`
  - `crates/server/Cargo.toml`
  - `crates/server/tests/**`
  - `/home/friel/c/aaronfriel/hyhac/scripts/**` only if a tiny focused helper
    is strictly necessary for the repro
- Validator:
  - fastest useful check:
    `cargo test -p server --test dist_multiprocess_harness legacy_hyhac_large_object_probe_reaches_daemon_after_full_profiles_setup -- --nocapture`
  - expected narrower truthful checks added by the worker if they materially
    shorten the loop
  - strong checks:
    - `cargo test -p server --test dist_multiprocess_harness -- --nocapture`
    - `cargo test --workspace`
- Expected artifacts:
  - a smaller truthful post-success probe, or a clean proof that the current
  honest probe is already the smallest worthwhile loop
  - no product-code drift outside the harness-owned surface

### Entry `mph-010` - Outcome

- Timestamp: `2026-03-28 01:02Z`
- Kind: `outcome`
- End commit: `94b13c5`
- Artifact location:
  - no reconciled harness code result
  - interrupted worker state replaced by a new broader truthful boundary
- Evidence summary:
  - the fresh-context worker was interrupted after the product fix changed the
    baseline
  - the old assignment was no longer the right question once the full-schema
    large-object path passed on integrated `main`
  - a direct live full-schema pooled run then showed the next honest failure:
    `roundtrip` fails first with `ClientReconfigure`
- Conclusion: the harness workstream should stop trying to shorten the cleared
  large-object path and should instead target the new full-schema pooled
  `ClientReconfigure` boundary.
- Disposition: `reframe`
- Next move: preregister one new harness pass against the first full-schema
  pooled `ClientReconfigure` failure.

### Entry `mph-011` - Preregistration

- Timestamp: `2026-03-28 01:02Z`
- Kind: `preregister`
- Hypothesis: a harness-owned pass can reduce the first remaining truthful
  full-schema pooled atomic failure to a smaller truthful repro without
  drifting into product implementation.
- Owner: delegated worker `019d31ce-0ba4-7d51-bb1a-347bd18dad3d` (`Bernoulli`)
  in `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/full-schema-roundtrip-repro`
- Start commit: `94b13c5`
- Worktree / branch:
  - `/home/friel/c/aaronfriel/hyperdex-rs/worktrees/full-schema-roundtrip-repro`
    on `full-schema-roundtrip-repro`
- Mutable surface:
  - `Cargo.toml`
  - `crates/server/Cargo.toml`
  - `crates/server/tests/**`
  - `/home/friel/c/aaronfriel/hyhac/scripts/**` only if a tiny focused helper
    is strictly necessary for the repro
- Validator:
  - fastest useful check:
    the new focused truthful full-schema pooled repro if landed
  - strong checks:
    - `cargo test -p server --test dist_multiprocess_harness -- --nocapture`
    - `cargo test --workspace`
- Expected artifacts:
  - a focused truthful repro for the first remaining pooled atomic failure, or
    a clean proof that the broader full-schema pooled loop is already the right
    one
  - no product-code drift outside the harness-owned surface
