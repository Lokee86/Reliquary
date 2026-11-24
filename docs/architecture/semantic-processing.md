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
| `reliquary-entity-resolution` | Resolves possible entity matches and produces association proposals, including decision-backed paths. It also judges pair-specific cross-REL identity evidence using bounded structured output and validated citations. It does not create entity links or publish identity receipts. |
| `reliquary-freshness` | Scores and propagates freshness/historical effects and plans due transitions. Durable freshness history is owned by `reliquary-freshness-store`. |
| `reliquary-insomnia` | Extracts and validates memory candidates against accepted episode/turn evidence, retaining source identity and ownership classification. It does not publish memories. |
| `reliquary-retrieval` | Routes owner-scoped candidates and scores graph/vector/semantic evidence for retrieval. It does not authorize or deliver results to a host. |
| `reliquary-vectors` | Validates embedding batches and compares compatibility probes/profiles. Durable profiles and vector rows remain in `reliquary-vector-store`. |
| `reliquary-project-environment` | Reads project files and Git/Lore history into bounded snapshots/references for correlation; Application validates and attaches them to owner workflows. |

Together with Application and the runtime, these packages complete the 23-package workspace described in [system architecture](system.md). This grouping is responsibility-based; the [DAG](../../architecture/target-crate-dag-2026-11-03.json) remains the exact dependency contract.

## Flow, failure, and invariants

Inputs are grounded in accepted owner records or explicit caller-provided evidence, bounded before expensive operations where the contract requires it, and validated before a result leaves the semantic owner. Model-backed classifications and decisions are treated as fallible inputs: malformed, incomplete, out-of-range, or unavailable responses become errors or no result, not implicit durable facts. Stable source references and deterministic compatibility identities permit workflow retries without claiming a semantic result is already published.

Stateful lifecycle and recovery are not hidden in these computation packages. For example, candidate reevaluation after an owner change belongs to Application's recheck step; durable write/replay belongs to owner-session/stores. The exact external model behavior is not proven by unit tests and is governed by configured endpoints in [Switchboard](switchboard.md).

Archive hybrid search uses the shared lexical scoring and hybrid weights with exact cosine on the accepted packed rows. Each lexical posting records a whole-word match and its non-overlapping substring frequency capped at four, preserving the score from a full text scan. Both the durable projection and the in-memory rebuild use the same analyzer and count representation. Ranking breaks ties by row ordinal. Semantic Memory search defaults to exact cosine across all available vector candidates for the selected profile. An explicitly requested Community route uses the persisted snapshot and falls back to the global scan if routing is unavailable. Manual Community recompute uses the original deterministic Leiden scan-and-merge in the Graph crate. The solver receives the owner session's eligible Memory IDs and unique structural pairs and returns a partition and modularity. Application prepares the result for authorized publication through owner-session.

Agentic REL retrieval applies an explicit half-open source or mentioned time window to the authorized Memory cut before Freshness limits the candidate offer. Application passes the resolved filter and bounded evidence to the model and projects the same evidence into previews. Refinement rechecks previously declined candidates against the live cut, and traversal checks both the anchor and its neighbors. A new query can replace or clear the filter. Confirmed selections still suppress duplicates across queries in the same external turn.

Gateway binds the filter to the query frontier in prepared delivery manifests. Recovery restores that window with the manifest frontier or keeps the current window when merging confirmed selections into a newer query. Unfiltered manifests omit the field and retain their original digest bytes. Inspection and preparation publish no accepted-use receipt.

The Entity resolver can nominate pairs by looking up equal keys in two complete owner indexes. Keys represent whole Entity names or aliases and selected structured identifiers found near a resolved mention. Keys shared by more than 32 Entities in either owner are too common to nominate a pair alone. Multiple matching keys produce one candidate. The index contains Entity IDs and key digests. Nomination requires no model call and carries no judgment about identity.

Comparison starts with the Memories that mention the two Entities. Each owner follows current, resolved mention links and takes up to four excerpts from directly sourced, active Memories. A mention identifies the referent at one position in the text. The surrounding words supply the evidence for comparison. Existing grounded Relationships add participant roles and source quotes when available. A pair supported only by names, or only by a common source, stays unresolved. When both sides have independent witnesses, one call to the configured Entity endpoint judges the pair and selects the supporting excerpts. For a match, the model evaluates whether the sources identify their endpoints through the same distinguishing fact, allowing each source its own wording. Core checks each excerpt against its own source and requires independent support on both sides. A short named participant reference can also be grounded through compatible Relationships and roles. Application prepares the validated judgment for publication in the recording REL. For a proposed distinction, the evaluator explains what each cited excerpt establishes using a bounded category. The resolver accepts the distinction when each side independently identifies a separate referent. A description or association alone leaves the pair unresolved, and conflicting accounts can remain unresolved even when both are well sourced. These judgments can change as new evidence arrives. Consumers read the `effective` field from pair inspection. An applicable operator confirmation keeps the pair separate even when the evaluator finds a match. [Operator review and undo](../public-product-api.md#confirm-or-undo-separation)

## Observation inference

The shared Memory Web auditor packs exact owner-supplied passages into one structured discovery request with separate Ego and Perception candidate lists. Ego accepts Fresh Memory evidence. Perception also accepts eligible Stale and Dormant Memory evidence and current verified Observations under the relational evidence rules. Each list validates its own citations, coverage and optional owner-local lookup results.

Perception synthesizes one exact statement and its citations, generates separate support, challenge, qualification and change receptors, and uses a bounded owner back-check for additional evidence. The native Jev Verify contract judges one exact draft per request. A material challenge or qualification in a back-check vetoes clearance. One bounded resynthesis can produce a distinct draft for verification.

Existing Observations receive one native Jev contribution request per head and bounded hit batch. Time and scope choices suppress incompatible challenge and qualification triggers. Definitive source invalidation goes straight to review. Review verifies the current claim when its proof remains eligible and uses the same Verify contract for a changed draft. [Tested contracts](../planning/perception-implementation-plan-2026-11-24.md#release-contracts)

## Code map and tests

- Observation discovery and decisions: [shared audit](../../crates/reliquary-memory-audit/src/lib.rs), [Ego extraction](../../crates/reliquary-ego/src/lib.rs) and [Perception](../../crates/reliquary-perception/src/lib.rs). The [structured stages test](../../crates/reliquary-perception/tests/structured_stages.rs) covers proposal profiles, exact slots, synthesis and receptor binding.
- Representative owners: [`chronos`](../../crates/reliquary-chronos/src/lib.rs), [`context-engine`](../../crates/reliquary-context-engine/src/lib.rs), [`dream`](../../crates/reliquary-dream/src/lib.rs), [`entity-resolution`](../../crates/reliquary-entity-resolution/src/lib.rs), [`freshness`](../../crates/reliquary-freshness/src/lib.rs), [`insomnia`](../../crates/reliquary-insomnia/src/lib.rs), [`retrieval`](../../crates/reliquary-retrieval/src/lib.rs), [`vectors`](../../crates/reliquary-vectors/src/lib.rs), and [`project-environment`](../../crates/reliquary-project-environment/src/lib.rs).
- Focused tests include temporal parse/detection and inference tests in `reliquary-chronos/src/`; budget and evidence tests in `reliquary-context-engine/src/behavior_tests.rs`; candidate ranking/frontier/evaluator tests in `reliquary-dream/src/`; `reliquary-vectors/src/contract_tests.rs`; and Git/project snapshot tests in `reliquary-project-environment/src/`.
- Cross-owner provenance, retry, and publication protections are tested in Application modules linked from [Application workflows](application.md).

## Extension seams

Add pure or bounded interpretation to the semantic owner for that domain. Add persistent facts to the matching store and expose them through owner-session. Add cross-domain policy and publication to Application. Add provider-selection changes to Switchboard without introducing store access or durable product policy into semantic crates.

## Related docs

- [Reliquary source and binary distribution terms](../../README.md#license)
- [Application workflows](application.md)
- [Storage and durable state](storage.md)
- [Switchboard](switchboard.md)

## Notes

Model outcomes are inputs to product workflows; this page does not promise any model's accuracy or availability.
