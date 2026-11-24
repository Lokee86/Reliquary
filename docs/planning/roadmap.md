# Current planning owner

Parent index: [Planning](./INDEX.md)

## Purpose

Keep future corrections separate from implemented product behavior and dated acceptance evidence.

## Overview

The current public contract is owned by [API](../public-product-api.md), [CLI](../reference/cli.md), [configuration](../configured-runtime.md) and [architecture](../architecture/INDEX.md). Historical completion of individual rebuild gates does not establish future capability completeness.

## Current status and planned ownership

The [homogeneous-REL and dependency-DAG roadmap](../rel-homogeneity-and-dependency-roadmap-2026-11-10.md) records the broader architecture correction. The [REL dependency port specification](rel-dependency-port-spec-2026-11-18.md) and [execution plan](rel-dependency-port-plan-2026-11-18.md) cover the durable 2.1 declaration, authorized resolution and ambient context. The [acceptance record](../acceptance/rel-dependency-port-2026-11-18.md) records the implemented dependency behavior and preservation of existing owner identity and headers.

The [standards remediation specification](../engineering-standards-remediation-spec-2026-11-10.md) governs ES01–ES06. [Final local verification](../acceptance/es06-local-verification-2026-11-11.md) records completed local structural, policy, behavioral and independent source-to-document checks. Hosted CI and formal adoption remain unverified; no future product capability is inferred from this remediation.

Manual community recomputation follows the [Leiden specification](leiden-community-recomputation-spec-2026-11-18.md) and [execution plan](leiden-community-recomputation-plan-2026-11-18.md). The operator runs preview and apply through the existing REL management boundary.

Core discovers Entity pairs across authorized RELs from matching names, aliases and structured identifiers in Memories that mention the endpoints. It records and revisits their judgments, and callers can list pairs by peer or request a specific comparison. The 28-day and Ellis copies produced zero candidates with these signals. Retrieval of pairs that share only a description, retained-corpus calibration and Perception Observations, including Relationship Observations, remain future work. The cost of rebuilding discovery indexes under sustained writes on a year-scale REL has yet to be measured. [Acceptance results](../acceptance/cross-rel-entity-identity-2026-11-23.md) · [Comparison specification](cross-rel-entity-identity-spec-2026-11-23.md)

Core supports explicit, reversible operator confirmation for a selected cross-REL pair. Enabling the action in Hermes requires a trusted operator interaction and its own host verification. [Public API](../public-product-api.md#confirm-or-undo-separation)

## Shared Memory Web audit

Ego and Perception will use the same audit code for bounded batches, coverage of selected Memories, optional authorized REL search and source references. Each crate supplies its own extraction contract. Ego extracts candidate subjects for summaries. Perception extracts candidate propositions for evidence review before publication, including Relationships between Entities.

Perception periodically includes a small, rotating sample of Stale and Dormant Memories directly in its audit batches, even when recent Memories could fill the budget. Either feature can search for older evidence when following a lead. [Historical Ego audit plan](https://github.com/Lokee86/Reliquary/blob/2b5bd0eabfbac0a85f899d7d730220b5e8e93efa/docs/ego-service-crate-extraction-spec-2026-10-28.md#L55-L65)

## Implementation sequence and acceptance

The [homogeneity specification](rel-homogeneity-spec-2026-11-18.md) and [execution plan](rel-homogeneity-plan-2026-11-18.md) cover scope-free Core creation, Hermes role admission and legacy file compatibility. The [Core acceptance record](../acceptance/rel-homogeneity-2026-11-18.md) records the creation cut and original header verification.

For standards remediation: restore inventory → canonical docs → coverage/status → architecture review/enforcement → continuous checks → independent final acceptance. The spec contains the full criteria. An unavailable required gate remains an open item, never a successful check.

## Offline owner history migration

The [migration specification](owner-history-migration-spec-2026-11-18.md) and [execution plan](owner-history-migration-plan-2026-11-18.md) recover the offline converter and verifier and define the first proof on copies of the original 28-day REL and PHY. Both output files use native 2.1 admission.

The original pair passed independent retained-history comparison and native CLI, historical-read and grant checks. [Paired acceptance evidence](../acceptance/owner-history-migration-first28d-2026-11-18.md)

The original 14-day and Ellis pairs passed separate offline migrations with the same verification. [14-day and Ellis acceptance evidence](../acceptance/owner-history-migration-other-pairs-2026-11-19.md)

## Owner open performance

Buffered physical reads and reusable replay payloads reduced open time for all six 2.1 owners below their 2.0 references in the matched Windows comparison. Exact incomplete-tail recovery passed on both versions of the 28-day pair. Peak working set remains above the G5 limit for all six owners. [Measured comparison](../acceptance/g5-owner-open-and-recovery-2026-11-20.md)

The local Hermes host admits each mounted owner once. Core can reopen a REL from a verified checkpoint and read older history on demand. On the synthetic year REL, store-specific replay reduced median first signed attach from 16.48 s to 5.61 s. [Checkpoint and mount specification](rel-checkpoint-open-spec-2026-11-20.md) · [Implementation plan](rel-checkpoint-open-plan-2026-11-20.md) · [Acceptance](../acceptance/rel-checkpoint-open-2026-11-23.md)

**Open follow-up — cold Hermes attach on the migrated 28-day pair.** With Core `d9ea3d49`, median logical reads fell from 2.323 to 1.553 GiB, while median elapsed time went from 2.63 to 2.82 s and median host peak working set rose from 192.4 to 211.6 MiB. Elapsed ranges overlapped across five fresh attaches per build on a busy Windows host. Profile where the host spends time and reaches peak memory during first attach. Compare against the recorded baseline on fresh copies with an empty REL checkpoint and verify the same owner cuts and historical Memory body. [Matched measurements and fixture details](../acceptance/rel-checkpoint-open-2026-11-23.md#first-attach-with-store-specific-replay)

## Open decisions

The optional free-form REL type label remains separate from the creation and role admission work. Cross-owner and PHY reconciliation, additional provider surfaces, and a deployed daemon require their own specification and acceptance.

## Related docs

- [Limits](../limits/current-limitations.md)
- [Document disposition](../development/document-disposition.md)
- [System](../architecture/system.md)

## Notes

Graduate implemented facts into their current owners and leave only unresolved work here. Do not present a historical specification as a current implementation guarantee.
