# Workstream Plan: programmability

This workstream explores how `hyperdex-rs` should execute user-defined logic on
or near the database nodes.

## Purpose / Big Picture

This is the backlog home for WASM execution, stored programs, triggers, and
changefeed-adjacent programmable hooks.

## Goal

Define the first bounded implementation step for safe, observable, server-side
programmability.

## Acceptance Evidence

- The repository has a concrete first implementation step for programmable
  execution.
- The first step names safety, scheduling, and resource boundaries explicitly.

## Mutable Surface

- `crates/server/**`
- `crates/transport-core/**`
- new execution-runtime crates if justified
- supporting docs under `docs/**`

## Dependencies / Blockers

- Best promoted after transactions and region semantics are sharper.

## Current Hypothesis

The first step should likely be WASM-first, but only with explicit resource and
observability constraints.

## Next Bounded Step

Write the first bounded design for server-side programmability and decide
whether the initial form should be request-scoped WASM, trigger-style hooks, or
changefeed consumers.
