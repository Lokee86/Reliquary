# Tombstone/import upgrade execution contract

Parent: [Documentation index](INDEX.md).

Baseline: main at 1271192; existing uncommitted benchmark and Ego documentation work must be preserved. Canonical plan: [implementation plan](tombstone-import-history-compatibility-upgrade-plan-2026-10-17.md).

## Purpose

Record the frozen implementation seams and distinguish verified progress from accepted architectural direction.

## Overview

The 50-step plan is staged across isolated lifecycle, Archive, import, Freshness and compatibility lanes. Integration and retained-fixture promotion require direct verification.

## Frozen lifecycle contract

Terminal tombstones belong to the existing domain owner and advance only its existing clock plus the existing global publication clock. The shared lifecycle contract contains typed identities, state, idempotency and payload availability only. It owns no persistence or version allocation.

Direct closure: Archive conversations own their nodes, branch and metadata revisions and attachment exposure; Memory owns its revisions/body exposure; Entity and Relationship own their revisions, with replacement/endpoints retained structurally; CompatibilityProfile owns activation and generation/vector exposure; Ego owns its existing document/anchor lifecycle. Independently identified derived objects survive source deletion. Explicit cascade lists exact selected targets and never traverses provenance implicitly.

## Frozen import contract

Import metadata belongs to import/CVA ownership. Every actual import opens one durable deterministic ImportStart before its first mutation and closes with ImportComplete only after import-attributed work and sparse Freshness settlement are durable. No-import owners have no markers. Open imports fence unrelated writes, including after restart; matching input resumes and conflicting input fails. Markers capture exact existing clocks without allocating semantic tickets.

## Frozen prune contract

Tombstones preserve historical payload. Only an explicit transformed-copy prune may remove selected tombstoned payload after each domain proves its structural retention and surviving-reference obligations. Reopen verification compares clocks, transaction times, identities, topology, activity, import and Freshness history. Source files remain untouched. Ordinary reclaim stays conservative.

## Compatibility contract

Current endpoint verification uses policy 3 and cosine 0.99. Persisted policy-2 artifacts retain their exact identity and bytes. New artifacts use policy 3. Stored artifact validation and current endpoint comparison are separate responsibilities.

## Progress

Implementation in progress. No step is marked complete until repository evidence and its acceptance checks pass.

## Related docs

- [Implementation plan](tombstone-import-history-compatibility-upgrade-plan-2026-10-17.md)
- [ADR 0040](decisions/0040-terminal-tombstones-and-import-epochs.md)
- [Maintainer map](maintainer-map.md)

## Notes

Baseline root revision: 127119239601718ebb0dc95d38075d30fbf07804. Initial root library test attempts were cancelled during cold Lore dependency compilation; no passing baseline is claimed. Direct hashing confirmed all three authoritative REL fixture hashes match recorded acceptance inputs. Existing benchmark/Ego work was preserved.
