# ADR 0040: Terminal tombstones, import epochs and explicit history pruning

Parent: [Architectural decisions](INDEX.md).

Status: Accepted, 2026-10-17. Implementation is staged under the [execution plan](../tombstone-import-history-compatibility-upgrade-plan-2026-10-17.md); acceptance of this decision does not claim completed runtime support.

## Context

Domain lifecycle, repeated historical imports and physical reclamation need consistent semantics without moving semantic authority into Container or a shared ledger.

## Decision

Tombstones are terminal and domain-owned on existing clocks. Direct ownership closure is the default. Independently identified derived state survives; a cascade names exact additional targets explicitly. A shared lifecycle module supplies vocabulary and idempotency checks only.

Current projection and historical payload availability are separate. Tombstoning retains historical payload. Explicit transformed-copy pruning may destroy selected tombstoned payload only after owner retention proofs; stable identity, ancestry, topology, transaction time, semantic versions and provenance remain. Ordinary reclamation remains conservative.

Every actual import uses owner-local ImportStart and ImportComplete metadata with exact pre/post semantic watermarks. Normal operation needs no markers and no InitialBaseline. An open epoch durably fences unrelated writes. Only import-attributed derived completion and sparse Historical Freshness settlement authorize completion. Import identity belongs to import/CVA ownership; Freshness consumes it. Repeated imports start from the current pre-import projection and never replace pre-existing live history. Legacy CVAFRS03 is decoded at the persisted compatibility boundary only.

New endpoint verification uses compatibility policy 3 and cosine 0.99. Stored policy-2 artifact identity remains immutable. Retirement excludes a profile operationally while retaining historical evidence; numeric destruction requires explicit pruning.

## Consequences

The migration may have temporary red intermediate gates but must end with one epoch/settlement architecture. No universal ledger, extra semantic clock, implicit cascade, singleton-checkpoint fallback or in-place destructive transform is introduced. Reconciliation must preserve tombstones and completed epoch identity, reject unresolved epochs and surface update/delete conflicts.

This amends ADR 0007 for compatibility policy and profile lifecycle and ADR 0039 for historical import identity/settlement ownership. Their unrelated decisions remain applicable.
