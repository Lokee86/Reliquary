# ADR 0040: Terminal tombstones, import epochs and explicit history pruning

Parent: [Architectural decisions](INDEX.md).

Status: Accepted, 2026-10-17. Its lifecycle and import-epoch decisions remain in force. The implementation now uses CVAFRS06 semantic settlements and shared owner-maintenance evidence; PHY pruning and repeated-profile repair are implemented as part of ADR 0041's integration. See [ADR 0041](0041-semantic-import-boundaries-owner-pruning-history-rewrite.md) and the [semantic import/pruning/rewrite evidence](../semantic-import-pruning-history-rewrite-evidence-2026-10-18.md) for the updated implementation and completed Step 65 integration/compatibility acceptance. The older [final verification](../tombstone-import-history-final-verification-2026-10-18.md) records the earlier REL-only boundary.

## Context

Domain lifecycle, repeated historical imports and physical reclamation need consistent semantics without moving semantic authority into Container or a shared ledger.

## Decision

Tombstones are terminal and domain-owned on existing clocks. Direct ownership closure is the default. Independently identified derived state survives; a cascade names exact additional targets explicitly. A shared lifecycle module supplies vocabulary and idempotency checks only.

Current projection and historical payload availability are separate. Tombstoning retains historical payload. Explicit transformed-copy pruning may destroy selected tombstoned payload only after owner retention proofs; stable identity, ancestry, topology, transaction time, semantic versions and provenance remain. Ordinary reclamation remains conservative.

Every actual import uses owner-local ImportStart and ImportComplete metadata with exact pre/post semantic watermarks. Normal operation needs no markers and no InitialBaseline. An open epoch durably fences unrelated writes. Only import-attributed derived completion and sparse Historical Freshness settlement authorize completion. Import identity belongs to import/CVA ownership; Freshness consumes it. Repeated imports start from the current pre-import projection and never replace pre-existing live history. Legacy CVAFRS03/04/05 and CVAILEG1 are read only at the persisted compatibility boundary. Explicit repair may normalize legacy settlements to CVAFRS06 on its disposable working copy; passive open adds no markers. Native start/completion authority is never fabricated for historical bytes.

New endpoint verification uses compatibility policy 3 and cosine 0.99. Stored policy-2 artifact identity remains immutable. Retirement excludes a profile operationally while retaining historical evidence; numeric destruction requires explicit pruning. Active, shared or unknown references block payload destruction. Legacy prefix-bound settlements must be normalized on a working copy before maintenance; after semantic normalization, an unrelated physical offset alone does not prevent eligible destruction. Exact current FragmentId coverage, owner identity and a completed native import boundary are prerequisites for operational vector selection.

## Consequences

The migration may have temporary red intermediate gates but must end with one epoch/settlement architecture. No universal ledger, extra semantic clock, implicit cascade, singleton-checkpoint fallback or in-place destructive transform is introduced. Reconciliation must preserve tombstones and completed epoch identity, reject unresolved epochs and surface update/delete conflicts.

This amends ADR 0007 for compatibility policy and profile lifecycle and ADR 0039 for historical import identity/settlement ownership. Their unrelated decisions remain applicable.
