# External sources

Parent index: [Subsystem documentation](./INDEX.md)

## Purpose

Describe the implemented reference-only external conversation source core used when another host owns the canonical transcript.

## Overview

Reliquary supports two source-ingestion postures:

- **native Archive-backed mode**, where Reliquary owns source conversation history; and
- **external-derived mode**, where another host owns canonical transcript bodies and Reliquary stores only rebuildable source index state, identifier/hash provenance, and semantic derivations.

External-derived mode is intentionally not a second conversation database.

## Ownership invariants

The external host remains authoritative for transcript bodies, message/conversation identity, authorization, routing, edit/delete/rewind lifecycle, compaction commit, and backup.

Reliquary may transiently hydrate bounded source content for indexing or semantic processing. It must not persist that hydrated transcript as fake Archive nodes, Episodes, or Fragments merely to reuse native APIs.

Derived Memory bodies are legitimate Reliquary semantic objects; they are not copies of the source transcript contract.

## Source contract

`ExternalConversationSource` supplies stable source namespace, feed bounds/watermark, ordered changes, snapshot manifests, and authorized hydration.

`ExternalMessageRef` and `ExternalChunkRef` preserve source namespace, conversation/message identity, content hash, and bounded offsets. Hashes fence derivation: reusing the same host ID with changed content invalidates the old derived source row.

`ExternalIndexStatus` exposes rebuild/update state without transferring transcript authority.

## Durable derived state

The external source store persists rebuildable identity/index state such as source refs, content hashes, chunk ranges, cursor/watermark, vector bindings, and processing status. It does not persist canonical external message bodies.

Cursor advancement and derived-state mutation are replay-safe. Rebuild from a host snapshot/change feed is a supported recovery posture.

## Provenance

`SourceEvidenceRef` distinguishes native Reliquary provenance from `ExternalMemoryEvidence`. External evidence remains external even after it supports a REL or PHY Memory.

Source disappearance or hash mismatch changes evidence availability. It does not automatically delete or rewrite an independently owned Memory unless the semantic owner's lifecycle rules require that outcome.

## Gateway integration

Gateway exposes authorized external rebuild/update, search, hydration/evidence lookup, and processing seams. Authorization is revalidated around hydration and publication boundaries.

External processing normalizes transient source evidence into the existing semantic pipeline rather than forking a host-specific Insomnia implementation.

## Search and hydration

External-source search returns references and derived metadata. Source text is hydrated only after host authorization/hash/range validation. Normal REL/PHY semantic results are returned as Reliquary-owned objects and require no external transcript copy.

## Failure and recovery

- Host indexing failure cannot block the host's own transcript commit.
- Replay of an already applied source change is idempotent.
- Hash mismatch makes the derived source reference stale.
- Missing source data leaves provenance unresolved rather than fabricating Archive evidence.
- Authorization loss prevents hydration/publication.
- Rebuildable external index loss is recovered from the host source, not from duplicated transcript bodies.


## Index backing admission

External Index storage decodes and installs its own cursor/source snapshots. The REL lifecycle checks retained vector/profile backing above that store through `facade/external_index_admission.rs`; invalid derived namespaces are discarded and reported damaged through the same recovery contract. The index store receives no Vector store capability.

External Message/Chunk reference values live in the neutral `source_identity.rs` vocabulary and retain their existing public reexports. Memory owns accepted external Memory evidence and window bindings. The external publication caller validates source contracts and passes Memory-owned records, bindings and exact body locations into Memory. JSON byte arrays remain in the same V2 frames; a transient typed span lets Memory read and hash its own body without an External Source decoder or a second payload copy.

## Code Map

- `src/external_source/**`
- `src/external_index/**`
- `src/runtime_inference/runtime_host_gateway_external*`
- `src/memory/**`

## Related docs

- [Architecture](../architecture.md)
- [Native import](../native-import.md)
- [Runtime multiplexing](runtime-multiplexing.md)
- [External host integration plan](../conversation-index-integration-plan.md)
- [ADR 0038](../decisions/0038-gateway-instance-runtime-multiplexing.md)

## Notes

The remaining Hermes adapter work is an integration/release concern. The provider-neutral external-source core described here is implemented.
