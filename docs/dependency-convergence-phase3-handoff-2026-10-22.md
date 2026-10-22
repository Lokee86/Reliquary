# Phase 3 owner-contract handoff

Parent index: [Documentation index](INDEX.md)

## Purpose

Identify concrete interfaces for the deferred History, Maintenance and Import/Reconciliation work. This is an implementation handoff, not Phase 2 acceptance. Acceptance remains in the [execution ledger](dependency-convergence-phase2-execution-2026-10-22.md).

## Overview

Phase 3 composes accepted owner facts above the stores. It must preserve the accepted DAG, REL/PHY privacy, historical integrity and V2 formats. Historical reconstruction, maintenance policy, reconciliation composition and CVAFRS06 settlement minimization remain Phase 3 work. Observation synthesis, Ego generation, Context Engine assembly and External Sources redesign remain later roadmap work.

| Owner | Concrete seam | Composition obligation |
| --- | --- | --- |
| Archive | `retention_proof`, `pruned_content_markers`, `file_memory_link_records`, existing historical source reads | Match Maintenance manifests and Memory targets above Archive; preserve source history and terminal coordinates. |
| Memory | `retention_proof`, `pruned_body_markers`, `validate_provenance_fields`, `retained_body_bytes`, accepted `GroupedBodySpan` inputs | Resolve Archive provenance above Memory. Outer Insomnia/External envelopes remain decoded by their existing author or lifecycle caller. Memory validates its own body bytes and hash. |
| Perception | Entity/Resolution/Relationship owner records and local validation; owner-service reference admission | Resolve foreign Memory/Entity references in the existing authorized author. Do not make replay depend on a mutable facade. |
| Graph | `semantic_nodes`, local rebuild and publication operations | Compose endpoint and principal admission in `facade/graph_endpoint_admission.rs`; preserve current exclusion and historical existence semantics. |
| Freshness | Accepted event/proof values, exact requested import-event reads and existing settlement records | History and reconciliation consume accepted identities and coordinates. The stored historical policy digest and event value are owned by `freshness/historical_contract.rs`. CVAFRS06 remains unchanged. |
| Vectors | Pending referenced IDs, accepted body-ID sets, accepted fragment-version maps and Archive version limit | Lifecycle supplies immutable foreign-owner facts. Owner rebuild retains record validation order and local profile/matrix/binding checks. Pure stored-profile validation lives in `compatibility_profile_validation.rs`; provider calls stay in the higher probe service. |
| Dream/Insomnia | Typed Dream replay entries, Insomnia episode-source values and completion receipts | Existing higher authors compose Memory publication and replay transport. Scheduling execution and provider calls remain above durable work state. |
| Import | `ImportEpochStore` status reads and owner-local epoch records | The completion coordinator remains responsible for readiness, receipt and settlement admission. Its nested module has access to the store's private completion append; stores do not accept a facade token. |
| Maintenance | Accepted manifest storage in `src/maintenance/` | `facade/owner_history_proof.rs` composes the unchanged source-history digest and cross-owner evidence. Repair, prune eligibility and rewrite publication remain explicit higher operations. |

The interfaces above are crate-internal unless already public. They do not authorize direct store exposure, a generic replay callback, shared coordinator state or new durable evidence.

## Related docs

- [Phase 2 specification](dependency-convergence-phase2-owner-local-boundaries-spec-2026-10-22.md)
- [Owner contracts](dependency-convergence-phase2-contracts-2026-10-22.md)
- [Architecture](architecture.md)
- [Maintainer map](maintainer-map.md)

## Notes

Phase 3 begins only after the blocking Phase 2 gates pass. The original Phase 0 graph and reviewed debt remain evidence; a source refresh does not authorize recapturing the baseline or accepting new unreviewed edges.
