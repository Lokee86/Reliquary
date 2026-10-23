# Phase 3 to Phase 4 Project Environment handoff

Date: 2026-10-23
Status: Prepared; final Phase 3 C3/D3/A3 acceptance pending.

## Purpose

Carry the concrete ownership, physical-access and current-format constraints from Phase 3 into the Project Environment phase without adding later roadmap features.

## Overview

The [Phase 3 ledger](dependency-convergence-phase3-execution-2026-10-22.md) is the authority for task and gate acceptance. This handoff does not independently accept an unfinished gate. The original target remains the unchanged 34-component, 163-edge DAG. The next phase must preserve its downward dependencies and default-deny source enforcement.

Phase 3 separates read-only History proofs, Maintenance policy, Import epoch coordination, concrete owner lifecycle and physical Container mechanics. Supported public Cva and Phylactery methods remain outward adapters. Freshness owns the single CVAFRS07 semantic settlement codec and canonical chronological scorer.

## Ownership carried forward

| Responsibility | Canonical owner and constraint |
| --- | --- |
| Physical identity, extent, framing, leases, clocks, append and sync | Container; cross-owner services use scoped concrete lifecycle capabilities. Do not move semantic selection or policy into Container. |
| Concrete REL/PHY reconstruction, physical copy and publication transport | Owner lifecycle; physical access remains private and scoped, with no upper service callback or raw handle escape. |
| Historical cuts, retained proof composition and inspection | History; immutable sources and owner reads only. Inspection cannot publish, reinforce accepted use or acquire a writer. |
| Prune, reclaim, repair, rewrite and generic divergent reconciliation policy | Maintenance; producers validate their own semantic evidence and retained payload eligibility. |
| Import start, readiness, preparation, completion and epoch reconciliation | Import; service coordination invokes lower producers and immutable History views. It has no Maintenance dependency. |
| Freshness numeric authority and accepted event evidence | Freshness store; the service is an adapter over one canonical scorer and one current codec. Journal event identity, digest, turn and provenance remain exact. |
| Public Cva/Phylactery/CLI surfaces | Supported adapters; preserve their signatures, refusal order, source/output lease checks and exact failure cleanup. |

Phase 4 must not recover old routes through facade internals or introduce a generic store/session/root framework. Project or environment facts should cross boundaries as concrete immutable values with stable IDs; the referring owner retains their semantics. Add no ownership exception merely because two stores need similar physical mechanics.

## Format and evidence constraints

CVAFRS07 retains exact scope, accepted journal event references, chronological causal inputs and sparse irreducible accepted score corrections. It stores no copied per-event recipient sets or complete derived output record maps. Retained journal lookup must remain owned by Freshness, and the existing start/completion identity and exact payload-digest checks remain mandatory.

The numeric basis is independent of removable source/vector payload and permanent physical offsets. Prune, reclamation and repacking must preserve the retained evidence needed to resolve it. Rewrite remains an explicit generation boundary with the existing lineage policy. The 64 MiB expanded UTF-8 limit bounds dictionary and reference expansion; count, index, ordering, canonical-integer, checksum and trailing-byte refusal remain part of the current wire contract.

Previously accepted numeric scores cannot be silently regenerated as replacement authority. The frozen corpus demonstrates 2,165 score differences under current regeneration. The format preserves those exact differences and original scope digests. Tests compare new physical hashes against the same immutable historical semantic oracle, rather than recapturing expected semantics.

## Required continuation safeguards

- Keep the original logical DAG and prior phase baselines as historical evidence. Current graph evidence describes the final source; do not erase prior debt provenance by recapturing its baseline.
- Run the physical, owner, read-only History and negative source fixtures for ownership changes. Source counts or acyclicity alone do not prove a permitted dependency.
- Preserve accepted owner cuts, local/global versions, optional transaction times, provenance, attribution and exact retry behavior.
- Keep inspection byte- and modification-time pure. Publish only through the existing checked append/sync sequence, and validate source identity again where the existing operation requires it.
- Keep fixture before-images and format conversion receipts distinct from semantic oracles. A current-format corpus runner may opt into a verified conversion receipt; it may not replace historical gold or older frozen pins.

The final Phase 3 acceptance receipt and stabilized source graph are the starting evidence for Phase 4. Until their gates close in the ledger, this document is a prepared handoff rather than an authorization to implement new features.

## Exclusions

This handoff adds no sync protocol, Observations, Ego/Context behavior, Gateway operation or repository VCS feature. Real-model inference and unrelated calibration fixtures are not Phase 3 acceptance requirements. Project Environment convergence needs its own bounded implementation and verification plan.

## Related docs

- [Parent architecture campaign](dependency-convergence-architecture-refactor-2026-10-21.md)
- [Phase 3 specification](dependency-convergence-phase3-history-maintenance-import-spec-2026-10-22.md)
- [Phase 3 execution ledger](dependency-convergence-phase3-execution-2026-10-22.md)
- [Selected current settlement format](dependency-convergence-phase3-settlement-format-2026-10-23.md)
- [Accepted component DAG](dependency-convergence-target-dag-2026-10-21.md)
- [Architecture](architecture.md)
- [Maintainer map](maintainer-map.md)

## Notes

Task completion and acceptance are recorded only with actual test, corpus, source, deletion and documentation evidence. Preserve unrelated working-copy changes when preparing or verifying the tracked release tree.
