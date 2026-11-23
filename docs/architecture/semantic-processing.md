# Semantic processing

Parent index: [Reliquary architecture](./INDEX.md)

## Purpose

Map the semantic-processing packages to their responsibilities, state boundaries, and protecting tests.

## Overview

Semantic packages interpret accepted evidence and return bounded views, candidates, scores, or decisions. They do not publish durable owner state. Application owns the product workflow that validates and commits a result; owner-session owns any grouped durable mutation. Persistent compatibility profiles, vectors, freshness state, graph relations, and memory records stay in their respective stores.

## Responsibility owners

| Crate | Responsibility and boundary |
| --- | --- |
| `reliquary-chronos` | Parses and normalizes temporal expressions, resolves relative/calendar/recurrence boundaries, and returns temporal assessments. It does not write freshness state. |
| `reliquary-context-engine` | Builds bounded context/compaction views from supplied messages and evidence, using explicit token budgets and source references. It does not fetch or persist owner data. |
| `reliquary-dream` | Ranks candidates and classifies/verifies proposed graph organization. It produces a draft/decision; Application and owner-session own recheck and publication. |
| `reliquary-entity-resolution` | Resolves possible entity matches and produces association proposals, including decision-backed paths. It does not create entity links in the store. |
| `reliquary-freshness` | Scores and propagates freshness/historical effects and plans due transitions. Durable freshness history is owned by `reliquary-freshness-store`. |
| `reliquary-insomnia` | Extracts and validates memory candidates against accepted episode/turn evidence, retaining source identity and ownership classification. It does not publish memories. |
| `reliquary-retrieval` | Routes owner-scoped candidates and scores graph/vector/semantic evidence for retrieval. It does not authorize or deliver results to a host. |
| `reliquary-vectors` | Validates embedding batches and compares compatibility probes/profiles. Durable profiles and vector rows remain in `reliquary-vector-store`. |
| `reliquary-project-environment` | Reads project files and Git/Lore history into bounded snapshots/references for correlation; Application validates and attaches them to owner workflows. |

Together with Application and the runtime, these packages complete the 23-package workspace described in [system architecture](system.md). This grouping is responsibility-based; the [DAG](../../architecture/target-crate-dag-2026-11-03.json) remains the exact dependency contract.

## Flow, failure, and invariants

Inputs are grounded in accepted owner records or explicit caller-provided evidence, bounded before expensive operations where the contract requires it, and validated before a result leaves the semantic owner. Model-backed classifications and decisions are treated as fallible inputs: malformed, incomplete, out-of-range, or unavailable responses become errors or no result, not implicit durable facts. Stable source references and deterministic compatibility identities permit workflow retries without claiming a semantic result is already published.

Stateful lifecycle and recovery are not hidden in these computation packages. For example, candidate reevaluation after an owner change belongs to Application's recheck step; durable write/replay belongs to owner-session/stores. The exact external model behavior is not proven by unit tests and is governed by configured endpoints in [Switchboard](switchboard.md).

Archive hybrid search uses the shared lexical scoring and hybrid weights with exact cosine on the accepted packed rows. Each lexical posting records a whole-word match and its non-overlapping substring frequency capped at four, preserving the score from a full text scan. Ranking breaks ties by row ordinal. Semantic Memory search defaults to exact cosine across all available vector candidates for the selected profile. An explicitly requested Community route uses the persisted snapshot and falls back to the global scan if routing is unavailable. Manual Community recompute uses the original deterministic Leiden scan-and-merge in the Graph crate. The solver receives the owner session's eligible Memory IDs and unique structural pairs and returns a partition and modularity. Application prepares the result for authorized publication through owner-session.

## Code map and tests

- Representative owners: [`chronos`](../../crates/reliquary-chronos/src/lib.rs), [`context-engine`](../../crates/reliquary-context-engine/src/lib.rs), [`dream`](../../crates/reliquary-dream/src/lib.rs), [`entity-resolution`](../../crates/reliquary-entity-resolution/src/lib.rs), [`freshness`](../../crates/reliquary-freshness/src/lib.rs), [`insomnia`](../../crates/reliquary-insomnia/src/lib.rs), [`retrieval`](../../crates/reliquary-retrieval/src/lib.rs), [`vectors`](../../crates/reliquary-vectors/src/lib.rs), and [`project-environment`](../../crates/reliquary-project-environment/src/lib.rs).
- Focused tests include temporal parse/detection and inference tests in `reliquary-chronos/src/`; budget and evidence tests in `reliquary-context-engine/src/behavior_tests.rs`; candidate ranking/frontier/evaluator tests in `reliquary-dream/src/`; `reliquary-vectors/src/contract_tests.rs`; and Git/project snapshot tests in `reliquary-project-environment/src/`.
- Cross-owner provenance, retry, and publication protections are tested in Application modules linked from [Application workflows](application.md).

## Extension seams

Add pure or bounded interpretation to the semantic owner for that domain. Add persistent facts to the matching store and expose them through owner-session. Add cross-domain policy and publication to Application. Add provider-selection changes to Switchboard without introducing store access or durable product policy into semantic crates.

## Related docs

- [Application workflows](application.md)
- [Storage and durable state](storage.md)
- [Switchboard](switchboard.md)

## Notes

Model outcomes are inputs to product workflows; this page does not promise any model's accuracy or availability.
