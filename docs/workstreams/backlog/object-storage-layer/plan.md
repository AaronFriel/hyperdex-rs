# Workstream Plan: object-storage-layer

This workstream covers object-store-like and erasure-coded storage features.

## Purpose / Big Picture

This is the backlog home for object storage, erasure coding, filesystem-style
metadata layers, and JuiceFS-like product directions.

## Goal

Define the first bounded implementation step for storage-product features that
use the shared placement and durability substrate rather than bypassing it.

## Acceptance Evidence

- The repository has a concrete first implementation step for object-storage
  or erasure-coded storage work.
- The design makes the split between metadata placement and bulk data placement
  explicit.

## Mutable Surface

- storage crates under `crates/**`
- placement and server crates as needed
- supporting docs under `docs/**`

## Dependencies / Blockers

- Best promoted after region semantics, durability policy, and observability
  are stronger.

## Current Hypothesis

The first useful step is probably metadata-first, with the data-placement and
erasure-coding strategy explored as a bounded spike.

## Next Bounded Step

Write the first bounded design for object-storage features, including the split
between metadata and bulk data placement and the first erasure-coding decision
point.
