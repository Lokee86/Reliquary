# Rebuild workspace setup evidence

Parent index: [Documentation index](INDEX.md)

## Purpose

Record the independent target workspace and its initial dependency graph.

## Overview

The `C:\!bin\workspace\Reliquary-rebuild` orphan worktree on `rebuild/clean-architecture` was created separately from the pinned reference. Its 23 packages correspond to the selected target DAG. A `cargo metadata --format-version 1 --no-deps --offline` result was compared programmatically against every internal edge in the target JSON: 23 package names matched and no edge differences were found. Packages that contain only boundary documentation are incomplete scaffolds, not implemented capabilities. The reference is not a dependency.

## Container slice

The new Container implements the typed 2.1 header and framed I/O, exclusive writer/shared reader leases, explicit repair and bounded streaming validation. Nine focused tests passed using `cargo test -p reliquary-container --locked --offline -j 1`, including old and mixed header rejection without mutation, wrong-kind rejection, partial-append recovery, and a cross-process lease check. Owner sessions, semantic clocks and grouped publication remain downstream work.

## Related docs

- [Rebuild plan](architecture-rebuild-implementation-plan-2026-11-03.md)
- [Target dependency graph](architecture/target-crate-dag-2026-11-03.json)
- [Supported behavior inventory](supported-behavior-inventory.md)

## Notes

This is a graph and isolation check, not storage, product, or release acceptance. Implementations and tests remain in progress.
