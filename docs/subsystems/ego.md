# Ego state

Parent index: [Subsystem documentation](./INDEX.md)

## Purpose

Describe the implemented durable Ego state owned by REL/PHY files and separate it from the still-future synthesis/context-assembly machinery.

## Overview

Current Ego persistence stores user-facing identity/personality/anchor and compact synthesis state with explicit revisions and owner-local history. The active plans describe future Ego Memory-Web synthesis and Context Engine-owned cross-chat/cross-REL selection and final assembly; those plans are not evidence that the current synthesis or assembly pipelines exist.

## Durable state

Implemented Ego records include:

- identities with stable IDs, revisions, names, and text;
- personality projection with source Memory-version watermark;
- anchors with stable IDs, revisions, priority, and bounded text; and
- Web-synthesis text with revision and source Memory-version watermark.

Owner stores validate identities, revisions, lifecycle, and compact history on write/reopen.

## Ownership

Ego owns semantic synthesis and persisted Identity, Personality, Anchors and summary outputs, not inference-time context selection, budgeting or final model-facing assembly. Those future assembly responsibilities belong to Context Engine, while Runtime/Gateway supplies authorized scope and execution. This separation is an accepted target boundary, not a claim that future synthesis/assembly is implemented.

Ego state belongs to its durable REL/PHY owner. PHY-private Ego state does not become REL-shared merely because a REL consumes it for context.

Source Memory watermarks are dependency evidence for derived projections, not authority to mutate Memory.

## Compact history

Ego compact records preserve the exact current-state/history semantics required by reopen, rewrite, reconciliation, and historical cuts. Compact codecs are storage mechanics; the Ego store remains the semantic owner.

## Lifecycle and transforms

Rewrite/prune/reconciliation must preserve or intentionally select Ego state according to the explicit operation policy. Terminal owner lifecycle prevents resurrection through stale replay.

## Failure behavior

- invalid/repeated revisions fail validation;
- malformed compact history fails reopen rather than guessing state;
- cross-owner state cannot be silently merged;
- source watermark drift does not itself synthesize new content; and
- future synthesis scheduling must publish through the existing Ego owner.

## Code Map

- `src/ego/**`
- `src/facade/cva_ego.rs`
- `src/facade/phylactery_ego.rs`

## Related docs

- [Architecture](../architecture.md)
- [Storage format](../storage-format.md)
- [Ego Memory-Web synthesis plan](../ego-web-synthesis-plan.md)
- [Cross-chat context plan](../ego-cross-chat-context-plan.md)
- [Cross-REL summary inclusion plan](../ego-cross-rel-summary-inclusion-plan.md)

## Notes

The plans own unfinished synthesis/context behavior. This document owns the state that actually exists today.
