# Crate-first Reliquary architecture

Status: **Step 2: target layout established; production extraction not yet started** (2026-10-25).  
Starting commit: `c43d33c`. Branch: `refactor/crate-first-restart`.

## Decision

Retain the accepted dependency-convergence architecture, not its Phase 0–7 execution and evidence system. Extract into independent Rust libraries **as the dependency corrections are made**, rather than first untangling a single monolith. Cargo rejects physical crate dependency cycles; tests protect behavior and data. No bespoke whole-repository dependency-proof framework is a prerequisite.

The authoritative targets are [component-dag.json](architecture/component-dag.json) and [crate-map.json](architecture/crate-map.json). The former is the original 34-component, explicit-allowlist DAG, modified only to give Echo an independent owner in accordance with ADR 0014: 35 logical components, 173 approved direct logical edges. The latter records their physical ownership mapping and permitted crate dependencies. Allowed edges are *maximum permissions*, not requirements or imports to insert before they are used.

The original October 21 phase execution ledger is archived in the Step 1 Git bundle, not carried into this new branch. No roadmap features are included.

## First-class libraries: 20

| Crate | Domain owned |
| --- | --- |
| `reliquary-container` | Physical REL/PHY persistence; local codecs and clock primitives |
| `reliquary-archive` | Conversation/source history, Episodes, checkpoints (not Echo) |
| `reliquary-memory-core` | Memory publication, revisions, lifecycle and accepted use |
| `reliquary-echo` | Ordered source-scoped computational/tool evidence; receiver acknowledgments |
| `reliquary-graph` | Memory associations, semantic topology and Communities |
| `reliquary-perception` | Entity/mention semantics and resolution/admission |
| `reliquary-freshness` | Freshness store, triggered reinforcement, propagation and decay |
| `reliquary-insomnia` | Memory extraction and durable work state |
| `reliquary-dream` | Association, duplicate and Community synthesis; Dream journal |
| `reliquary-chronos` | Temporal reasoning |
| `reliquary-ego` | Identity, Personality, Anchors and summary synthesis/store |
| `reliquary-retrieval` | Passive/semantic/agentic search and evidence selection |
| `reliquary-context-engine` | Token budgeting, compaction and final model-facing context |
| `reliquary-vectors` | Embedding generation, compatibility and vector storage |
| `reliquary-history` | Nonmutating historical reconstruction |
| `reliquary-switchboard` | Provider transport, routes and related config/security contracts |
| `reliquary-project-environment` | Validated Lore/Git path and project authority |
| `reliquary-import` | Historical import coordination, but not its low-level epoch store |
| `reliquary-maintenance` | Authorized prune/rewrite/reconcile/repair, but not its low-level store |
| `reliquary-runtime-gateway` | REL/PHY runtime execution, sessions, host source integration |

## Five necessary support crates

The 34-component logical graph is acyclic, but **collapsing all stores and services into just 20 physical crates makes two physical cycles**. Perception's service invokes Retrieval while Retrieval reads Perception's store; History reads Import/Maintenance stores while those services invoke History and Runtime relies on those owners.

To keep the approved owner semantics and a physically compilable graph, keep these five *small, real authority seams*, not general-purpose managers:

| Support crate | Reason it must not be folded into the service crate |
| --- | --- |
| `reliquary-perception-store` | Retrieval may read accepted Entity data without importing the higher-level Perception service |
| `reliquary-import-store` | Historical reconstruction/lifecycle read import receipts without depending on Import coordination |
| `reliquary-maintenance-store` | History can inspect durable repair/prune evidence without depending on the mutating Maintenance service |
| `reliquary-external-index-store` | Owner lifecycle can open host-index state without importing Runtime/Gateway |
| `reliquary-owner-lifecycle` | Validated REL/PHY aggregate open/replay must remain below History, Import, Maintenance and Runtime |

This yields 25 extracted libraries and the existing `reliquary-memory` **legacy product package** in the workspace. The product remains the present functional monolith. To avoid colliding with its existing Cargo package name (reliquary-memory), the new Memory domain library is reliquary-memory-core, while its logical identity remains memory. No production APIs are migrated in Step 2.

Other folded logical components remain distinct **inside** their crate: `pure-codec` and `review-clocks` in Container; `config-security` in Switchboard; owner stores inside their owning libraries where they do not induce cycles; `external-sources` inside Runtime/Gateway pending the separately deferred redesign. The 35-component logical allowlist remains valid inside multi-component crates even though Cargo only checks cross-crate dependencies.

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

This step reserves names, build units, dependency permissions, and the adapted DAG **only**. The empty `src/lib.rs` files deliberately export no behavior. Cargo can check the workspace layout now, but **cannot yet prove that the legacy monolith follows these boundaries**. That proof comes naturally as code is moved to the crates, imports go through compiled crate APIs, and the monolithic copies are removed.

Next work is a **real Container extraction**, including writer, inspection/repair and clock contracts, while keeping the legacy product functional. No crate is marked implemented until it owns its production implementation and tests.
