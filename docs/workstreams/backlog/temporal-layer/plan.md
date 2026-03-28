# Workstream Plan: temporal-layer

This workstream covers temporal visibility and time-travel features.

## Purpose / Big Picture

This is the backlog home for timestamp-driven historical visibility, temporal
encoding, and time-travel query semantics.

## Goal

Define the first bounded implementation step for temporal features on top of
the current data model and future transaction semantics.

## Acceptance Evidence

- The repository has a concrete first implementation step for temporal
  semantics.
- The design distinguishes between simple timestamp fields and real historical
  visibility semantics.

## Mutable Surface

- `crates/data-model/**`
- `crates/server/**`
- storage crates if historical retention semantics require them
- supporting docs under `docs/**`

## Dependencies / Blockers

- Best promoted after transactions and version semantics are clearer.

## Current Hypothesis

Basic timestamp fields may be easy, but real time-travel queries likely need a
more explicit transaction and storage-version model.

## Next Bounded Step

Write the first bounded design for temporal semantics, separating schema-level
timestamp support from true historical query behavior.
