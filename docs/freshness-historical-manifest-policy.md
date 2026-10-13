# Historical Freshness manifest and timeline policy

Parent: [Documentation index](INDEX.md).

## Purpose

Freeze Step 13 / H1 of the [linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md) into a deterministic contract for the timeline planner, historical event producers and superseding Freshness checkpoint.

## Overview

**Status:** N3/N4 fragment-source and accepted-Archive-clock runtime implemented, frozen 2026-10-12. The version-3 descriptor below implements accepted chronology; native exclusive reconstruction/publication and recovery are implemented through C6. C7 freezes the contract and C8 implements bounded offline embedding/importer integration; C9–C12 release gates remain. Step 12 independently released prospective opt-in live F8. This contract resolves the choices left open in the [historical reconstruction design](freshness-historical-import-reconstruction-spec-2026-10-03.md); its precise rules govern Steps 14–25.

Reconstruction is an explicit one-shot operation on a newly generated, complete, single-owner REL import. It uses the final authorized graph, approximate provenance-based availability and exact existing Freshness mathematics. It creates no second clock, permanent event ledger or historical consumed-use receipts. Ordinary live semantic reinforcement remains local and non-propagating.

## Ownership and supported input

Archive owns immutable source identities, ancestry, content, timestamps, Episodes and accepted activity positions. Memory/Insomnia own source bindings and grouped publication identity; Dream owns first-cycle proof and duplicate representative authority; Graph/Community own completed topology/membership. Freshness owns the policy, resulting scores, checkpoint acceptance and retry identity. The facade coordinates these owners without acquiring their authority.

Version 1 requires:

- A durable REL UUID, the full deduplicated Archive activity prefix 1..N and its necessary ancestry, and a provably complete generated import cohort. Empty activity, missing ancestry, cycles, conflicting immutable identities or unrelated pre-existing Memories reject the operation.
- Every cohort member is backed by an Import Episode or explicit verified import-publication evidence. Ordinary enrollment is not retroactively labeled provisional. The exact provisional birth/admission/Dream scoring identities must be distinguishable before Step 21 can supersede them.
- Generation has settled: no open import Episode tail, pending Insomnia/grouped publication, pending first-cycle Dream pass or publication intent. Freeze current Memory bodies/lifecycle, Graph, duplicate authority, Community and vectors together.
- No already accepted historical checkpoint or completed live semantic history may be overwritten. An exact accepted import retry resolves first; an already-enrolled/regenerated owner restores its accepted state. Live F8 stays off during generation/reconstruction and can be enabled prospectively at cut N after checkpoint acceptance.
- No owner mutation may occur between manifest freeze and commit. Validate the complete owner input token again under the existing mutation boundary. Any drift, including same-turn body/graph/Dream changes, invalidates the attempt. Legitimate live events begin after checkpoint publication and retain normal scoring.

PHY, dependency RELs and cross-owner provenance/traversal are unsupported even if the caller can view them. A binding must resolve to this exact durable owner; labels and caller-supplied counters cannot prove ownership.

## Canonical transcript order

A source key is the exact pair (conversation ID UTF-8 bytes, node ID UTF-8 bytes) within the owner UUID. Do not normalize Unicode, case, whitespace or IDs. Repeated appearances on branches are one source if the complete immutable Node binding agrees; conflicting duplicates reject. Include all first-accepted genuine user/assistant Nodes in the frozen activity prefix. System/tool nodes, retrieval inserts, summaries and Echo sidecars provide no extra activity or semantic source.

For an existing REL, consume the accepted Archive activity prefix 1..N directly. Require unique contiguous positions, exact owner/source bindings, valid ancestry and activity count N. Physical Archive versions are input proofs, not decay units. Missing/cross-conversation parents, cycles and conflicting identities still reject.

Timestamps are provenance evidence. Missing/malformed timestamps required by provenance rules remain errors; valid negative and zero timestamps remain valid. Timestamp regressions, including ancestor/descendant regressions, are reported without reordering accepted positions or rejecting solely for timestamp order. No independently timestamp-sorted clock is compared against existing acceptance.

Fresh native imports continue to choose deterministic ancestry-respecting order through the importer. Once accepted, that order is authoritative. Reconstruction and fixture normalization cannot rewrite it.

## Availability, admission and cohorts

Resolve provenance from the frozen current Memory binding, structured `MemorySourceRef`, its Import Episode and explicit content/grounding references. The Episode supplies the conversation for a bare source-node ID; never search all conversations for a coincidental ID. Structured and legacy fields that both exist must agree. A partially specified conversation/node pair, foreign owner, missing referenced object or contradictory binding fails before scoring.

The necessary evidence set contains every declared source, content/authority and grounding turn plus all accepted turns on the generating Episode's validated start-to-end path. The stored model has no separate authority-turn field: resolve authority through content-source/source bindings, not an invented field. For an Episode, its historical completion is the last necessary accepted position on that path, not `finalized_at_ns` or the bulk processing time. Non-turn path nodes add ancestry but no tick; a path without accepted evidence is unsupported. Require `source_through_ns` to equal the endpoint Node timestamp, as the existing Episode builder records; it is corroborating path evidence, not a separate clock. A non-turn endpoint adds no tick; availability still uses the latest necessary accepted position.

For each supported Memory:

`availability_position = max(positions of all necessary evidence and Episode completion)`.

Its availability timestamp is the timestamp at that position. Equal timestamps do not imply equal availability: all necessary evidence must already have appeared in the ordered prefix. `source_time_ns`, `created_at_ns`, `updated_at_ns` and wall-time Dream execution are never substitutes for source identities. A direct already-active import publication needs explicit first-publication/source proof and no unexplained Episode binding; its position is the latest necessary proven source.

A cohort is all first-created Memories from one verified grouped Import Episode publication. Union outputs from repeated attempts against the same Episode; revisions of existing IDs are not new births. Its stable key is a domain-separated hash of owner UUID and EpisodeId; manifest membership is a sorted unique MemoryId list. Every supported member uses the maximum necessary availability over that cohort. If a member lacks usable required evidence, classify the entire cohort unsupported with null availability; no partial cohort admission is permitted. Conflicting origin/cohort assignments reject. A proven direct publication without an Episode uses a singleton cohort keyed by owner UUID and MemoryId.

Current nonarchived, nonsuperseded knowledge/canonical recipients admit at that availability, including isolated Memories with no Graph node. Start them at +100 and leave the ten-turn remainder/lazy tail to the existing scorer. Current archived, superseded, unresolved duplicate, extracted and never-admitted entries are classified as credit-excluded; the full-cohort checkpoint must represent that exclusion explicitly, without inventing retrospective admission. Historical lifecycle reconstruction is not promised.

**Missing-provenance policy:** no guessed fallback. A Memory with no usable required source proof is marked unsupported, receives no historical admission or reinforcement and retains its ordinary provisional projection unchanged. It remains traversable when its authorized Graph identity exists. List its ID/reason and untouched scoring identities in the manifest; exclude them from the checkpoint's superseded subset. A missing *declared* reference, conflicting provenance, incomplete cohort membership, or inability to prove scoring-isolation rejects the whole attempt. Step 21 must prove that preserving unsupported members cannot replay provisional effects into reconstructed members; if it cannot separate those effects, reject rather than silently double-credit. This makes partial coverage explicit and safe.

## Frozen graph, Community and vector proof

Pin the entire completed owner-local Memory adjacency projection of Graph, not a recipient-filtered induced graph. Use the existing Freshness Memory-to-Memory edge projection; other typed Graph nodes are neither Memory recipients nor invented Memory bridges, and their excluded kinds/counts are reported. Commit the full typed relationships used to derive this projection. Hash canonical Memory adjacency and the typed relationship/provenance records used for Dream roots; pin graph version and canonical duplicate representative evidence. Missing referenced local nodes, dangling canonical authority or foreign endpoints reject. Authorized future, archived and unsupported recipients can remain traversal intermediates; authorization is never relaxed.

Use one current Community snapshot only when its derived Graph version equals the pinned Graph version and membership is unique, owner-local and structurally valid. Hash sorted Community IDs/members and pin generation/algorithm parameters. If absent or stale, freeze an empty membership map and explicit `unknown-outside-v1` mode. Unknown origin or destination costs the existing outside-origin hop cost (10 in default v1); do not recompute Communities or invent membership. A present malformed snapshot rejects. Later membership cannot alter an accepted attempt.

Historical v1 requires the pinned base `FreshnessPolicy::default_v1()` descriptor/digest: ten turns per point, +25 accepted use, +50 linkage, -5 local/-10 outside hops and -100/+100 score bounds. A different base policy is unsupported for this reconstruction version; never change an enrolled owner's policy to make an import fit.

Historical semantic matching preserves the calibrated F8 constants under the N3 source-policy descriptor/digest: inclusive cosine 0.75, bonus 1..10, strongest comparison per fragment/Memory, exact cosine/MemoryId ties and at most 64 eligible direct roots per fragment. Filter historical availability/lifecycle/profile/self/cohort before that limit; future vectors cannot consume it. Propagated recipients are not capped to 64.

Pin genuine retained Archive fragments, including mixed-turn fragments, their represented source identities, selected generation/cut, profile descriptor/digest and immutable row bytes. Each fragment's availability is the latest accepted activity-bearing position it contains at that cut. Evaluate eligibility there once; never reuse its later context at every contained turn. Missing/invalid/incompatible evidence has explicit coverage. A separately requested bounded fill populates missing rows for existing genuine fragments only; no exact-turn fragment creation or per-turn embedding requests. Finish fill before manifest freeze.

## Event identity, order and eligibility

Historical events have a separate namespace from live semantic/Dream receipts. Stable identities exclude positions, bonuses and the manifest hash; changed evidence for the same identity conflicts rather than creating a second award.

| Event | Stable identity input | Position / roots |
| --- | --- | --- |
| Historical semantic | owner UUID, semantic policy domain, fragment identity | Fragment availability; at most 64 qualifying variable-bonus roots |
| Historical first-cycle Dream | owner UUID, import ID, originating MemoryId | Cohort availability; one +50 multi-root event |
| Birth/admission | MemoryId and cohort proof in checkpoint input | Availability; +100, no propagation |

At each position apply cohort birth/admission first, then independent semantic and Dream events in ascending stable event-ID bytes. Do not rely on worker completion order or introduce a kind-priority tie rule. Each recipient's sparse scorer sorts (position, event ID), applies existing settle/reinforce at each boundary with -100/+100 caps, and keeps tail decay lazy.

Semantic self exclusions cover all Memories created from any turn represented by the fragment, all corresponding generating Episodes/cohorts, and content/grounding provenance. Each fragment/target receives at most one strongest award. Overlapping fragment identities remain independent; re-embedding/profile rotation cannot create another award for the same fragment. Generation/profile/row digests are replay evidence, not event identity. Dream excludes its source and same-birth cohort from credit and requires strictly older eligible roots. A recipient otherwise qualifies when availability <= event position and all necessary evidence is present; equal timestamps alone do not suffice. The birth/admission phase makes other supported same-position recipients available.

Dream proof precedence is fixed: use actual completed first-cycle proof from this generated cohort; verified no-link means no event and cannot fall back to final adjacency. Otherwise use only completed direct Dream-origin links to strictly older active/canonical representatives. Record `actual-first-cycle`, `verified-no-link`, `completed-graph-approximation` or `unsupported` per source. Existing duplicate authority resolves representatives; pure rewires/non-Dream edges never manufacture an event. A verified duplicate-origin first-cycle proof may originate one event even when the source is a final archived alias; it still receives no own/cohort credit. Without usable proof, a credit-excluded source does not acquire invented fallback Dream history.

For either event, traverse the full authorized graph until remaining strength is nonpositive, with a fixed originating Community per root and existing -5/-10 costs. Take the maximum path/root contribution per recipient for the entire originating event. Apply temporal/self/cohort exclusions only at recipient credit, never neighbor expansion. Do not pass import exclusions through the ordinary live `PropagationEvent::excluded` field, which currently also removes bridges. Step 17 supplies a narrow import adapter; ordinary +25/+50 validation and live locality stay unchanged. Independent events may accumulate; multiple roots/fragments within one event never sum. No historical +25 consumed use is inferred.

## Manifest identity and canonical hashing

Use one logical versioned manifest, not a new durable subsystem. Temporary planning records may spill; only the eventual existing-Freshness checkpoint becomes authority. Manifest schema version remains 2; reconstruction policy version is 3.

Canonical encoding C is defined here independently of JSON rendering:

- Tags: 0 null; 1 boolean followed by u8 0/1; 2 signed i64 little-endian; 3 unsigned u64 little-endian; 4 bytes; 5 UTF-8 string; 6 array; 7 string-keyed map.
- Bytes/strings carry u32 little-endian byte length. Arrays carry u32 element count then C of each element. Maps carry u32 entry count and C(key), C(value) pairs sorted by raw UTF-8 key bytes. Reject duplicate keys, overflow, unsupported types, malformed UTF-8 and unknown schema fields.
- UUIDs/IDs/digests are raw 16/32-byte values. Nullable values are explicit null. Policy/profile descriptors are their existing canonical byte strings, not reparsed floating-point maps. No floating decimal serialization in C: hash existing validated profile descriptors and encode Community floating parameters as their exact finite IEEE-754 binary64 little-endian bytes.
- H(tag, value) = SHA-256(ASCII tag, one zero byte, C(value)). Tags are literal domain separators.

The corpus digest is H(`reliquary:historical-corpus:v1`, immutable required Node records sorted by (conversation ID UTF-8 bytes, node ID UTF-8 bytes)). Each Node record has exact fields `conversation_id, node_id, parent_id, role, principal_id, timestamp_ns, content_id`; include necessary non-turn ancestry, exclude transient branch enumeration and duplicate appearances. Resolve/verify ContentId bytes against retained content before freezing. Import ID is H(`reliquary:historical-import:v1`, map `owner_uuid, corpus_digest`). It is independent of processing time, worker count, graph generation and produced Memory IDs. The same owner/corpus cannot evade a previous checkpoint by regenerating outputs.

The manifest has exactly these top-level fields:

| Field | Required commitment |
| --- | --- |
| schema_version, policy_version, owner_uuid, import_id, corpus_digest | Version/owner/corpus identity above |
| archive_cut | Archive version, activity count N, exact final source key and canonical activity-order digest |
| source_records | Ordered immutable Node bindings and activity position; fragment proof is separate |
| memory_records | MemoryId-sorted BodyId/revision/lifecycle bindings, sorted necessary source keys, availability or null, cohort key, status/reason, current/provisional scoring proof digest |
| cohort_records | Cohort-key-sorted origin proof, sorted members, common availability or null, complete grouped-publication proof |
| graph_proof | Graph version, full adjacency/relationship digest, duplicate-authority digest |
| community_proof | Validated snapshot/membership digest and generation/parameters, or explicit unknown-outside mode |
| vector_proof | Profile and selected retained generations/cuts, FragmentId-ordered fragment_sources with represented Node bindings, availability, mode, immutable row evidence and coverage digest; embedding-pass disposition |
| scoring_policy | Complete base Freshness descriptor/digest, frozen F8 descriptor/digest, this reconstruction policy digest and event-order tag |
| replacement_proof | Exact provisional scoring identities/subset to supersede, unsupported untouched subset and isolation proof |
| input_token | Frozen Archive/Memory/Graph/Community/vector/Dream/Freshness owner versions and accepted-prefix digests |

Array orders and sorted sets are canonical, with no duplicated identities. Nested proof objects must include their underlying immutable IDs/content commitments rather than a bare mutable version number. Runtime resources (worker count, spill paths, timing) are outside the manifest. Hash the complete manifest with `reliquary:historical-manifest:v1`. The current typed descriptor below has `reliquary:historical-policy:v1` digest `f72f73b74a30a59de228c3a14a0e9344314c61724116efe3d822f22a25ca1fdb`. JSON is a human-readable rendering; hash C of the typed map with version as u64. The timeline consumes contiguous first-accepted Archive positions with complete identity/count/ancestry proof; source timestamps remain immutable provenance. Unknown digests reject; no maintained pre-release conversion framework is introduced.

```json
{
  "version": 3,
  "timeline": "accepted-archive-activity-v1",
  "clock": "exact-archive-prefix-v1",
  "availability": "latest-evidence-cohort-v1",
  "missing_provenance": "unsupported-preserve-or-reject-v1",
  "scope": "single-owner-rel-v1",
  "community": "validated-or-unknown-outside-v1",
  "semantic": "f8-v1-fragment-availability-top64-max-v1",
  "semantic_identity": "owner-historical-fragment-domain-v1",
  "dream": "actual-no-link-before-final-fallback-v1",
  "credit": "availability-inclusive-self-cohort-excluded-v1",
  "traversal": "authorized-full-graph-credit-filter-only-v1",
  "event_order": "admission-then-position-event-id-v1",
  "scoring": "existing-per-event-caps-lazy-tail-v1",
  "replacement": "provisional-isolated-subset-atomic-v1",
  "activation": "historical-before-prospective-live-v1",
  "encoding": "typed-le-c-v1",
  "identity": "owner-corpus-stable-import-v1"
}
```

Cohort tags are `reliquary:historical-episode-cohort:v1` and `reliquary:historical-singleton-cohort:v1`. Cohort input maps have exact fields `owner_uuid, episode_id` or `owner_uuid, memory_id`. Historical semantic identity is SHA-256 of ASCII `reliquary:historical-semantic-fragment:v1`, one zero byte, raw owner UUID and raw FragmentId. It excludes import, generation, profile and row bytes. Dream identity remains H(`reliquary:historical-dream:v1`, exact map `owner_uuid, import_id, originating_memory_id`), using C. Event IDs are the resulting raw 32 digest bytes; lowercase hex is display only and must preserve their byte order. No hash includes itself. Exact import ID + manifest + policies retries are no-ops after acceptance; a conflicting manifest for that import ID rejects `ImportManifestConflict`. Input drift before acceptance permits a discarded/recomputed attempt; after acceptance requires a separately designed migration, never automatic replay.

The checkpoint codec/transaction and transformed-history rebasing are Steps 21–23. They may choose physical framing, but cannot alter this identity/clock/scoring contract. In particular, unresolved foreign provenance or reordered accepted cuts cannot be hidden by copying numeric anchors.

## Acceptance examples and handoff

| Input | Required policy result |
| --- | --- |
| Independent conversations at timestamp T | Preserve their accepted Archive positions |
| Tied child whose key sorts before parent | Preserve accepted ancestry and positions; no timestamp re-sort |
| Child timestamp earlier than activity ancestor | Report regression; preserve accepted positions and timestamps |
| Shared ancestor on multiple branches | One source/activity tick; semantic events belong to fragment identities |
| Grounding at later position but same timestamp | Availability moves to that later position; earlier event cannot credit |
| Grouped Episode outputs with different necessary evidence | Shared latest cohort availability; no same-cohort own-event credit |
| Isolated active Memory with complete provenance | Admit +100 and decay normally without requiring a Graph node |
| Missing undeclared legacy provenance | Explicit unsupported/untouched projection; reject if scoring isolation cannot be proved |
| Foreign/dangling declared source or conflicting cohort | Reject before checkpoint |
| Future/unsupported Memory bridges two older recipients | Traverse bridge; suppress its credit only |
| Stale/absent Community | Explicit empty membership, every unknown hop costs 10 under default v1 |
| Verified initial Dream no-link with later final edges | No historical Dream event |
| Retry same corpus with changed generated bodies | Same import ID, conflicting manifest; no second accepted reconstruction |
| Actual accepted order differs from timestamp sort | Accepted Archive prefix remains the sole clock |
| Same-turn independent events near +100 | Admission first, event-ID order, cap at each event; no carried excess |

Step 14 implements pure timeline/provenance/manifest types and deterministic fixtures. Step 15 executes the Archive-clock gate. Steps 16–20 implement pinned event planning, import-only traversal and sparse scoring; Steps 21–25 supply atomic replacement/recovery/transforms/coordinator/release acceptance. This document freezes policy, not a checkpoint codec or shipped historical API.

## Documentation impact

- Inspected: core architecture/invariants/storage/maintainer/limitations/roadmap, ADR 0039, historical design, linear plan and current Archive/Memory/Episode/Community/Freshness seams.
- Updated: this contract, historical design cross-reference, linear completion marker, index, maintainer routing and roadmap handoff.
- Not affected: shipped API/storage bytes, Step 12 live activation/locality, PHY and ordinary Dream/use principals.
- Compliance check: both documentation-policy modes and git diff --check passed; the saved canonical descriptor reproduces the frozen policy digest. No Rust behavior changed.
- Known documentation gaps: future checkpoint framing, implementation and release evidence remain owned by Steps 14–25.

## Architecture impact

- Standards added or changed: frozen future historical v1 manifest/timeline policy; existing ownership unchanged.
- Ownership or boundary impact: source/topology owners supply proofs; only Freshness accepts a superseding score checkpoint.
- Pitlord enforcement impact: no source/ownership/static-policy change; Step 12's passing enforcement remains the source baseline.
- Other verification impact: policy tables define required later executable fixtures; no Rust runtime change or new inference.
- Known architectural gaps: historical planner/adapter/checkpoint are not implemented; transformed-history conversion remains unsupported until proven.

## Related docs

- [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md)
- [Frozen F8 calibration](freshness-semantic-calibration-2026-10-11.md)
- [Live F8 release evidence](freshness-semantic-release-verification-2026-10-12.md)

## Notes

The historical approximation does not estimate unseen graph evolution, relationship creation time or old lifecycle changes. Missing evidence is visible coverage or an explicit rejection. The mathematical result remains exact for the selected admissions/events under the existing owner REL clock.


## Previous runtime implementation routing

The C3–C8 notes below record the turn-source/timestamp-planner runtime before N2–N4. They are implementation history, not exceptions to the corrected intended contract.

### C3 native implementation routing

The Cva coordinator now builds every H1 top-level field from retained owner evidence, using checked typed canonical encoding. The supplied-digest freeze seam and simplified planner manifest hash are removed. Native grouped Episode publications and verified direct singleton publications use their distinct frozen cohort domains. Unsupported undeclared singleton provenance stays untouched; declared missing/foreign/conflicting evidence rejects. Complete publication payload digests, exact Node/body bindings, retained Dream roots/authority, all active typed relationships and full Memory adjacency, validated-or-unknown Community, profiles/generation/source coverage/target rows, isolated provisional event identities and accepted payload-family prefixes are pinned.

The canonical reconstruction-policy digest is reproduced from the typed descriptor and checked against the shared planner/checkpoint constant. C3 corrects the earlier incorrectly transcribed constant to the frozen H1 value; wrong-digest unreleased checkpoints reject. No compatibility path is introduced. Manifest construction publishes neither scores nor a checkpoint. Event production and native one-shot publication are integrated beneath the Cva owner boundary.


## C4 native event integration

The private native freeze now carries one ordered batch of canonical semantic and Dream events derived from its verified owner projection. Semantic identity is the frozen typed owner/import/exact-source map, represented as raw 32 bytes; positions, amounts, evidence digests and manifest hashes do not enter identity. Complete source fragments reduce by strongest match, with owner/provenance/cohort/availability/lifecycle/profile filtering before the 64-root limit. The producer's credit exclusions pass directly to the import propagation adapter, whose graph expansion remains complete.

Dream first-cycle roots and duplicate-origin proof are checked against the accepted source-pass graph transaction range. Verified no-link remains authoritative even for a final credit-excluded source. Existing representative authority resolves aliases; only strictly older eligible roots receive +50. A settled marker lacking direct retained proof is explicit unsupported and cannot manufacture a final-graph fallback. The pure producer retains its separately verified completed-direct-link approximation contract. Native mixed events are ordered by (position, raw event ID), without kind priority or recipient identity rehashing. Final credit-excluded records replace provisional admission with an unadmitted projection; unsupported records and disjoint provisional events remain untouched. Native score publication is integrated by C5 as described below.


## C7 embedding-only handoff

The [active completion plan, C7 frozen embedding-only contract](freshness-historical-completion-execution-plan-2026-10-12.md#c7-frozen-embedding-only-contract) defines bounded source-only fill, profile/generation selection, cancellation/retry, immutable vector reuse and the requested-complete typed embedding_pass proof. C8 implements it through the explicit endpoint/configured-runtime operation; new endpoint-free fill requests still reject. No-fill uses not-requested. H1 top-level schema and numerical policy digest stay unchanged. Final retained coverage, not transient attempt cost/progress, supplies manifest evidence.

## C5 native publication integration

The frozen manifest retains the exact authorized Memory adjacency, validated-or-unknown Community membership and recipient availability used by native preparation. The owner-derived canonical semantic/Dream batch propagates over that projection; one maximum contribution per recipient/source becomes a sparse score event with the original raw source identity and position. Scoring settles through the frozen cut. The sole accepted operation ends in private Cva commit, FreshnessStore and CVAFRS03 append/sync. Exact file/version drift rejects before mutation; accepted retry resolves the stored checkpoint before fresh-input checks. No durable event queue or second ledger is introduced. Bounded embedding and mounted/importer integration remain separate gates.
