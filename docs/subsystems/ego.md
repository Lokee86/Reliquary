# Ego state

Parent index: [Subsystem documentation](./INDEX.md)

## Purpose

Describe the implemented durable Ego state owned by REL/PHY files and separate it from the still-future synthesis/context-assembly machinery.

## Overview

Current Ego persistence stores user-facing identity/personality/anchor and compact synthesis state with explicit revisions and owner-local history. The active Ego plans describe future Memory-Web synthesis, cross-chat selection, and cross-REL summary inclusion; those plans are not evidence that the current synthesis pipeline exists.

## Durable state

Implemented Ego records include:

- identities with stable IDs, revisions, names, and text;
- personality projection with source Memory-version watermark;
- anchors with stable IDs, revisions, priority, and bounded text; and
- Web-synthesis text with revision and source Memory-version watermark.

`reliquary-ego-store` is the compiled semantic owner for Ego values, compact codecs, replay and the `EgoStore` state machine. It depends on `reliquary-container`; it does not own physical framing or file I/O. REL/PHY facades keep product admission, including validation of source Memory watermarks against the live owner, and map store errors to the supported API errors. History and reconciliation use the store replay interface while keeping multi-owner coordination in the root.

Owner stores validate identities, revisions, lifecycle, and compact history on write/reopen.

## Ownership

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

- `crates/reliquary-ego-store/src/` — canonical values, codecs, lifecycle facts, replay, and state machine
- `src/facade/cva_ego.rs`
- `src/facade/phylactery_ego.rs`
- `src/facade/historical_ego_fingerprint.rs` and root reconciliation consumers — multi-owner projections over store records

## Related docs

- [Architecture](../architecture.md)
- [Storage format](../storage-format.md)
- [Ego Memory-Web synthesis plan](../ego-web-synthesis-plan.md)
- [Cross-chat context plan](../ego-cross-chat-context-plan.md)
- [Cross-REL summary inclusion plan](../ego-cross-rel-summary-inclusion-plan.md)

## Notes

The plans own unfinished synthesis/context behavior. This document owns the state that actually exists today.
