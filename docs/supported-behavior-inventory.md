# Supported Behavior Inventory

Parent index: [Documentation index](INDEX.md)

## Purpose

Select reference behavior for the independent rebuild.

## Overview

This inventory summarizes behavior exercised through Reliquary's public crate facade and representative downstream consumers. It is an evidence map, not a complete API reference or a claim that every feature is production qualified. The focused API reference and behavioral-contract documents remain authoritative for details.

## F02 selection decisions

F02 selects externally observable behavior from the pinned F01 reference (`f62b1fb44ca7f0bd8351e3a30fb5d76211f85102`). “Retain” means the behavior is in rebuild scope; it does not require retaining its current implementation. “Replace” means the behavior remains in scope but must be expressed through the selected clean architecture and REL 2.1 boundary. “Defer” means it is not required for the initial rebuild acceptance. “Retire” means no replacement is required for the legacy mechanism itself. Test evidence demonstrates exercised cases only; it does not establish production readiness or exhaustive compatibility.

| Disposition | Selected behavior | Evidence and boundary |
| --- | --- | --- |
| **Retain** | Owner creation/open/reopen, append and durable owner-local state; typed REL/PHY ownership and explicit recovery/error outcomes. | Public facade and persistence contracts are exercised by [native import](../tests/native_import.rs), [historical payload boundary](../tests/historical_payload_boundary.rs), and [REL activity clock contract](../tests/rel_activity_clock_contract.rs). Physical format acceptance is narrowed to REL 2.1 by the rebuild plan; 2.0 conversion is a separate data task, not runtime compatibility. |
| **Replace** | Container mechanics, durable owner codecs/replay, product facade composition, and old cross-owner orchestration. | Retain asserted reads, writes, receipts, and failure boundaries while implementing them in the selected package/ownership graph, not by copying Root's former aggregate or requiring a CVA-wide publication manifest for normal writes. Evidence: [target crate layout](dependency-architecture-and-crate-layout-2026-11-03.md), [invariants](invariants.md), and [target crate DAG](architecture/target-crate-dag-2026-11-03.json). |
| **Retain** | Archive conversation/node/branch semantics, path materialization, transcript reads, stable identity, and bounded graph roundtrip. | [Archive roundtrip](../examples/archive_roundtrip.rs), [conversation management consumer](../tests/workspace_management.rs), and [multiplexing conversation contract](../tests/multiplexing_phase0b_conversation_contract.rs). Retain tested semantics and stable-ID goldens; F03 owns the separate identity/corpus proof. |
| **Retain, bounded** | Supported native import profiles, canonicalized conversation/branch/source evidence, exact retry receipts, and no mutation on tested repeated import. | [Native import integration](../tests/native_import.rs) exercises maintained synthetic profiles and explicitly checks tested private-marker handling and retry clocks/receipt. Do not infer arbitrary provider-export compatibility; real adapters and broader conformance remain separate work in [current limitations](current-limitations.md#native-import-profile-and-integration-limits). |
| **Retain** | Owner-local Memory publication/provenance, retrieval/use boundary, Freshness activity, and structural distinctions among available, tombstoned, and pruned historical payload. | [Freshness retrieval boundary](../tests/freshness_retrieval_boundary.rs), [Freshness owner contract](../tests/freshness_owner_contract.rs), and [historical payload boundary](../tests/historical_payload_boundary.rs). Preserve explicit authorization, receipts, and owner-local state boundaries described in [behavioral contracts](behavioral-contracts.md). |
| **Retain** | Authenticated host-facing management: mount/admission, owner selection, conversation/transcript paging, title update, Memory creation and summary/provenance read. | [Workspace management consumer](../tests/workspace_management.rs) names and invokes the downstream API. Retain only its asserted operations and limits; unsupported management operations remain explicit. |
| **Replace** | Gateway/Instance authorization, lifecycle, multiplexing, journal recovery, and host adapter behavior. | Dedicated public consumer contracts cover [baseline](../tests/multiplexing_baseline_contract.rs), [Gateway API](../tests/multiplexing_gateway_api_contract.rs), and [Phase 0b lifecycle, events/auth, journal, personal, host, and conversation contracts](../tests). Rebuild the externally exercised runtime behind the selected Gateway/Application/owner-session seams. A management consumer contract is not evidence of a deployed Warlock/Hermes integration. |
| **Defer** | Perception Observations, whole-Web bootstrap, reasoning-directed expansion/reconsideration, and ambiguity clarification; Ego Memory-Web synthesis and cross-chat/cross-REL context assembly. The source-grounded Insomnia publication required by V01 remains in scope. | These remain future work in [roadmap](roadmap.md) and [current limitations](current-limitations.md). Existing store/model crates or plans are not evidence that these behaviors are implemented or required for initial rebuild acceptance. |
| **Defer** | Production qualification of agentic retrieval/recall, broad provider interoperability, and release-level Warlock/Hermes adoption. | The reference documents weak retained recall and pending consumer/release acceptance in [roadmap](roadmap.md) and [current limitations](current-limitations.md). Keep implemented narrow behavior where covered by contracts; defer the unproven quality/production claims. |
| **Retire** | Legacy internal Root aggregation, incidental module/file boundaries, pass-through helper crates, and implementation-only internal methods without a required behavior contract. | F02 acceptance is based on tests/examples and consumer use, not function-for-function parity. The selected merges/retirements are listed in [target crate layout](dependency-architecture-and-crate-layout-2026-11-03.md); no test evidence makes old internal APIs mandatory. |
| **Defer** | Broad historical/version-history views, restore-and-continue, expanded synchronization/reconciliation, and project/file usability beyond tested consumer operations. | These are separately scoped in [roadmap](roadmap.md), [version-history plan](version-history-plan.md), and [current limitations](current-limitations.md). Do not silently include them in initial rebuild acceptance. |

### Reference and consumer evidence limits

The pinned reference's public integration suite includes `tests/native_import.rs`, `tests/workspace_management.rs`, `tests/freshness_retrieval_boundary.rs`, `tests/historical_payload_boundary.rs`, `tests/multiplexing_*`, and `tests/rel_activity_clock_contract.rs`; examples include `examples/archive_roundtrip.rs`. F02 uses these as behavioral evidence alongside the documented target boundaries. The reference checkout is maintained separately at `C:\!bin\workspace\Reliquary-reference`; its Git pin and custody are recorded in the F01 evidence note (`C:\!bin\workspace\reliquary-f01-evidence-2026-11-03.md`). No source implementation or test has been copied into this inventory. The six-file old-format corpus check is ignored/configuration-dependent, so this selection does not claim that corpus compatibility is proven; F03 owns that question. Synthetic fixtures, ignored gates, pending acceptance, and untested downstream deployments remain limitations.

## Public entry points and persistence

- Consumers create and reopen owners through `Cva` (also exported as the `Reliquary` alias) and `Phylactery`; examples exercise create, append, sync, drop, and reopen flows. Evidence: [archive roundtrip](../examples/archive_roundtrip.rs), [native import integration](../tests/native_import.rs), and exports in `src/lib.rs`.
- REL and PHY preserve owner-local semantic state across reopen. Public integration suites assert durable values and retry behavior, including native import's exact receipt and unchanged version clocks on a repeated import. Evidence: [native import integration](../tests/native_import.rs).
- The low-level physical container API is public, while semantic replay and authorization remain owned by the product facade. Its public behavior and recovery limits are documented in [Rust API](api.md#physical-container-access).

## Archive and conversation data

- Archive consumers can append Nodes and Branches, materialize path Episodes/Fragments, inspect conversation turns, and search stored content. The JSONL `archive_roundtrip` example reconstructs a graph fixture, reopens it, and checks the expected paths. Evidence: [archive roundtrip](../examples/archive_roundtrip.rs).
- Native import normalizes supported ChatGPT, Claude, Hermes JSONL, and structured-agent fixture profiles, persists canonical content/branch topology and attachment evidence, and deduplicates exact retries. Tests explicitly reject leaked private markers in the tested content path and compare the exact receipt after reopen. Evidence: [native import integration](../tests/native_import.rs). These are synthetic maintained fixtures; they do not certify arbitrary provider exports (see [current limitations](current-limitations.md#native-import-profile-and-integration-limits)).
- Conversation metadata, transcript reconstruction, branch topology, and historical payload availability have focused root-level contracts; the complete guarantees and refusal cases are in [behavioral contracts](behavioral-contracts.md).

## Memory, history, and retrieval

- A public consumer can publish Memory with explicit source/provenance fields, then use owner reads and retrieval flows. The public freshness boundary test builds Episodes, publishes Memory, and exercises use/graph activity through the facade. Evidence: [freshness retrieval boundary](../tests/freshness_retrieval_boundary.rs).
- Historical payload access distinguishes available payload, tombstoned payload, and pruned payload with structural history. A compile-time style integration test verifies root's re-export is the same type as `reliquary-history` and preserves all variants. Evidence: [historical payload boundary](../tests/historical_payload_boundary.rs).
- Retrieval, ranking, Freshness, Graph, and Community behavior is intentionally split between owner authorization/state and compiled services; it is not a guarantee of cross-owner final context composition. See [behavioral contracts](behavioral-contracts.md#compiled-memory-owner), [Rust API](api.md), and [current limitations](current-limitations.md).

## Host-facing management/runtime

- A downstream host can mount a REL, authenticate an instance through a supplied admission adapter, obtain `WorkspaceManagement`, select a conversation, page conversation/transcript results, set a title, create Memory, and read its summary/provenance. Evidence: [workspace management consumer](../tests/workspace_management.rs).
- Runtime and gateway contracts expose configured routes and authorized owner operations, but the test suite's management example is a consumer contract, not evidence of Warlock deployment or complete cross-owner orchestration. The scope boundary is documented in [README](../README.md#current-boundaries) and [current limitations](current-limitations.md).

## Evidence limits

- Most evidence is unit or integration testing against synthetic fixtures. It demonstrates the asserted cases, not exhaustive compatibility with all historical files, providers, workloads, or host integrations.
- Some gates are explicitly pending or limited. For example, retrieval service extraction acceptance is still marked in progress, and current limitations describe the unresolved recall and production-qualification findings. See [current limitations](current-limitations.md#retrieval-service-extraction-acceptance) and [current limitations](current-limitations.md#agentic-memory-retrieval-status).
- This inventory does not introduce new guarantees. For exact semantics, use the linked API and contract sections and the cited test assertions.


## Related docs

- [Rebuild plan](architecture-rebuild-implementation-plan-2026-11-03.md)
- [Capability plan](architecture/implementation-rebuild-capabilities-2026-11-03.json)

## Notes

Selection evidence does not establish final acceptance.

