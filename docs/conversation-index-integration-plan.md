# External canonical-transcript host integration plan

Parent index: [Documentation index](INDEX.md)

Current implementation: [External sources](subsystems/external-sources.md)

## Purpose

Track the remaining host-specific integration work for frameworks that already own canonical conversation transcripts.

The provider-neutral Reliquary external-source core is implemented. This plan no longer describes that core as future work.

## Overview

Reliquary can already store reference-only external index state, external provenance, rebuild/update cursors, vectors, transient hydration, and Gateway-facing processing/search surfaces without copying canonical host transcript bodies into Archive.

The remaining work is to connect real hosts—first Hermes—to that core and prove the complete operational contract.

## Fixed ownership model

The external host owns:

- canonical transcript bodies;
- native conversation/message/branch identity;
- authorization;
- edit/delete/rewind semantics;
- canonical compaction commit;
- host routing/session lifecycle; and
- transcript backup/recovery.

Reliquary owns:

- rebuildable derived external index state;
- external source refs/hashes/offsets/vector bindings;
- semantic derivations such as Memories;
- REL/PHY semantic lifecycle under ordinary owner rules; and
- Gateway/Instance authorization around Reliquary operations.

No adapter may make Reliquary a shadow transcript database.

## Implemented Reliquary baseline

The current external-source core includes:

- `ExternalConversationSource`;
- stable source/message/chunk identity;
- snapshot/change-feed rebuild/update;
- durable cursor/index status;
- content-hash fencing;
- transient authorized hydration;
- `ExternalMemoryEvidence` and `SourceEvidenceRef`;
- external semantic-processing preparation/publication;
- Gateway integration; and
- rebuildable search/index semantics.

See [External sources](subsystems/external-sources.md).

## H1 — Hermes source contract

Inventory Hermes's real source/session model and freeze the adapter contract for:

- profile/source namespace;
- conversation, branch, and message identity;
- feed floor/high-water/cursor;
- create/edit/delete/rewind operations;
- operation identity and retry behavior;
- authoritative timestamps;
- authorization checks;
- bounded hydration; and
- snapshot/rebuild.

Do not invent Reliquary IDs where Hermes already has stable identities.

### Exit

A versioned normalized adapter contract exists and can represent actual Hermes history/replay semantics without transcript duplication.

## H2 — Change-feed adapter

Implement the Hermes-side source adapter against `ExternalConversationSource`.

Requirements:

- monotonic/reconstructable change consumption;
- idempotent replay;
- content hashes on every derived source identity;
- deterministic rebuild from a current snapshot;
- deletion/rewind invalidation;
- bounded batching/backpressure; and
- no acknowledgement dependency from Hermes's normal chat commit path.

Reliquary outage must not block Hermes transcript durability.

## H3 — Authorized hydration

Hydration must occur through Hermes authority at the point of use.

Prove:

- principal/source authorization before content exposure;
- message hash verification;
- range validation;
- revocation between search and hydration;
- missing/deleted source behavior; and
- no durable transcript-body copy left in REL/PHY.

## H4 — Semantic processing

Feed transient external windows into shared semantic extraction rather than a Hermes-specific Memory engine.

Required behavior:

- normalized source chronology/provenance;
- shared Insomnia extraction semantics;
- ordinary REL/PHY routing;
- external evidence retained on produced Memories;
- no fake Archive Episode/Node identity; and
- retry/rebuild does not duplicate semantic publication.

## H5 — Search and source drill-down

Integrate external derived search with Hermes-facing tools/UI.

Search returns refs/metadata. Hermes performs final hydration and authorization. Reliquary-owned Memory/Entity/Observation results remain ordinary semantic objects.

Prove stale-hash and deleted-source behavior explicitly.

## H6 — Compaction proposal integration

Reliquary may propose semantic compaction of host history, but Hermes owns commit.

A proposal must bind:

- exact source range/branch;
- input message identities and hashes;
- generated summary/semantic artifact;
- policy/model identity where relevant; and
- enough proof for Hermes to reject a stale proposal.

Committing a proposal must never silently mutate Reliquary source authority.

## H7 — Gateway/Instance session integration

Map real Hermes sessions onto authenticated Gateway/Instance usage.

Validate:

- simultaneous sessions;
- independently selected RELs/branches;
- same-principal shared PHY behavior;
- different-principal isolation;
- cancellation/reconnect;
- source revocation;
- Gateway remount/epoch changes; and
- external-source operations under exact instance grants.

## H8 — End-to-end recovery

Exercise:

- Reliquary unavailable while Hermes continues;
- later catch-up from feed/snapshot;
- crash between derived mutation and cursor advance;
- duplicate feed delivery;
- host edit/delete/rewind after indexing;
- lost/corrupt derived index rebuild;
- semantic publication retry; and
- ambiguous in-flight provider/runtime outcomes independently of source indexing.

## Acceptance

The integration is complete when a real Hermes instance can run concurrent authorized sessions, rebuild/index its canonical history, hydrate exact evidence, derive Memories without source duplication, survive restart/replay/revoke/delete/rewind, and operate while Reliquary indexing is temporarily unavailable.

## Non-goals

- replacing native Archive-backed operation;
- persisting Hermes transcript bodies in Archive;
- creating a Hermes-specific semantic schema;
- making ACP or another protocol the canonical storage model;
- requiring host chat commits to wait for Reliquary; or
- granting Reliquary authority over host compaction commit.

## Related docs

- [External sources](subsystems/external-sources.md)
- [Runtime multiplexing](subsystems/runtime-multiplexing.md)
- [Multiplexing release plan](multiplexing-implementation-plan.md)
- [Architecture](architecture.md)
- [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)

## Notes

The earlier version of this document mixed implemented core design with future Hermes work. That current-state material has been graduated to the subsystem documentation.
