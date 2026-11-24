# System architecture

Parent index: [Reliquary architecture](./INDEX.md)

## Purpose

Explain the responsibilities and call direction of the current Reliquary workspace. This is an architecture owner, not a claim of final standards compliance or a replacement for exact API and CLI references.

## Overview

The workspace has 23 Cargo packages: the `reliquary-memory` root facade and 22 focused crates. The root facade exposes owner handles, immutable product values, configured runtime types, maintenance and management operations, and host interfaces from [`src/lib.rs`](../../src/lib.rs). The checked production edges are recorded in the [approved package DAG](../../architecture/target-crate-dag-2026-11-03.json) and guarded by [`scripts/check_architecture.py`](../../scripts/check_architecture.py). The guard resolves Cargo dependency aliases to package names, verifies known workspace packages point to their inventoried canonical manifests and declared package names match their local target, includes target-conditioned runtime dependencies and workspace-inherited declarations, and checks the resulting production edges against the approved graph. Development dependencies are reported separately as test-only edges and do not authorize production imports. The 23/76 figures describe the currently approved graph; a future graph change requires updating its specification, guard, and these owner pages together.

## Responsibilities and non-owners

The Graph crate pins `leiden-rs` 0.8.1 for deterministic Community calculation. Application uses the immutable owner-session input and the typed writer interface for recompute.

Owner-session uses MessagePack for its private checkpoint projection. The workspace and the separate consumer and migration workspaces pin the codec in their Cargo locks. Native REL and PHY record codecs retain authority over the owner files.

| Responsibility | Packages | Owner boundary |
| --- | --- | --- |
| Physical format and durable domain state | `reliquary-container`, `reliquary-archive`, `reliquary-memory-core`, `reliquary-graph`, `reliquary-perception-store`, `reliquary-ego-store`, `reliquary-freshness-store`, `reliquary-insomnia-store`, `reliquary-vector-store` | Container owns framing and physical writes; each domain store owns its records and replay. See [storage](storage.md). |
| Owner-scoped composition and grouped mutation | `reliquary-owner-session` | Opens REL and PHY owner sessions and is the mutable store boundary. See [owner sessions](owner-session.md). |
| Semantic proposals and derived results | `reliquary-chronos`, `reliquary-context-engine`, `reliquary-dream`, `reliquary-entity-resolution`, `reliquary-freshness`, `reliquary-insomnia`, `reliquary-retrieval`, `reliquary-vectors`, `reliquary-project-environment` | Computes bounded interpretations, candidates, rankings, context views, and compatibility assessments; does not publish durable state. See [semantic processing](semantic-processing.md). |
| Product workflows | `reliquary-application` | Validates and coordinates named cross-owner workflows, then asks owner sessions to publish. See [application](application.md). |
| Runtime access and provider routing | `reliquary-runtime-gateway`, `reliquary-switchboard` | Gateway owns mounted owner access, authorization, workers, queue and host lifecycle; Switchboard owns runtime config and provider adapters. See [Gateway](gateway.md) and [Switchboard](switchboard.md). |

The Rust call direction is intentionally constrained: semantic crates do not own persistence; Application may depend on semantic owners and owner-session value/commit APIs; Gateway composes Application and owner sessions but does not implement semantic policy or encode domain-store records. Container and writable stores do not escape to higher layers. The exact permitted package edges are the DAG contract, not this table.

## Flow and state

For a durable user change, a caller enters through the root API or CLI, the Gateway authenticates and scopes the request when operating through a trusted host, Application validates and prepares the workflow, and an owner session performs the owner-local publication. Container frames and syncs the resulting bytes; domain stores replay their own records on reopen. Semantic work can produce a proposal or decision, but a proposal becomes current state only through an explicit Application/owner-session workflow.

REL and PHY files are separate owner-scoped state domains. A `RelSession` or `PhySession` is the only cross-store composition point for one owner and kind; higher layers do not share a writable store across owners. Runtime queue and configuration journals have their own owners and formats, separate from semantic REL/PHY data.

## Failure, recovery, and invariants

The central invariants are single-writer owner authority, bounded and validated inputs, durable replayable publication, stable identities/provenance, and no implicit promotion of proposals into memory. Container refuses ordinary writes after a failed partial append until explicit repair/recovery. Owner-session fences revalidate staged cuts before grouped publication. Gateway persists a host handoff intent before calling the host and reconciles uncertain operations by stable ID instead of blindly redispatching. Exact format and workflow contracts belong to the linked storage, owner-session, and Gateway pages.

Tests protecting representative boundaries include Container reopen/partial-append behavior in [`crates/reliquary-container/src/lib.rs`](../../crates/reliquary-container/src/lib.rs), owner publication and reopen cases in [`crates/reliquary-application/src/lib.rs`](../../crates/reliquary-application/src/lib.rs), and uncertain host operation recovery in [`crates/reliquary-runtime-gateway/src/host_runtime.rs`](../../crates/reliquary-runtime-gateway/src/host_runtime.rs). These are implementation tests, not historical acceptance reports.

## Static policy and test dependencies

Repository-local Pitlord checks both resolved `imports` and Cargo `depends-on` evidence. Foundation, store, owner-session and semantic-service areas must remain independent of Application/Gateway in every dependency category; put cross-owner integration tests at higher orchestration, Root or consumer boundaries. Observed area cycles include test edges. This is distinct from the exact production-only package DAG: permitted Application/Gateway dev dependencies on lower stores are not mislabeled as production edges.

The vector store uses Lodestone v0.2.0 for packed row representation and Archive exact cosine ranking. Cargo pins the Lodestone revision across Root, CLI, the external consumer, and the offline history tools. REL and PHY publication continue through their owner sessions. Archive uses the locked Aho-Corasick library to compute substring frequencies when building its session-local lexical index. Retrieval uses the locked `time` and `serde` libraries for checked RFC3339 bounds and manifest-compatible temporal values. Application already depends on Retrieval and Chronos and combines their results with owner-session evidence.

Application uses the locked `chrono` and `chrono-tz` libraries to translate Chronos calendar expressions from the host's IANA timezone into the UTC bounds used by Archive retrieval. Root freezes those bounds for the search and keeps model-visible exact ranges within the admitted turn's authorization.

The native Freshness store uses SHA-256 to bind retained authority bytes. The offline history writer and comparison tool have separate workspaces. The FRS07 source reader runs from an isolated historical checkout outside the production dependency graph.

## Extension seams

Add persistent record behavior to the owning store and expose it through a narrow owner-session operation. Add a new product workflow in Application when it combines owners or defines publication policy. Add semantic capabilities as proposal/decision owners; keep provider selection in Switchboard and external host lifecycle in Gateway. Update the DAG and focused owner page whenever package ownership or call direction changes.

## Related docs

- [Reliquary source and binary distribution terms](../../README.md#license)
- [Storage and durable state](storage.md)
- [Owner sessions](owner-session.md)
- [Application workflows](application.md)
- [Semantic processing](semantic-processing.md)
- [Gateway and host lifecycle](gateway.md)
- [Switchboard](switchboard.md)

## Notes

Package count and approved edges are revision-specific architecture evidence, not permanent product limits.
