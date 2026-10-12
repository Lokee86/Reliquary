# Live semantic Freshness policy/API contract

Parent: [Documentation index](INDEX.md).

## Purpose

Freeze Step 2 of the [linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md): the live F8 ownership, policy interface, eligibility and exactly-once event contract.

## Overview

**Date:** 2026-10-11. **Status:** accepted implementation contract; F8 remains unimplemented. This document refines [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md). Numerical calibration belongs to Steps 3–4; pure implementation, producer timing, storage encoding and transformation verification belong to Steps 5–11.

Live semantic reinforcement changes the existing owner-local Freshness score directly. It never invokes graph propagation, creates accepted-use receipts, launches Dream, or produces recursive activity. A genuine later accepted-use receipt still independently earns the existing +25 propagation; first-cycle Dream retains its +50 contract.

### Policy object and validation

The planned graph-independent module owns a validated `SemanticFreshnessPolicy`. Its immutable identity consists of a semantic policy version and a canonical digest of the complete configuration. This is distinct from `FRESHNESS_POLICY_VERSION`, which currently versions decay, access/linkage principals and hop costs. Both identities are pinned in accepted semantic evidence; changing semantic configuration cannot reinterpret previously accepted effects.

| Field | Frozen meaning |
| --- | --- |
| semantic policy version | Positive version of threshold, curve, reduction, eligibility and numerical evaluation rules. |
| cosine threshold | Finite value in [-1, 1]; equality qualifies. 0.75 is a calibration candidate, not an approved production default. |
| bonus curve | Explicit deterministic monotone cosine-to-integer mapping; zero below threshold, qualifying bonuses in 1..=100. Includes rounding rules and curve parameters. |
| per-Memory per-turn bonus cap | Positive integer <=100; applied to the strongest match, never a sum over fragments. |
| usable candidate limit | Positive number of distinct eligible targets across the entire accepted turn, not per fragment. |
| numerical evaluation revision | Fixes vector traversal, cosine accumulation, normalization and rounding order so worker count cannot alter ranking or effects. |

Step 3 accepts explicit experimental configurations without publishing production receipts. Step 4 selects the initial production configuration and activation/transition rules. A production event pins exactly one configuration; profile rotation or policy rotation never grants a second credit for an old turn. Unsupported versions or digest/configuration mismatch fail explicitly. No default threshold, curve, limit or transition policy is invented in Step 2.

### Authority and planned seams

Names below specify responsibilities and data contracts for subsequent implementation; they are not shipped Rust APIs.

| Seam | Input/output and owner |
| --- | --- |
| accepted-turn resolver | Archive resolves exact owner UUID, conversation ID and node ID to the first accepted REL activity position, verifies immutable source equality and activity-bearing user/assistant role. Raw caller-supplied counters are insufficient. |
| genuine fragment provider | Archive ingestion/vector owner supplies the complete selected genuine-source fragment set, immutable fragment/source bindings, compatibility profile and retained generation proof. Generated retrieval/context/summary echoes are excluded. Step 6 fixes the concrete readiness point. |
| semantic candidate adapter | Retrieval/vector owner validates source and current Memory-body bindings, searches only the authorized local owner, and returns validated target matches and bounded coverage. It has no scoring authority. |
| pure semantic planner | Freshness policy module validates policy/compatibility, reduces strongest matches, orders and caps candidates, returns a sorted MemoryId-to-local-bonus map. It takes no Graph/Community or mutable store. |
| semantic event acceptor | Existing Freshness owner resolves duplicate identity before candidate work, validates source/policy/evidence, and atomically accepts the receipt and direct effects through its journal/replay path. |

The facade coordinates these concrete owners. The container supplies physical publication mechanics only. Fragment/vector indexes remain rebuildable evidence access; they do not own event identity or score authority. No generalized owner framework, separate semantic ledger, parallel score or per-Memory semantic revision is introduced.

### Source identity and evidence

A source is `(owner_uuid, conversation_id, node_id)`, not a timestamp, ingestion call ID, fragment ID, vector generation or numeric REL position. Its accepted position is verified against Archive activity and used as the scoring cut. Duplicate turn ingestion retains the original identity/position and does not create a new event.

One production attempt selects one compatible profile and a frozen fragment/vector evidence cut for the turn. Every source vector must resolve to genuine content of that exact accepted node; a mixed-turn fragment must be split/rebuilt into turn-only evidence or excluded with coverage reported. Multiple fragments, repeated text and repeated processing do not create more credits. Retrieval results, generated summaries and inserted context are not genuine new source content.

No fragment vectors yet available is a retryable readiness outcome, with no completion receipt. A complete provider result with no usable vectors may finish as an explicit empty event with coverage; it must not be confused with pending work. Once an event is accepted, later vector generation, profile rotation, source regeneration or newly created targets cannot reopen it. Regeneration of an enrolled Archive preserves existing receipts.

### Eligible targets and self-source exclusion

All conditions are conjunctive and checked before ranking/candidate limits:

- The target is a Memory in this same authorized REL owner. Dependency RELs and PHY Memories are excluded; authorization alone does not supply a foreign decay clock.
- It is nonarchived, in settled `knowledge` or `canonical` lifecycle, and has an authoritative Freshness admission no later than the source cut. Extracted, unresolved duplicate/retired and unadmitted targets are excluded. Fresh/Stale/Dormant score state never excludes an otherwise eligible target.
- Its authoritative birth position is strictly earlier than the accepted turn. Births at the turn, future births and missing birth/admission proof are excluded; legacy targets require the existing explicit enrollment baseline, never an invented historical birth.
- Its provenance does not identify this exact turn as the creating source. The producer supplies the complete newly-created-Memory cohort from that source, including asynchronous publication; those IDs are excluded even if encountered later. This guards self-source credit independently of numeric-cut checks.
- The vector is bound to the target's current immutable MemoryBodyId at the frozen evaluation cut and to the exact selected CompatibilityProfileId. Dimensions, normalization and endpoint compatibility must validate. Source and target generation evidence must be valid for their respective selected snapshots; generation numbers from different stores are not compared for equality.
- Components and computed cosine are finite, dimensions match, and both vector norms are nonzero. Invalid rows cannot become candidates or consume the usable budget.

Current lifecycle/body eligibility is evaluated at the frozen processing cut; birth/admission and self-source checks protect the accepted source cut. This does not promise historical lifecycle reconstruction. Delayed/reconciled events replay pinned effects rather than searching today's vectors.

### Reduction, ordering and bounded search

For each eligible MemoryId, retain the highest valid cosine over all selected source fragments. Equal matches choose the smallest immutable fragment ID, then the smallest immutable vector binding identity. Across targets, sort by descending cosine, then ascending MemoryId byte order. Validate cosine in [-1, 1] using the frozen numerical rules; do not depend on completion order or an unstable partial comparison.

Apply the inclusive threshold before selecting at most the policy's usable candidate limit. Compute the monotone bonus once per selected target and cap it once. Return effects in MemoryId order. There is no fragment-count multiplier, graph expansion, sum over paths or recursive event. Negative/Dormant records can reactivate; reinforcement uses existing lazy decay, remainder preservation and -100/+100 saturation at the accepted REL cut.

The usable limit counts distinct eligible above-threshold Memories across the turn. Future, self-source, wrong-owner, invalid, duplicate and incompatible candidates do not consume it. A raw top-K followed by filtering is insufficient. The adapter must continue scanning/paging until it proves the eligible ranking is complete or fills the limit with a correctness-preserving search bound. An explicit resource exhaustion/incomplete-search result publishes no partial event or completion receipt and remains retryable. Exact streaming scan is a valid initial implementation; temporary candidate memory is bounded, while calibration must measure total vector work. No approximate-neighbor recall contract is silently introduced.

### Step 7 candidate adapter status

The crate-private candidate adapter now enumerates only the current owner-local Memory vector bindings for the accepted source profile. It filters lifecycle, Freshness admission, exact source provenance and the complete newly-created source cohort before the usable candidate limit, and delegates strongest-match reduction and deterministic ordering to the pure semantic policy. It reports scan coverage and remains read-only; durable event identity and score application remain Step 8/9.

## Event identity, durability and replay

The event namespace is `semantic-live-v1`. Its stable ID is the string prefix `semantic-live-v1:` followed by lowercase hexadecimal SHA-256 of the following concatenation: ASCII `reliquary:freshness:semantic-live:v1`, one zero byte, the owner's 16 UUID bytes, the conversation ID's UTF-8 byte length as little-endian u64 followed by its bytes, then the node ID's UTF-8 byte length as little-endian u64 followed by its bytes. IDs use exact Archive strings without Unicode normalization. Step 8 implements this frozen identity format; it is distinct from scoring-policy versions. REL position, policy version, profile, fragment count, worker count and effects are excluded from identity. Thus one accepted turn has one semantic event and at most one effect per Memory. Any codec version change must preserve duplicate lookup for already accepted source identities.

Reserve distinct semantic producer kind 3 (existing Dream/use kinds are 1/2), subject to codec validation in Step 8. Semantic effects are validated by their own local bonus contract; do not weaken `FreshnessPolicy::is_valid_event_principal` or submit variable bonuses to the +25/+50 propagation validator.

Extend the existing Freshness event/compact-receipt representation with the minimum source and policy evidence: stable source identity, accepted REL position, base and semantic policy identities, selected profile, frozen source/target evidence digest, and sorted local effects/normal postimages. Empty completed events retain identity and completion evidence. Raw candidate lists, per-match cosine and embeddings are discarded after acceptance. The evidence digest commits to canonical source fragment/vector and selected target body/vector bindings; it is not a second event log.

Semantic proof is typed source/vector evidence, not fabricated Graph/Community proof. Step 8 must make the existing graph-cut/replay machinery distinguish this local proof instead of using a fake graph version zero as authority.

Resolve an existing source receipt before rerunning search: an exact retry returns the stored completion/effects. A submitted conflicting policy, source evidence or explicit effect payload for that identity fails; it is never a second credit or silent overwrite. Ordinary retry after current profile rotation returns the original receipt. Receipt retention/compaction must preserve duplicate resolution through the existing authoritative Freshness history, not rely solely on a recent in-memory cache.

Canonical scoring order remains accepted REL position followed by stable event ID, consistent with current Freshness ordering. Admission/birth prerequisites precede scoring at the cut. Semantic IDs share this order with existing Dream/use IDs; arrival order is not authority. A late event requires existing canonical replay/postimage rebuild, not reinforcement of today's score. Step 8 implements and tests this order, including saturation-sensitive mixed events.

On reconciliation, map the exact conversation/node source to the destination's accepted activity position and replay the pinned local deltas; preserve stable source identity and policy/profile evidence. Missing source mapping, unsupported evidence or conflicting same-source effects fail explicitly. Repack/copy/reclamation preserve accepted evidence and receipts without rerunning similarity or propagation. Divergent policies require the explicit transition rules from Step 4 and transformation closure in Step 11.

## Related docs

- [Step 4 frozen v1 constants, evidence and activation rules](freshness-semantic-calibration-2026-10-11.md)
- [Step 1 baseline](freshness-semantic-historical-baseline-2026-10-11.md)
- [Linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)
- [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md)
- [Architecture](architecture.md), [invariants](invariants.md), [storage format](storage-format.md) and [maintainer map](maintainer-map.md)
- [Freshness completion execution plan](freshness-completion-execution-plan-2026-10-03.md)
- [Historical reconstruction specification](freshness-historical-import-reconstruction-spec-2026-10-03.md)

## Notes

Step 2 verification on 2026-10-11: `cargo test --locked --lib freshness:: -j 2 -- --nocapture` passed 28 tests (0 failures) using the isolated Step 1 target directory and `CARGO_PROFILE_TEST_DEBUG=0`; existing dead-code warnings remain. Both structural and `--changed-from origin/main` documentation-policy checks passed. `git diff --check` passed. No runtime source or public API changed; the full root/CLI release gates remain Step 12 work.

This contract was checked against `src/freshness/policy.rs`, `src/archive/archive_activity.rs`, `src/facade/cva_turn_ingest.rs`, `src/facade/cva_memory_vectors.rs`, `src/retrieval/compatibility_profile_model.rs`, `src/freshness_storage.rs`, `src/freshness/freshness_storage_ordering.rs` and `src/facade/cva_reconcile_freshness.rs`, plus the Step 1 seam inventory. Existing ingestion does not yet supply genuine fragment embeddings, existing event proof is graph-oriented, and reconciliation does not yet validate semantic evidence. These are explicit later implementation gates, not shipped behavior.

Step 2 freezes ownership and event semantics. Step 8 persists the frozen semantic identity/evidence through the existing Freshness journal. Step 11 repairs producer-kind/empty-event decoding and preserves typed evidence through source-identity-based reconciliation, with passing retry/rotation/reopen and transformation fixtures. Step 4 supplies numerical choices; Step 6 selects the concrete fragment-readiness seam; Step 8 implements the frozen identity encoding and versioned storage; Step 11 proves transformations. Historical import propagation remains a separate later mode and cannot enter this live contract.
