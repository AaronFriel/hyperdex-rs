# hyperdex-rs agent rules

These instructions apply to the entire repository.

## Purpose

This repository exists to deliver a real Rust replacement for HyperDex. Plans,
ledgers, and harnesses are support tools. They are not the product. Progress
must show up primarily as material improvements to the distributed runtime,
public protocol compatibility, storage, placement, transport, or proof systems.

## Material progress

- Do not treat root coordination or ledger churn as progress by itself.
- A root coordination pass should usually reconcile a substantive code result,
  launch multiple substantial workstreams, or record a real strategy change.
- If recent history shows mostly `docs/**` and little product code, stop and
  reorient toward landing code.
- Prefer commits that mainly touch `crates/**`, `proto/**`, or other product
  surfaces over commits that mainly touch `docs/**`.
- Add or update planning files when needed, but piggyback those updates on
  substantive merges whenever possible.

## AutoPlan discipline

- Use AutoPlan as a control system, not as a diary.
- Keep the root AutoPlan, root loop ledger, and workstream files current, but
  do not create routine doc-only commits just to narrate motion.
- Record retries only when the retry meaningfully changes scope, ownership,
  validator, or hypothesis.
- If a workstream produces repeated no-diff or no-evidence outcomes, change the
  assignment or close it. Do not keep recording the same failure shape.

## Delegation

- Delegate aggressively when parallel work can move the repository forward.
- Give forked workers large, meaningful ownership: a bug fix, feature
  increment, end-to-end compatibility step, proof effort, or tightly scoped
  investigation with a concrete conclusion.
- Do not split work so finely that coordination costs more than the code
  landing rate.
- Each active worker must own a clear mutable surface, a main validator, and a
  fastest useful check.
- Prefer letting a worker finish a coherent iteration over interrupting it
  early.

## Validation loops

- Shorten the path from change to evidence before starting a long implementation
  pass.
- Prefer focused tests, small repro harnesses, or narrow protocol probes during
  iteration.
- Use broad validators such as package-wide or workspace-wide test runs at
  merge points, not as the only day-to-day loop.
- Add harness coverage only when it directly helps expose or prove the next
  product change.

## Harnesses and proof code

- Test and harness code should support product work, not substitute for it.
- Do not keep growing a harness file just to demonstrate activity.
- Add a new probe only when it isolates the next failing boundary or proves a
  fix that product code depends on.

## Product bias

- When choosing between another documentation adjustment and a product change,
  bias toward the product change.
- When compatibility work is active, drive the public behavior forward on a
  live system instead of collecting broad speculative notes.
- When deterministic proof work is active, connect it to an observed product
  risk or acceptance requirement.

## Communication

- Be direct about whether a pass landed material code, only moved planning
  state, or found a blocker.
- Do not present planner maintenance as engineering progress.
