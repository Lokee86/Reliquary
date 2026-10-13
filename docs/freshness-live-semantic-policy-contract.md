# Live semantic Freshness policy/API contract

Parent: [Documentation index](INDEX.md).

## Purpose

Freeze Step 2 of the [linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md): the live F8 ownership, policy interface, eligibility and exactly-once event contract.

## Overview

**Date:** 2026-10-12. **Status:** N1 corrected design contract; fragment-source implementation is pending N2. The previous turn-source runtime remains prospective opt-in live v1; [Step 12 verification](freshness-semantic-release-verification-2026-10-12.md) owns the standalone release disposition. Historical native primitives exist; retained-corpus acceptance remains blocked. This document refines [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md). Numerical calibration belongs to Steps 3–4; pure implementation, producer timing, storage encoding and transformation verification belong to Steps 5–11.

Live semantic reinforcement changes the existing owner-local Freshness score directly. It never invokes graph propagation, creates accepted-use receipts, launches Dream, or produces recursive activity. A genuine later accepted-use receipt still independently earns the existing +25 propagation; first-cycle Dream retains its +50 contract.

### Policy object and validation

The planned graph-independent module owns a validated `SemanticFreshnessPolicy`. Its immutable identity consists of a semantic policy version and a canonical digest of the complete configuration. This is distinct from `FRESHNESS_POLICY_VERSION`, which currently versions decay, access/linkage principals and hop costs. Both identities are pinned in accepted semantic evidence; changing semantic configuration cannot reinterpret previously accepted effects.

| Field | Frozen meaning |
| --- | --- |
| semantic policy version | Positive version of threshold, curve, reduction, eligibility and numerical evaluation rules. |
| cosine threshold | Finite value in [-1, 1]; equality qualifies. The calibrated production threshold is 0.75. |
| bonus curve | Explicit deterministic monotone cosine-to-integer mapping; zero below threshold, qualifying bonuses in 1..=10. Includes rounding rules and curve parameters. |
| per-Memory per-fragment bonus cap | The calibrated cap is 10; applied to the strongest match, never a sum over comparisons for that fragment. |
| usable candidate limit | The calibrated limit is 64 distinct eligible targets across one source fragment. |
| numerical evaluation revision | Fixes vector traversal, cosine accumulation, normalization and rounding order so worker count cannot alter ranking or effects. |

Step 4 calibrated the numerical configuration; N1 preserves its constants while correcting the source unit. A production event pins exactly one configuration; profile rotation or policy rotation never grants a second credit for an old fragment. Unsupported versions or digest/configuration mismatch fail explicitly. N2 must pin corrected semantic evidence without reinterpreting previously accepted effects.

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

A semantic source is one genuine retained Archive fragment, including mixed-turn fragments. Use its existing Archive vector row; do not split it, create exact-turn fragments, or separately embed its turns for Freshness. Retrieval/context/summary echoes are excluded.

At the selected Archive/vector cut, Archive resolves every represented source turn to its first accepted activity position. The fragment availability position is the maximum of those activity-bearing positions. All represented identities and immutable fragment/vector bindings are pinned; missing or foreign bindings are unavailable evidence, never guessed positions. Non-turn content adds no activity tick.

One fragment has at most one effect per target under the accepted semantic policy domain. Overlapping fragments are independent observations, but visiting a fragment through several contained turns does not multiply it. Re-embedding, profile rotation, worker retries and later targets do not reopen an accepted fragment event. Vector generation/profile/row digests are replay evidence only.

Pending vectors produce retryable readiness without a completion receipt. Complete unusable evidence may produce an explicit empty event with coverage. Incomplete search publishes neither partial effects nor completion.

### Eligible targets and self-source exclusion

All conditions are conjunctive and checked before ranking/candidate limits:

- The target is a Memory in this same authorized REL owner. Dependency RELs and PHY Memories are excluded; authorization alone does not supply a foreign decay clock.
- It is nonarchived, in settled `knowledge` or `canonical` lifecycle, and has an authoritative Freshness admission no later than the fragment availability cut. Extracted, unresolved duplicate/retired and unadmitted targets are excluded. Fresh/Stale/Dormant score state never excludes an otherwise eligible target.
- Its authoritative birth position is strictly earlier than fragment availability. Births at that position, future births and missing birth/admission proof are excluded; legacy targets require the existing explicit enrollment baseline, never an invented historical birth.
- Its provenance does not identify any represented turn as a creating/content/grounding source. The producer supplies the complete newly-created-Memory cohorts from all represented source turns, including asynchronous publication; those IDs are excluded even if encountered later. This guards self-source credit independently of numeric-cut checks.
- The vector is bound to the target's current immutable MemoryBodyId at the frozen evaluation cut and to the exact selected CompatibilityProfileId. Dimensions, normalization and endpoint compatibility must validate. Source and target generation evidence must be valid for their respective selected snapshots; generation numbers from different stores are not compared for equality.
- Components and computed cosine are finite, dimensions match, and both vector norms are nonzero. Invalid rows cannot become candidates or consume the usable budget.

Current lifecycle/body eligibility is evaluated at the frozen processing cut; birth/admission and self-source checks protect the accepted fragment availability cut. This does not promise historical lifecycle reconstruction. Delayed/reconciled events replay pinned effects rather than searching today's vectors.

### Reduction, ordering and bounded search

For each eligible MemoryId, retain the highest valid cosine over compatible comparisons for this source fragment. Equal matches choose the smallest immutable fragment ID, then the smallest immutable vector binding identity. Across targets, sort by descending cosine, then ascending MemoryId byte order. Validate cosine in [-1, 1] using the frozen numerical rules; do not depend on completion order or an unstable partial comparison.

Apply the inclusive threshold before selecting at most the policy's usable candidate limit. Compute the monotone bonus once per selected target and cap it once. Return effects in MemoryId order. There is no fragment-count multiplier, graph expansion, sum over paths or recursive event. Negative/Dormant records can reactivate; reinforcement uses existing lazy decay, remainder preservation and -100/+100 saturation at the accepted REL cut.

The usable limit counts distinct eligible above-threshold Memories for this fragment. Future, self-source, wrong-owner, invalid, duplicate and incompatible candidates do not consume it. A raw top-K followed by filtering is insufficient. The adapter must continue scanning/paging until it proves the eligible ranking is complete or fills the limit with a correctness-preserving search bound. An explicit resource exhaustion/incomplete-search result publishes no partial event or completion receipt and remains retryable. Exact streaming scan is a valid initial implementation; temporary candidate memory is bounded, while calibration must measure total vector work. No approximate-neighbor recall contract is silently introduced.

## Event identity, durability and replay

The corrected durable identity is based on owner UUID + semantic policy domain + immutable fragment identity. Positions, profile, generation, row digests, worker count and effects are excluded. Configuration rotation within the accepted domain cannot award the fragment again. N2 must freeze the typed encoding and duplicate lookup before accepting corrected events; this design does not prescribe a new public format or a maintained pre-release migration framework. The former `semantic-live-v1` owner/conversation/node identity describes the previous runtime only, as recorded in [Step 12 evidence](freshness-semantic-release-verification-2026-10-12.md).

Reserve distinct semantic producer kind 3 (existing Dream/use kinds are 1/2), subject to codec validation in Step 8. Semantic effects are validated by their own local bonus contract; do not weaken `FreshnessPolicy::is_valid_event_principal` or submit variable bonuses to the +25/+50 propagation validator.

Extend the existing Freshness event/compact-receipt representation with the minimum source and policy evidence: stable source identity, accepted REL position, base and semantic policy identities, selected profile, frozen source/target evidence digest, and sorted local effects/normal postimages. Empty completed events retain identity and completion evidence. Raw candidate lists, per-match cosine and embeddings are discarded after acceptance. The evidence digest commits to canonical source fragment/vector and selected target body/vector bindings; it is not a second event log.

Semantic proof is typed source/vector evidence, not fabricated Graph/Community proof. Step 8 must make the existing graph-cut/replay machinery distinguish this local proof instead of using a fake graph version zero as authority.

Resolve an existing source receipt before rerunning search: an exact retry returns the stored completion/effects. A submitted conflicting policy, source evidence or explicit effect payload for that identity fails; it is never a second credit or silent overwrite. Ordinary retry after current profile rotation returns the original receipt. Receipt retention/compaction must preserve duplicate resolution through the existing authoritative Freshness history, not rely solely on a recent in-memory cache.

Canonical scoring order remains accepted REL position followed by stable event ID, consistent with current Freshness ordering. Admission/birth prerequisites precede scoring at the cut. Semantic IDs share this order with existing Dream/use IDs; arrival order is not authority. A late event requires existing canonical replay/postimage rebuild, not reinforcement of today's score. Step 8 implements and tests this order, including saturation-sensitive mixed events.

On reconciliation, resolve every represented turn in the destination Archive and recompute fragment availability from its accepted positions and replay the pinned local deltas; preserve stable source identity and policy/profile evidence. Missing source mapping, unsupported evidence or conflicting same-source effects fail explicitly. Repack/copy/reclamation preserve accepted evidence and receipts without rerunning similarity or propagation. Divergent policies require the explicit transition rules from Step 4 and transformation closure in Step 11.

## Related docs

- [Step 4 frozen v1 constants, evidence and activation rules](freshness-semantic-calibration-2026-10-11.md)
- [Step 1 baseline](freshness-semantic-historical-baseline-2026-10-11.md)
- [Linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)
- [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md)
- [Architecture](architecture.md), [invariants](invariants.md), [storage format](storage-format.md) and [maintainer map](maintainer-map.md)
- [Freshness completion execution plan](freshness-completion-execution-plan-2026-10-03.md)
- [Historical reconstruction specification](freshness-historical-import-reconstruction-spec-2026-10-03.md)

## Notes

N1 freezes intended behavior only. N2 must update runtime receipt identity, candidate evidence, self/cohort exclusions, readiness and transformation tests. Current API/storage documentation continues to describe shipped turn-source behavior until those gates pass. The numerical constants remain the calibrated inclusive 0.75 threshold, +1..+10 curve and top-64 eligible targets, now applied per fragment; live effects remain local. The design oracles in `scripts/test_freshness_normalization_contract.py` protect the intended rules without claiming runtime acceptance.
