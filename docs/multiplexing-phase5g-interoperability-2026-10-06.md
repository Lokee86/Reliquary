# Multiplexing P5-G — import, interoperability, and external transcript adapters

Parent: [P5/P6 parallel execution](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)  
Canonical external-transcript plan: [External Conversation Index Integration](conversation-index-integration-plan.md)  
Status: **next-wave parallel track. Framework-neutral import/external-source work can start immediately; provider importers and the external-derived R0–R4 core can be split into independent subtracks. ACP/Hermes adapter completion remains source/SDK dependent and does not block legacy-host deletion once core contracts are independent.**

## Purpose

Own the external interaction ingestion/interoperability surface that the A-D rewrite left unassigned: production transcript imports, ACP/live adapter integration, and the reference-only external canonical transcript mode. Keep protocol/provider formats outside Reliquary semantic ownership and reuse E/F/A/B seams.

## Overview

The roadmap requires normalized historical imports for ChatGPT, Claude/provider exports, Codex/Hermes/structured agent sessions and ACP interoperability. A separate accepted plan defines external-derived mode for hosts such as Hermes that already own canonical transcript history. Current source has native import queue primitives and external Memory provenance, but no complete production adapter family or reference-only external index implementation. Phase 0B found no ACP source checkout and no direct Hermes Reliquary consumer at that time; applicability must be rechecked rather than assumed.

## Target architecture

Two explicit source modes:

1. **Native/imported Archive mode**: normalized source turns become canonical Reliquary Archive evidence through one idempotent import path.
2. **External-derived mode**: host transcript remains canonical; Reliquary stores stable external IDs/content hashes/cursors and rebuildable derived semantic/index state, hydrates source text transiently through an authorized host API, and returns external source references.

Both feed shared Insomnia/Dream/Perception/vector semantics where evidence contracts allow it. ACP/Hermes/provider adapters translate transport/source formats into E's normalized runtime/import contracts. No protocol-specific semantic record type.

## Source ownership

G owns importer/adapter modules, external-source contract/index modules and their tests. Existing conversation-index-integration-plan R0-R7 remains the detailed design for external-derived mode. E owns live adapter/session contract, F owns management invocation, B owns semantic workers, A owns owner-qualified publication/retrieval. C may host the Warlock-side adapter but not redefine G contracts.

## Implementation steps

### G0 — applicability and source audit
Recheck available Hermes, ACP and provider import repos/formats. Record actual maintained sources and versioned fixtures. Unknown/unavailable adapter repositories are explicit external blockers, not silently omitted implementation.

### G1 — normalized production import contract
Define provider-neutral imported conversation/message/branch/artifact identity, edits/replacements/deletion markers, stable source namespace and deterministic re-import key. Import normalization must not infer provider-private hidden state.

### G2 — ChatGPT/Claude/structured agent importers
Implement supported production importers with golden fixtures, branch/edit semantics, artifact provenance and deterministic re-import. Route normalized native imports through one Archive/Insomnia commit path; no importer-specific semantic policy.

### G3 — ACP/live interoperability
Implement Reliquary ACP adapter against E's transport-neutral runtime when an ACP source/SDK is available. Capture durable normalized events and context injection without making ACP canonical storage. Protocol version instability remains isolated in adapter code.

### G4 — external canonical transcript R0-R4
Execute the existing plan's framework-neutral contracts, reference-only derived index, search/rebuild, external provenance/Insomnia processing and lifecycle invalidation. Store no duplicate canonical transcript body. Rebuild from host feed/hydration must reproduce the same derived identity.

### G5 — Hermes adapter
When the real Hermes integration repository/API is available, implement R5: durable/reconstructable change feed, stable message refs, authorized hydration, cursor/rebuild, search source refs and semantic processing. Hermes-specific transport stays outside core.

### G6 — semantic compaction proposals and additional adapters
Implement compaction only as host-validated proposal output per the existing plan. Additional provider/framework adapters reuse G1/E contracts and get their own fixtures; they never modify core semantic ownership.

### G7 — acceptance
Cross-mode tests: deterministic re-import, edit/replacement/delete, branch identity, artifact provenance, crash/restart, rebuild equality, no stored external transcript body, source authorization, host unavailable/reconnect, source-reference search, Insomnia dedup and private owner isolation. Run adapter-specific compile/tests in actual external repos plus Reliquary integration/docs gates.

## Dependencies

G1/G2 and external-derived R0-R4 can start against current core plus A/B primitives. G3 consumes E. G5 requires actual Hermes source/API. F may expose import controls after G contracts stabilize. P6 adds failure/load verification.

## Out of scope

Making ACP or Hermes the canonical semantic model, scraping private chain-of-thought, duplicating external transcripts into Archive in derived mode, generalized synchronization service, arbitrary unsupported provider formats.

## Related docs

- [External conversation index plan](conversation-index-integration-plan.md)
- [ADR 0015 ACP](decisions/0015-acp-inline-interaction-stream.md)
- [ADR 0016 shared runtime](decisions/0016-native-product-surface-and-shared-interaction-runtime.md)
- [P5-E live runtime](multiplexing-phase5e-live-runtime-surface-2026-10-06.md)

## Notes

G includes real implementation where source contracts exist and explicit external blockers where they do not. That is different from dropping a scope: every known interoperability responsibility has an owner and an acceptance path.
