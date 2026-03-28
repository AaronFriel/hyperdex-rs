# Workstream Plan: graph-vector-layer

This workstream covers richer graph and vector capabilities built above the
shared substrate.

## Purpose / Big Picture

This is the backlog home for graph abstractions, graph-shaped query support,
vector search, HNSW, and alternate ANN architectures.

## Goal

Define the first bounded implementation step for graph or vector capabilities
without inventing a separate storage or consistency model.

## Acceptance Evidence

- The repository has a clear first implementation step for graph or vector
  capability work.
- The design stays tied to the shared placement, durability, and transaction
  semantics.

## Mutable Surface

- `crates/data-model/**`
- `crates/server/**`
- indexing or query crates added only if justified
- supporting docs under `docs/**`

## Dependencies / Blockers

- Best promoted after transactions and region semantics are sharper.

## Current Hypothesis

The first vector step should likely start with HNSW as a baseline while keeping
other ANN designs open. The first graph step should stay at the level of data
model and query abstraction rather than an isolated graph engine.

## Next Bounded Step

Write the first bounded design for graph and vector capability work, including
the first likely HNSW-shaped implementation step and the criteria for choosing
alternate ANN designs later.
