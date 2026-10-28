# Crate-first Reliquary architecture

Status: **Container, Archive, Memory Core, and Echo owner crates extracted; remaining owner crates are reserved targets** (2026-10-26).
Starting commit: `c43d33c`. Branch: `refactor/crate-first-restart`.

## Purpose

Retain the accepted dependency-convergence architecture, not its Phase 0–7 execution and evidence system. Extract into independent Rust libraries **as the dependency corrections are made**, rather than first untangling a single monolith. Cargo rejects physical crate dependency cycles; tests protect behavior and data. No bespoke whole-repository dependency-proof framework is a prerequisite.

The authoritative targets are [component-dag.json](architecture/component-dag.json) and [crate-map.json](architecture/crate-map.json). The former is the original 34-component, explicit-allowlist DAG, modified only to give Echo an independent owner in accordance with ADR 0014: 35 logical components, 173 approved direct logical edges. The latter records their physical ownership mapping and permitted crate dependencies. Allowed edges are *maximum permissions*, not requirements or imports to insert before they are used.

The original October 21 phase execution ledger is archived in the Step 1 Git bundle, not carried into this new branch. No roadmap features are included. The detailed source, API, directory and document crosswalk is the [crate layout specification](crate-first-layout-spec-2026-10-25.md).

## Overview

| Crate | Domain owned |
| --- | --- |
| `reliquary-container` | Physical REL/PHY persistence; local codecs and clock primitives |
| `reliquary-archive` | Conversation/source history, Episodes, checkpoints (not Echo) |
| `reliquary-memory-core` | Memory publication, revisions, lifecycle and accepted use |
| `reliquary-echo` | Ordered source-scoped computational/tool evidence; receiver acknowledgments |
| `reliquary-graph` | Memory associations, semantic topology and Communities |
| `reliquary-perception` | Entity/mention semantics and resolution/admission |
| `reliquary-freshness` | Deterministic reinforcement, propagation, historical scoring and derived due scheduling; consumes Freshness Store, Memory Core and Graph |
| `reliquary-insomnia` | Memory extraction and processing over an independent durable work store |
| `reliquary-dream` | Association, duplicate and Community synthesis; Dream journal is lower-owned |
| `reliquary-chronos` | Temporal reasoning |
| `reliquary-ego` | Identity, Personality, Anchors and summary synthesis; durable Ego state is lower-owned |
| `reliquary-retrieval` | Passive/semantic/agentic search and evidence selection |
| `reliquary-context-engine` | Token budgeting, compaction and final model-facing context |
| `reliquary-vectors` | Embedding generation and compatibility decisions over a separate Vector Store |
| `reliquary-history` | Nonmutating historical reconstruction |
| `reliquary-switchboard` | Provider transport, routes and related config/security contracts |
| `reliquary-project-environment` | Validated Lore/Git path and project authority |
| `reliquary-import` | Historical import coordination, but not its low-level epoch store |
| `reliquary-maintenance` | Authorized prune/rewrite/reconcile/repair, but not its low-level store |
| `reliquary-runtime-gateway` | REL/PHY runtime execution, sessions, host source integration |

## Ten support crates: strict physical dependency direction

The original 20-crate collapse produced two physical cycles. The initial 25-library correction removed those cycles, but still permitted **ten physical layering inversions**: Owner Lifecycle and History would import the higher-level Ego, Freshness, Vectors, Insomnia and Dream packages for their stores. Cargo would compile that graph without guaranteeing the approved lower-to-higher separation.

The final physical target separates all necessary rank-2 stores from their higher-rank services:

| Support crate | Responsibility |
| --- | --- |
| `reliquary-perception-store` | Durable accepted Entity, mention and Relationship evidence for independent Retrieval/History reads |
| `reliquary-ego-store` | Durable Identity, Personality, Anchors and Ego revisions, independent of synthesis |
| `reliquary-freshness-store` | Durable scores, settlements and accepted effects, independent of reinforcement policy |
| `reliquary-vector-store` | Durable vector/profile/generation evidence, independent of inference and embeddings |
| `reliquary-insomnia-store` | Durable extraction work queue and completion evidence, independent of processing |
| `reliquary-dream-store` | Dream journal and first-cycle evidence, independent of Dream inference |
| `reliquary-import-store` | Import epochs/receipts visible to History without Import orchestration |
| `reliquary-maintenance-store` | Maintenance manifests and proof evidence readable without mutation policy |
| `reliquary-external-index-store` | Host-derived index state without Gateway authority |
| `reliquary-owner-lifecycle` | Validated REL/PHY aggregate opening and replay using lower stores only |

The `reliquary-freshness` source cutover is integrated: propagation, semantic
policy, historical calculations, scoring/indexing and due dispatch are in the
service crate. The root Cva adapter connects due refreshes to accepted REL
activity. The service owns pure evaluation and rebuildable due scheduling; it
does not own durable records, event receipts or replay, which remain in
`reliquary-freshness-store`. Root facades/runtime continue cross-owner
authorization, I/O and acceptance orchestration. Final verification gates
remain pending. See the [service extraction
specification](freshness-service-crate-extraction-spec-2026-10-28.md).

This yields **30 libraries plus the existing `reliquary-memory` product package**. The root retains the semantic owners and consumes the extracted Container through a Cargo dependency. To avoid colliding with its Cargo name, the new Memory domain is `reliquary-memory-core`.

Other folded logical components remain distinct **inside** their crate: `pure-codec` and `review-clocks` in Container, `config-security` in Switchboard and `external-sources` inside Runtime/Gateway pending a separately deferred redesign. The original 35-component logical allowlist remains valid, but actual lower-level authority is now independently enforceable with Cargo.

## Echo correction

ADR 0014 says Echo is its own durable owner, although the old source stored `EchoStore` under `src/archive/`. The target DAG now has an independent `echo` component instead of placing Echo under `archive-store`. History, Lifecycle, Retrieval (one selected visible source only), Import, Maintenance, Context Engine and Runtime may use Echo through explicit contracts. Echo must never become a second Archive transcript, Memory truth, or a global unscoped search service.

## Extraction discipline

1. Work from the verified pre-refactor baseline in this separate branch. Do not rewrite `main` while developing.
2. Port one bounded domain at a time, starting with physical Container and shared lower contracts. Move implementations and their behavior tests. Do not create forwarding layers only for source compatibility.
3. Add a Cargo path dependency **only when code actually needs it** and only when allowed in `crate-map.json`; compilation catches physical cycles immediately.
4. Pass immutable data or narrow owned capabilities downward; never break a cycle by giving a lower crate a Runtime, facade, service locator, or broad state handle.
5. Keep source-of-truth for current-format data: the verified six-file `CVAFRS07` corpus and required format corrections from the old refactor must be recovered before claiming the new build can open it. Do not add compatibility readers for retired formats.
6. Run `cargo check --workspace --locked`, focused crate tests and the appropriate root integration/corpus tests at every meaningful extraction. Run full relevant system checks at integration points. **Do not** generate compiler call inventories, million-line evidence datasets or new architecture ratchets.
7. Delete displaced code, redundant facades and temporary adapters as each extracted library becomes authoritative. Do not leave dual implementations.

## Current acceptance boundary

`reliquary-container` owns physical implementation; `reliquary-archive`, `reliquary-memory-core` and `reliquary-echo` own their production stores, with Container as each crate's only direct Reliquary dependency. The root consumes them through Cargo dependencies and retains product lifecycle/composition. Ordinary open is nonmutating and writer/repair access is explicit. The remaining libraries still reserve layout only. The restart baseline does not implement promoted CVAFRS07 semantic settlements: current Freshness/Import recovery and all-six disposable-copy corpus admission remain required before product acceptance. See [current limitations](current-limitations.md#crate-first-recovery-boundary).

## Related docs

- [Architecture](architecture.md) owns current implementation.
- [Crate layout](crate-first-layout-spec-2026-10-25.md) owns the remaining extraction targets.
- [Container extraction](container-crate-extraction-spec-2026-10-25.md) owns this bounded cut.

## Notes

Reserved library targets do not claim an implemented ownership boundary.
