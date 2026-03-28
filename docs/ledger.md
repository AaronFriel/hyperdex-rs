# Root Ledger

This is the active root ledger for the current phase of `hyperdex-rs`.

Earlier orchestration history is archived under
[archive/phase-1/ledger.md](/home/friel/c/aaronfriel/hyperdex-rs/docs/archive/phase-1/ledger.md).

| Iteration | Timestamp (UTC) | Action | Evidence | Disposition | Next root move |
| --- | --- | --- | --- | --- | --- |
| 1 | 2026-03-28 10:20Z | Reset the root documentation layout for the post-baseline phase. | The old root package moved to `docs/archive/phase-1`; the active root files are now `docs/autoplan.md` and `docs/ledger.md`; workstreams are split into `active`, `backlog`, and `completed`; and `docs/workstreams.md` becomes the tracked index. | `advance` | Update instructions and gitignore for local worktree tracking, then verify the reorganized docs tree. |
| 2 | 2026-03-28 10:32Z | Added the capability grouping note and reshaped the backlog around dependency order instead of a flat feature list. | `docs/capability-ladder.md` now groups the roadmap into foundation, distributed semantics, programmability, data models/query layers, and storage products; `docs/autoplan.md`, `docs/workstreams.md`, and `docs/future-directions.md` now reference that structure; and backlog workstreams now include programmability, graph/vector, temporal, and object-storage tracks. | `advance` | Keep the active board small, and only promote backlog workstreams when their dependencies and validators are ready. |
