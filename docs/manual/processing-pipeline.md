# Processing Pipeline

Parent index: [Reliquary operator manual](INDEX.md)

## Purpose

Explain which processing stage consumes which state and which results are durable.

## Overview

The current common front half is:

```text
source turns/files
    ↓
Archive + Episodes
    ↓
Insomnia
    ↓
Memories + routing metadata + Entity mention spans
```

From that durable Memory state, there are two semantic lanes:

```text
Dream lane:
Memories
  → verified Memory-to-Memory Graph/lifecycle structure
  → Memory-only Community/retrieval inputs

Perception lane:
Memories + Entity mentions
  → candidate retrieval + Entity resolution
  → durable Entities + Memory→Entity Graph associations
```

Dream's current production lane is implemented. Perception Entity pass 1 is also implemented end to end in the long-lived runtime host: exact extracted mentions flow through bounded candidate construction, zero-candidate Admission or non-empty-candidate V4 identity resolution, and deterministic Entity/Graph/resolution-state persistence after Dream. The frozen zero-Entity corpus proves the owner path, while runtime tests cover automatic REL/PHY scheduling and lock-free model inference.

## 1. Ingest source state

Use `Cva::ingest_turn` for turn ingestion when attachments need to be part of the same provenance operation. Lower-level Archive calls exist, but application/runtime integrations should avoid recreating turn-ingestion ownership manually.

## 2. Materialize/finalize Episodes

Episodes are deterministic semantic-processing windows. Live runtime scheduling and bulk import paths own their respective Episode lifecycle.

Do not make PHY Episodes. A routed PHY Memory points back to source identity in the REL instead.

## 3. Run Insomnia

Configured finite execution is available through `ConfiguredRuntime::run_insomnia_files`. The runtime host owns long-lived scheduling.

Insomnia currently produces:

- Memory candidates and durable Memory publication.
- authority/grounding provenance.
- temporal metadata.
- lexical routing metadata.
- exact Entity mention spans.
- completion/retry operational state.
- optional REL→PHY User Memory routing.

Entity mention extraction means “this span may name a referent.” It does not mean an Entity has been identified.

## 4. Run Dream

Dream operates on owner-local Memories. It performs bounded candidate discovery, relation classification/verification, Graph publication, duplicate/supersession handling, lifecycle projection, and Community-related derived work.

Dream currently sees the Memory-only Graph projection. Entity-only Graph mutations do not enter Dream traversal.

## 5. Entity resolution / Perception status

The following primitives are current:

- durable Entity objects in REL/PHY.
- exact one-to-many canonical-name/alias candidate lookup plus source-association recovery.
- bounded per-mention candidate retrieval combining exact surface, lexical Memory context, and first-hop Dream-neighbour evidence.
- bounded same-surface Admission context for zero-candidate mentions.
- **Entity Admission v2** for first-seen durable-referent judgment and initial stable metadata.
- calibrated **V4 identity resolution** for one or more existing Entity candidates.
- `resolve_entity_mention(...)` orchestration across model judgment, Entity creation/reuse/split, typed `Memory -> Entity` Graph association, and per-mention resolution state.
- full semantic Graph traversal and Knowledge read visibility.

The calibrated V4 contract remains unchanged from the frozen preconstructed-Entity calibration. It is **not** used for first-seen admission. The production split is:

```text
extracted mention
  → bounded candidate retrieval
      ├─ zero candidates → Admission v2
      │                    create / unresolved / reject
      └─ candidates     → V4 identity resolution
                           resolve / split-create / unresolved / reject
  → deterministic Entity + Graph + mention-state persistence
```

The 41-query frozen bootstrap has now proven the organic zero-Entity path without preconstructed Entity records: 24 first-occurrence creates, 13 repeat resolves, one distinct same-surface split, one unresolved, two rejects, and 25 durable Entities after reopen.

The runtime host now makes Entity resolution an automatic post-Dream capability. New/affected Dream Memories enqueue exact mention keys; unresolved mentions are also re-enqueued by bounded same-surface or candidate-Entity evidence events. The worker always rebuilds candidates/evidence before inference, skips unchanged candidate/context fingerprints, performs model inference outside REL/PHY locks, and commits only if the prepared Memory/Entity/Graph/resolution snapshot is still current.

## 6. Derived state refresh

Community and Memory-retrieval derived state follows the Memory Graph projection watermark, not every semantic Graph write. Perception-only Entity associations therefore do not force unchanged Dream/Leiden/retrieval state to rebuild.

## Related docs

- [Insomnia semantic validation](../insomnia-semantic-validation-2026-08-24.md)
- [Dream design record](../subsystems/dream.md)
- [Perception subsystem plan](../perception-subsystem-plan.md)
- [Working with Memories and Entities](memories-and-entities.md)

## Notes

Do not compensate for bad Entity mention extraction inside the resolver. Extraction errors and identity-resolution errors are separate failure classes.
