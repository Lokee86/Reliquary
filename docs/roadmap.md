# Roadmap

Parent index: [Documentation index](INDEX.md)

## Purpose

Own future Reliquary implementation work. Completed behavior belongs in current subsystem, architecture, API, storage, CLI, and behavioral-contract documentation.

## Overview

The active work is now concentrated in a small number of tracks:

1. finish the current compact/lossless storage-history rewrite;
2. complete Perception Observations and then Ego synthesis/context assembly;
3. finish explicit Insomnia commit semantics;
4. finish Warlock/Hermes consumer/release work around the implemented Gateway runtime;
5. expand historical/version recovery deliberately; and
6. continue product/security/interoperability hardening when those capabilities are actually needed.

Completed Freshness, Dream, decision-routing, tombstone/import/rewrite, external-source-core, and Gateway implementation sequences are not roadmap items.

## Active storage/history track

The current large active implementation is the compact/lossless history/storage rewrite.

Authoritative planning:

- [Storage/history specification](minimal-lossless-history-storage-spec-2026-10-19.md)
- [Linear execution plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md)
- [Development-format hard-cut interruption](development-format-hard-cut-interruption-spec-2026-10-19.md)

Do not duplicate that plan here. When it completes, graduate its final current contracts into Storage Format/Architecture/subsystem docs and retire the execution documents.

## Semantic sequence

### 1. Cross-scope Entity identity completion

The durable Entity substrate and ordinary mention-resolution foundation exist. Remaining work is relational-evidence-driven cross-scope candidate comparison/reconciliation, exact dirty triggering/receipts/repair, and measured calibration on retained corpora.

This work should stabilize Entity identity before Observation reasoning depends on it broadly.

### 2. Perception Observations

Implement the future work in [Perception future implementation plan](perception-subsystem-plan.md):

- Observation persistence/lifecycle;
- whole-Web bootstrap;
- Exploitation/Exploration routing;
- reasoning-directed evidence expansion;
- reasoning-specific verification;
- relational Observations;
- routing receptors;
- reconsideration/currentness; and
- ambiguity/clarification.

Relationship should converge toward the relational-Observation model rather than remain a duplicated long-term authority.

### 3. Ego synthesis and context assembly

After the semantic substrate is stable, implement the existing future-only Ego plans:

- [Memory-Web synthesis](ego-web-synthesis-plan.md)
- [Cross-chat context](ego-cross-chat-context-plan.md)
- [Cross-REL summary inclusion](ego-cross-rel-summary-inclusion-plan.md)

Ego should compose existing owner state rather than become another semantic database.

## Explicit Memory commit

Implement [Insomnia explicit Memory commit](insomnia-explicit-memory-commit-plan.md).

The operation must retain Insomnia as semantic author while giving an explicit user/model retention request a hard completion contract: the initiating turn is included in provenance and at least one accepted canonical Memory survives.

Do not expose a caller-authored direct Memory write API.

## Runtime consumer releases

Reliquary's Gateway/Instance multiplexing architecture is current behavior. Remaining release work is tracked in [the multiplexing consumer/release plan](multiplexing-implementation-plan.md).

Near-term:

- finish real Warlock provider/UI/multi-context acceptance;
- converge exact dependency pins and remaining legacy-shaped surface;
- run release-level recovery/privacy/stream/provider gates; and
- implement Hermes over the existing external-source core.

Hermes external transcript integration is detailed in [the external host integration plan](conversation-index-integration-plan.md).

## Agentic Memory retrieval qualification

The agentic controller is implemented. Remaining work is evaluation/product qualification rather than implementation of the basic controller.

The retained evidence-recall benchmark remains weak. Investigate candidate eligibility/discovery/presentation with measured changes, then rerun the frozen comparison and document a production disposition.

Do not weaken the successful-handoff Freshness boundary merely to improve benchmark recall.

## Chronos follow-on

Current Chronos is implemented and shared. Remaining work is in [Chronos future work](chronos-subsystem-plan.md):

- Perception/Observation integration;
- measured grammar/detector calibration;
- multi-evidence temporal validity composition; and
- optional disposable caching only if profiling justifies it.

## Historical Freshness follow-on

Explicit offline/exclusive REL historical reconstruction is complete.

Possible future work, each requiring a separate justification:

- mounted historical management;
- relevance-labelled/default-activation policy;
- wider scale/SLO evidence;
- a separately designed PHY activity/Freshness source; and
- source-identity conversion only with retained proof.

Do not reopen the completed REL historical reconstruction implementation to pursue these independent extensions.

## Versioning and historical recovery

Future whole-CVA historical views, restore-and-continue, retention policy, and rollback remain in [Versioning, historical cuts, and rollback](version-history-plan.md).

This work must compose owner-local history explicitly rather than inventing one fake global semantic ancestry chain.

## Project environment and VCS boundary

Continue reducing Reliquary responsibilities that duplicate authoritative project repository history.

Future work may simplify:

- duplicated project/blob ancestry;
- filesystem-recovery history better owned by Lore/Git;
- project-file revision resolution where an exact repository reference already suffices; and
- synchronization assumptions that conflate semantic-database history with project VCS history.

Preserve REL/PHY semantic history, provenance, Graph/Memory/Entity/Observation history, vector generations, and crash recovery independently of the project VCS.

## REL/PHY synchronization

Reassess whole-file reconciliation around divergent semantic owners only.

Future synchronization must:

- keep REL/PHY transport separate from project-repository transport;
- validate each semantic owner explicitly;
- define cross-device conflicts without assuming Git/Lore ancestry solves them;
- preserve recovery/promotion safety; and
- avoid growing reconciliation into another VCS.

## External/import interoperability

The framework-neutral external-source core is implemented. Remaining interoperability work is adapter/product work:

- real Hermes adapter and conformance;
- production ChatGPT/Claude/provider import adapters where useful;
- deterministic re-import/edit/delete/branch semantics;
- Codex/structured-session adapters;
- optional ACP integration over normalized interaction seams; and
- artifact provenance.

ACP or provider export formats must not become canonical semantic schemas.

## File usability

Product-facing file work may include:

- add/import/export;
- path/folder/tree organization;
- rename/move with stable historical meaning;
- content extraction/indexing;
- retrieval separated from filename metadata; and
- generated-artifact provenance/lifecycle.

Do not reintroduce REL-owned project VCS history to implement these conveniences.

## Runtime and security hardening

Future production hardening includes:

- OAuth refresh where providers require it;
- OS credential-store-backed master key handling;
- standard application/config locations;
- provider retry/backoff/rate-limit behavior;
- observability without content/secret leakage;
- crash-safe background work recovery; and
- authentication/authorization only if a future IPC/server boundary is introduced.

A standalone service/IPC layer requires its own architecture decision.

## Scope evolution

Current REL/PHY ownership is documented in [Scope ownership](subsystems/scope-ownership.md).

Future scope work may include:

- explicit UI/API management of REL dependency topology;
- richer learned-state routing beyond conservative User/Project classification;
- governed organization/team state with explicit authority;
- cross-file source/Memory export lineage; and
- eventual public-facing terminology cleanup around legacy `Cva` naming.

Do not weaken ownership/privacy merely to make scope composition easier.

## Configurable VCS management — deferred

A future Project Environment may separate repository backend from management policy:

```text
backend: lore | git
management: manual | assisted | automatic
```

Managed Git must preserve unrelated working-tree/index state, bind owned operations to exact base revisions, keep push separately authorized, and make mutation retry/recovery idempotent.

This remains deferred product work.

## Release discipline

For every active track:

- current behavior belongs in current docs;
- plans contain only unfinished work plus enough baseline context to execute it;
- completed execution steps are removed rather than accumulating status diaries;
- verification evidence may remain as dated evidence when it is materially useful;
- new persistent formats/owners require explicit architecture/storage documentation; and
- Archivist/Pitlord gates must be green before release.

## Related docs

- [Subsystem documentation](subsystems/INDEX.md)
- [Current limitations](current-limitations.md)
- [Architecture](architecture.md)
- [Storage format](storage-format.md)
- [Behavioral contracts](behavioral-contracts.md)

## Notes

The roadmap intentionally avoids repeating detailed active execution plans. It should remain a future-work map, not a historical project log.
