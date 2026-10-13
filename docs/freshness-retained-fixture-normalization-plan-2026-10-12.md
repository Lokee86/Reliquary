# Retained Freshness fixture normalization plan — 2026-10-12

Parent: [Historical Freshness completion execution plan](freshness-historical-completion-execution-plan-2026-10-12.md).

## Purpose

Advance the three retained authoritative development corpora to the current Freshness/Historical reconstruction assumptions **once**, without creating or maintaining a general product migration framework for unreleased development formats.

This is fixture normalization, not a public compatibility promise.

The normalized fixtures are:

| Fixture | Authoritative REL | Current REL SHA-256 before this normalization wave |
| --- | --- | --- |
| ChatGPT first 14 days | `C:\!bin\workspace\reliquary-fixtures\local\authoritative\chatgpt-first14d\project.prj.rel` | `49751a62dd8644800d19976a0696a69366dc1e4f8d7c39bb4b69fbd7a8a5d18b` |
| ChatGPT first 28 days | `C:\!bin\workspace\reliquary-fixtures\local\authoritative\chatgpt-first28d\project.prj.rel` | `8f4a2bf871d7f5368edde93954a2260a29c86131b28475b323a7b6c70f8c51e4` |
| Ellis pre-Relationship | `C:\!bin\workspace\reliquary-fixtures\local\authoritative\rhelm-david-r-ellis-pre-relationship\project.prj.rel` | `e53255ff5258aaeb5fc4437e5993613868adf646d9e5f711fe2b079f7a340fd4` |

The Ellis baseline above is the post-2026-10-12 state after completing the previously pending 15 Import Episodes in place. Its paired PHY is `110301f1f12873b8ac1a2eb5000a004fdcd0287d7e141622e6cd18e7b2afe937`.

## Overview

Do **not** add a general current-version-to-current-version migration subsystem before the first public release.

Instead:

1. correct the semantic source model so Freshness uses the Archive fragment vectors Reliquary actually stores;
2. correct H1 to treat the already-accepted Archive activity order as authoritative for existing RELs;
3. write a temporary, fixture-specific normalizer that accepts only these pinned retained owners/hashes;
4. materialize only current standard Reliquary records/state required by Historical Freshness;
5. validate and atomically promote the normalized bytes back to the same authoritative fixture paths;
6. rerun C10 on those normalized fixtures; and
7. delete the normalization implementation after the fixtures and verification records are final.

No public format version or compatibility chain is created by this work.

## Release-policy decision

Before the first public release:

- intermediate development formats are disposable;
- no promise exists that arbitrary unreleased REL/PHY files can migrate forward;
- production `migrate_file(source, output)` remains limited to its existing legacy owner/header migration purpose;
- the retained 14-day, 28-day and Ellis corpora are exceptions because they are valuable long-lived calibration/acceptance assets;
- conversion logic for these exact assets is temporary and must not become a generic migration API.

At the first public release, freeze the first externally supported storage baseline. Only migrations **from released formats** become maintained compatibility machinery.

## Hard boundaries

1. **No expansion of public migration APIs.** Do not change `migrate_file` to accept current owner-ID files.
2. **No generalized version registry or N→N+1 chain.** No migration graph, compatibility matrix, per-version dispatcher or durable migration ledger.
3. **No retained alternate fixtures.** The authoritative paths remain the same. A temporary sibling file is allowed only for validated repack/atomic replacement and must be removed after promotion.
4. **No new fixture-only durable format.** The output may contain only current standard REL/PHY records understood by ordinary current code.
5. **No fabricated consumed-use history.** +25 use history remains unavailable unless real accepted-use evidence exists.
6. **No fake “actual first-cycle Dream” claims.** Legacy Dream approximation must stay explicitly distinguishable from real retained first-cycle proof during reconstruction/verification.
7. **No per-turn embedding population.** Freshness uses the Archive fragment/vector model already owned by Reliquary.
8. **Accepted Archive order is authority.** Existing accepted positions are never reordered during normalization merely because source timestamps sort differently.
9. **Identity preservation is mandatory.** Owner UUIDs, Archive Node identities, accepted activity positions, Memory IDs/body identities, provenance, branch identities and retained semantic graph authority must remain stable unless a separately documented current owner operation legitimately adds new state.
10. **Normalization is destructive only after verification.** Never overwrite an authoritative fixture until the candidate output reopens, passes invariants and has recorded before/after hashes.

## Semantic source correction

The current F8/H1 implementation incorrectly excludes mixed-turn Archive fragments and treats an exact single-turn embedding as the required semantic source. That is not Reliquary's normal Archive embedding model.

The corrected model is:

- an **Archive fragment** is the source semantic evidence unit;
- the fragment uses its existing retained/current Archive vector row;
- mixed-turn fragments are valid, expected evidence;
- no single-turn fragment is created merely for Freshness;
- no turn text is separately embedded merely for Freshness;
- a source fragment becomes semantically usable at its deterministic **availability position**: the latest accepted activity-bearing turn contained in that fragment at the selected Archive/vector cut;
- target Memory eligibility is evaluated at that availability position;
- one fragment produces at most one reinforcement effect per target Memory;
- the strongest qualifying comparison for that fragment/target wins;
- source/self exclusion covers Memories whose provenance belongs to **any** source turn represented by that fragment, not only the fragment's final turn;
- overlapping Archive fragments are separate retained semantic observations, but each fragment identity may be credited only once per target under a given accepted policy event;
- vector regeneration/profile changes do not create a new semantic activity event for an already-credited fragment. The durable event identity is based on owner + semantic policy domain + fragment identity; generation/profile/row digests are replay evidence, not a reason to award the same fragment again.

Live F8 remains **local/non-propagating**. Historical semantic reconstruction may still use the accepted import-only propagation rule after the fragment-local root effect is established.

### Historical event position

For an existing imported REL, a fragment's historical semantic event occurs at the fragment's availability position in the **accepted Archive activity clock**. It does not occur retroactively at every turn contained in the fragment.

This prevents:

- repeated use of one mixed vector for every contained turn;
- leakage of later fragment context backward into earlier accepted positions; and
- an artificial requirement to embed every Archive turn separately.

### Vector completion after this correction

The bounded C7/C8 historical embedding operation must be corrected accordingly:

- fill missing rows for existing genuine Archive fragments;
- never create exact-turn fragment records;
- never request one embedding per turn;
- if no usable generation exists, use the existing ordinary current vector/profile machinery to establish a compatible current fragment generation before reconstruction;
- preserve already-valid retained rows when the selected compatible generation/profile permits it;
- report missing/invalid/incompatible fragment coverage explicitly.

The 14-day and 28-day corpora therefore need **current fragment vectors**, not per-turn embeddings. Ellis already has 498 retained mixed-turn Archive rows, but its current endpoint/profile compatibility may justify building a new current generation rather than trying to extend the old incompatible profile.

## H1 clock correction

For already-ingested REL state, H1 must not reconstruct a second authoritative order from timestamps and demand byte-for-byte positional equality.

The corrected authority is:

1. Archive first-accepted activity position is the numerical Freshness clock.
2. Branch ancestry and accepted Archive identity are already embodied in that durable order.
3. Source timestamps remain provenance/temporal evidence.
4. Timestamp regressions are retained facts and may be reported, but do not reorder accepted history.
5. A source timestamp can still make provenance unusable when it is absent/malformed in a way required by another rule, but “timestamp sorts differently than accepted order” is not itself a clock incompatibility.
6. Fresh native imports should continue to produce deterministic accepted order through the importer; once accepted, that order becomes authoritative.

This removes the artificial 14-day mismatch and the 28-day ancestor/descendant timestamp-regression blocker without rewriting either Archive.

## Legacy pre-Freshness evidence normalization

The retained corpora contain real pre-Freshness Insomnia/Memory/Dream-era evidence but lack later Freshness-specific publication fields. The one-off normalizer may derive current standard evidence **only where the old owner state proves it uniquely**.

### Publication/birth evidence

For each existing Memory eligible for historical reconstruction:

- bind the Memory to its retained source Episode/provenance and immutable first Memory revision;
- use retained Insomnia completion/publication evidence to prove that the Memory was actually published;
- derive the corresponding current birth/cohort input from that proof;
- do not infer a first-publication position from current file order when the source Episode/provenance does not prove it;
- unresolved publication authority remains an explicit normalization failure.

The normalizer must produce the same current standard state a present-day import/generation path would leave for H1. It must not introduce a permanent “legacy migration” record type.

### Legacy Dream evidence

Where a real current first-cycle Dream pass/proof exists, preserve and use it.

Where it does not exist, use the already accepted historical **legacy approximation**, not a fake actual pass:

- final authorized Memory graph is the propagation substrate;
- one source Memory can contribute at most one +50 historical Dream event;
- roots come only from justified retained Dream-origin/direct linkage authority to older canonical targets;
- duplicate authority uses the current canonical representative;
- source/cohort/self roots are excluded;
- no-link remains no event;
- pure rewiring without supported Dream-origin evidence creates no event;
- the verification/report must label the result as legacy approximation rather than actual first-cycle evidence.

If the current production historical event adapter cannot represent this distinction without new durable compatibility state, the **temporary normalizer** computes the approximation while producing ordinary current reconstruction input/checkpoint state. Do not add a permanent general legacy-Dream migration subsystem solely for these fixtures.

## Fixture-specific state

### 14-day ChatGPT

Known normalization work:

- preserve the existing 5,857-position accepted Archive clock exactly;
- stop treating timestamp-sorted order as an alternate required clock;
- complete any H1-required import/Episode generation coverage that is genuinely absent rather than fabricating completion;
- establish a current compatible Archive fragment vector generation and target Memory vectors using existing fragments;
- normalize provable pre-Freshness publication/birth evidence;
- provide actual Dream proof where retained, otherwise the explicit legacy approximation;
- run historical reconstruction and C10 measurement against the normalized authoritative state.

### 28-day ChatGPT

Known normalization work:

- preserve the existing 16,465-position accepted Archive clock exactly;
- retain the 11 known ancestor/descendant timestamp regressions as provenance anomalies; do not reorder the Archive;
- complete any H1-required import/Episode generation coverage that is genuinely absent;
- establish a current compatible Archive fragment vector generation and target Memory vectors using existing fragments;
- normalize provable pre-Freshness publication/birth evidence;
- provide actual Dream proof where retained, otherwise the explicit legacy approximation;
- run historical reconstruction and C10 measurement against the normalized authoritative state.

### Ellis

The 15 previously pending Import Episodes are already complete in the authoritative pair: 143/143 Episodes, zero terminal Episodes, +47 REL Memories and +47 PHY Memories.

Remaining normalization work:

- preserve the 2,972-position accepted Archive clock;
- establish a compatible current fragment/Memory vector space for the expanded 725-Memory REL rather than extending the incompatible retained profile blindly;
- complete the required current Dream processing/evidence for newly published Memories where that can be run genuinely;
- normalize pre-Freshness evidence for older Memories using the same rules as the ChatGPT fixtures;
- refresh downstream verification metadata affected by the 94 newly published REL+PHY Memories only where required by H1/C10;
- run historical reconstruction and C10 measurement on the same authoritative REL/PHY paths.

Entity-resolution recalibration is **not** automatically part of this Freshness normalization. Only rerun it if a current H1/Dream invariant actually depends on newly unresolved Entity authority.

## Temporary normalizer design

Implement one disposable developer-only normalizer. Preferred placement while it exists: a clearly named repo-local `examples/` or `scripts/` entry plus the smallest private library hooks needed to compose existing current owners.

It must:

- contain an explicit allowlist of the three fixture owner/path identities and expected pre-normalization SHA-256 hashes;
- refuse arbitrary input files;
- refuse an already changed hash unless the recorded normalization ledger says that exact fixture has already advanced;
- inspect first and print a deterministic proposed normalization report before mutation;
- write a temporary sibling candidate where physical repack is required;
- use current owner APIs/replay primitives rather than raw byte patching;
- reopen and verify the candidate through ordinary `Cva`/`Phylactery`;
- compare identity/activity/provenance/Memory/graph invariants;
- run the fixture's focused Historical Freshness preflight;
- atomically replace the authoritative file only after all candidate gates pass;
- update `AUTHORITATIVE.txt` and `SHA256SUMS.txt` with the new state/hash;
- restore fixture read-only attributes where the fixture uses them;
- delete all temporary sibling files;
- never leave a reusable generic migration command behind.

After all three fixtures pass C10, delete the normalizer source and any private hook used only by it. Keep the dated normalization verification document as the reproducibility record.

## Sequential execution

| # | Model | Work | Completion gate |
| ---: | :---: | --- | --- |
| **N1 — complete** | **Sol** | Freeze the mixed-fragment semantic-source correction and accepted-Archive-clock correction above. Update H1/F8 design tests before touching retained fixtures. | No exact-turn-only semantic assumption remains in the intended contract; accepted Archive positions are the existing-REL numerical clock. |
| **N2 — complete** | **Sol** | Refactor live F8 source identity from accepted-turn-only evidence to fragment-availability evidence. Update durable semantic event identity/evidence and self/cohort exclusions without enabling live propagation. | Mixed-fragment live fixtures pass; one fragment cannot be double-credited because it overlaps multiple turns or is re-embedded. |
| **N3 — complete** | **Luna** | Refactor historical manifest/event planning and C7/C8 embedding fill to the same fragment-source model. Remove exact-turn fragment creation/fill requirements. | H1 manifest proves fragment/generation/profile/availability evidence; historical semantic fixtures use mixed fragments deterministically. |
| **N4 — complete** | **Sol** | Change existing-REL H1 chronology validation to consume canonical Archive activity order. Keep timestamps as provenance evidence and preserve fresh-import deterministic order. | 14-day order mismatch and 28-day timestamp regressions no longer fail solely because timestamp sort differs from accepted Archive order. |
| **N5 — complete** | **Sol** | Implement the disposable fixture normalizer with exact owner/hash allowlisting, current-standard output only, candidate verification and atomic same-path promotion. No public migration API changes. | Tool refuses non-allowlisted files and cannot leave a partially promoted authoritative fixture. |
| **N6 — complete** | **Luna** | Normalize 14-day and 28-day in place: complete genuinely missing H1-required generation work, establish current fragment/target vector state, normalize provable pre-Freshness birth evidence, and materialize the accepted Dream legacy approximation where actual proof is absent. | Both authoritative fixtures reopen under ordinary current code, preserve owner/Archive/Memory identity invariants, have updated hashes, and pass historical preflight. |
| **N7 — complete** | **Luna** | Normalize Ellis in place from the new 143/143 baseline: complete compatible current vector state, required Dream work for newly published Memories, and older pre-Freshness normalization. | Same authoritative REL/PHY paths reopen and pass H1 generation/vector/Dream preflight; hashes/marker are updated. |
| **N8 — complete** | **Luna** | Rerun C10 retained-corpus acceptance/measurements on all normalized fixtures. Capture CPU/memory, semantic fragment coverage/matches, graph traversal, checkpoint cost/reopen and final Fresh/Stale/Dormant distribution. | At least one—and preferably all three—authoritative retained corpora complete the production historical path with nonempty justified events and reproducible hashes. |
| **N9 — complete** | **Sol** | Review normalization output for any rule that accidentally became required for normal post-release operation. Promote only genuine product behavior (mixed fragments, accepted clock); keep fixture conversion itself non-product. | No unreleased-version compatibility subsystem has leaked into the maintained runtime. |
| **N10 — complete** | **Luna** | Delete the fixture normalizer and any hook used only by it. Update current docs/API to the implemented fragment-source semantics, retain before/after hash/evidence report, then resume C11. | Final tree contains no temporary fixture migration implementation; C10 evidence remains reproducible from the retained verification record. |

## Verification requirements

For every normalized authoritative owner:

- record pre/post SHA-256 and byte length;
- record owner UUID and REL/PHY type;
- verify every existing accepted Archive identity retains the same activity position;
- verify branch identities and canonical flags;
- verify all pre-existing Memory IDs and body IDs remain present and semantically unchanged unless ordinary current processing legitimately adds a revision;
- verify provenance/source Episode/source Node bindings;
- verify Graph authority and duplicate representative behavior;
- verify current compatibility profile/vector generation evidence;
- verify Insomnia completion counts and no terminal work required by H1;
- verify Dream actual/approximation mode counts separately;
- verify no fabricated accepted-use receipts;
- reopen through ordinary current code;
- run the native historical reconstruction preflight;
- after accepted reconstruction, verify `CVAFRS03`, reopen, exact retry no-op and subsequent legitimate live Freshness behavior.

The normalization report must distinguish **preserved**, **derived from authoritative legacy evidence**, **newly executed current work**, and **approximation**. Do not collapse them into a generic “migrated successfully” statement.

## Deletion gate

The temporary normalizer may be deleted only after:

1. the authoritative fixture files and marker/hash files have been updated;
2. C10 measurements are captured in a committed verification record;
3. the normalization before/after hashes and derivation rules are committed to docs;
4. current tests cover the product-level rules that survive normalization: mixed-fragment F8/H1 semantics and accepted-Archive chronology;
5. no test depends on rerunning the deleted normalizer.

The retained normalized fixture bytes become the new development baseline. Older unreleased fixture formats receive no continuing support.

## First-release migration boundary

When Reliquary reaches its first public release:

- name/freeze the release storage baseline;
- add an explicit compatibility policy for that released baseline;
- from then on, every intentional incompatible storage change must either preserve decode compatibility or provide a maintained migration from the last released format;
- do not retroactively support these pre-release fixture formats unless a real user-facing need appears.

## Related docs

- [Historical Freshness completion execution plan](freshness-historical-completion-execution-plan-2026-10-12.md)
- [Historical import reconstruction specification](freshness-historical-import-reconstruction-spec-2026-10-03.md)
- [Freshness semantic + historical linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)
- [Rust API reference](api.md)
- [Current limitations](current-limitations.md)
- [Development and retained fixture paths](development.md)

## Notes

This plan intentionally prefers deleting pre-release conversion code over maintaining compatibility with unreleased formats. The lasting product changes are the semantic-source correction and accepted-Archive-clock correction; the fixture-forward conversion is a one-time development operation.

## N1 disposition — 2026-10-12

The intended live F8 contract, H1 manifest/timeline policy and historical design now use mixed-fragment availability and accepted Archive chronology. Previous runtime descriptor/implementation notes remain explicitly labelled; this design change does not silently change existing policy digests or storage bytes. N2 must freeze corrected live encoding and N3/N4 must freeze corrected historical descriptor/digest with their runtime tests.

`scripts/test_freshness_normalization_contract.py` contains executable design oracles for mixed-fragment availability, no backward leakage, strongest comparison, all-source self/cohort exclusion, overlap identity, re-embedding identity, owner separation, accepted timestamp regressions and invalid clock evidence. These are design tests, not proof that the previous runtime already meets N2–N4. No retained fixture was opened for mutation.

N1 verification: 10 design oracles passed via `python -m unittest discover -s scripts -p test_freshness_normalization_contract.py`; both structural and `--changed-from origin/main` documentation-policy checks and `git diff --check` passed. Rust runtime/source ownership is unchanged, so root/CLI runtime acceptance belongs to N2–N4 rather than this design gate.

Documentation impact:
- Inspected: repository guidance, shared documentation procedure, current owner documents, live F8 contract, H1 policy/design and completion/normalization plans.
- Updated: F8/H1 intended contracts, C7/C8 handoff, behavioral-contract routing and N1 disposition.
- Not affected: current public API/storage behavior, authoritative fixture bytes and historical verification records.
- Compliance check: both required documentation-policy modes and patch hygiene passed.
- Known documentation gaps: runtime encoding/digest and acceptance updates remain N2–N4.

Architecture impact:
- Standards added or changed: intended source unit and clock contract corrected; no shared standard changed.
- Ownership or boundary impact: Archive retains source/activity authority; Freshness retains score/receipt authority.
- Pitlord enforcement impact: no Rust ownership or static-policy change.
- Other verification impact: 10 design oracles; native regression gates remain N2–N4.
- Known architectural gaps: the existing runtime has not yet converged to the corrected contract.

## N2 disposition — 2026-10-12

Live F8 now discovers retained Archive fragments and accepts each at its latest represented activity position. Mixed-turn rows are used directly; no turn-only fragment materialization remains in live generation/worker paths. Current API reporting is fragment-based (`semantic_freshness_pending_fragments`, `completed_fragments`, `pending_fragments`). Source proof includes all represented Nodes; provenance and generating Episode cohorts are excluded before local top-64 matching.

The existing CVAFRS02 event codec uses producer kind 4 for fragment receipts, preserving prior kind-3 decoding. The fixed domain is `reliquary:freshness:semantic-fragment:v1`; identity hashes domain + zero byte + owner UUID + FragmentId, without generation/profile/row evidence. Typed evidence pins canonical represented identities plus row/content proof. A matching accepted fragment is resolved before target search, so regenerated rows/profiles and later targets cannot reopen it. Replay and divergent reconciliation recompute availability from all mapped activity sources; copy/repack/reclamation preserve accepted evidence. No public migration API or conversion framework is added.

Historical turn-source preparation remains unchanged pending N3, and existing-REL chronology remains pending N4. Authoritative fixtures are untouched. Ordinary Archive producers must materialize their configured fragments; F8 no longer invents source observations from uncovered turns.

Previous turn-semantic receipts remain readable as their frozen source model. New fragment receipts cannot be mixed with that previous live history: acceptance rejects before append, preventing an old observation from receiving a second award after the source-model correction. No conversion or maintained pre-release migration path is provided.

N2 verification:
- Root formatting and `cargo check --locked -j 2` passed. Full `cargo test --locked -j 2 -- --test-threads=4` passed: 1,258 library tests, 4 ignored measurements/retained diagnostics, 138 integration tests and doctests (0 tests). Both exhaustive native recovery-prefix tests passed.
- CLI formatting/check/locked tests passed: 19 tests. The Archive roundtrip smoke passed with two Nodes, one branch and one mixed fragment. The 10 N1 design oracles passed.
- Architecture enforcement passed: 75 P4 guard fixtures, 161 classified APIs, 11 external-ownership fixtures and 23 Pitlord rules on the captured 70,744-node/2,379-relationship snapshot (`73e6b27592a747b23bb4e81b3e1613393235f2e9e12a43f7c4624491a471fdd7`). This is scoped enforcement evidence, not proof of arbitrary dynamic Rust calls.
- The first full run exposed an obsolete turn-pending assertion and a Windows mock HTTP socket inheriting nonblocking mode. The continuation fixture now materializes ordinary configured fragments, and the mock switches accepted sockets to blocking mode before applying its timeout. The final full rerun passed both cases.
- Windows verification used repository-local `.t` scratch and process-local `CARGO_PROFILE_DEV_DEBUG=0` / `CARGO_PROFILE_TEST_DEBUG=0`. An initial build exhausted disk space; removing only generated worktree-local PDB files recovered about 25 GB. Build profiles and authoritative fixtures were not edited.

Documentation impact:
- Inspected: repository/shared guidance, architecture, invariants, storage, API, maintainer/coverage/behavior maps, limitations and Freshness contracts/plans.
- Updated: current live fragment semantics, kind-4 receipt framing, API reporting units, test routing and N2 disposition.
- Not affected: authoritative retained corpora, historical timeline/source runtime and public migration APIs.
- Compliance check: structural/change-impact documentation checks and patch hygiene passed on the final recorded tree.
- Known documentation gaps: historical convergence and retained-corpus evidence remain N3–N8; current live behavior is documented.

Architecture impact:
- Standards added or changed: no shared standard; fragment identity/availability now implements the frozen N1 contract.
- Ownership or boundary impact: Archive supplies immutable source/activity proof; Freshness remains the sole receipt/score owner. Live effects stay local.
- Pitlord enforcement impact: refreshed repository policy and current P4/external ownership guards pass; no new area/rule is required.
- Other verification impact: mixed-source/exclusion/rotation/reopen, missing-row retry, strict birth, incompatible source-model history and native rebase/reclamation fixtures pass.
- Known architectural gaps: historical fragment/clock convergence remains N3/N4; no maintained pre-release conversion subsystem was introduced.

## N3 disposition — 2026-10-12

Historical event and manifest planning now use genuine retained Archive fragments, including mixed-turn fragments, at their latest represented activity position. All represented Nodes are pinned as immutable proof. Semantic event identity is SHA-256 of the fixed historical fragment domain, zero byte, owner UUID and FragmentId; import/generation/profile/row proof does not reopen credit. H1 schema/policy 2 and requested-fill contract 2 commit fragment bindings, availability, generation/profile and immutable row bytes. The policy digest is `e4a40d7711ba768119336577b6276de1aff396d2d9d289606e9d3674615e088b`. CVAFRS03 framing is unchanged; previous unreleased policy digests reject without conversion support.

C7/C8 fill populates only missing rows of existing fragments using bounded ordinary Archive text rendering. No exact-turn Fragment records are created. Existing rows, including invalid evidence, remain unchanged. Coverage is per fragment (`unavailable_fragments`); the obsolete creation budget is removed and report `new_fragments` stays zero. Ordinary materialization and compatible profile/generation setup remain prerequisites. Cancellation, partial-batch retry, union amplification and preflight limits retain their existing behavior.

Source-cohort exclusions now cover every necessary represented source, including external grounding evidence, for both live and historical selection. Authorized historical graph bridges remain traversable; excluded cohorts receive no credit. The old turn-only preparation adapter is removed. Native fixtures now materialize ordinary mixed fragments. Authoritative retained corpora were untouched; accepted-clock convergence remains N4 and disposable corpus normalization remains N5–N10.

Documentation impact:
- Inspected: repository/shared guidance, current architecture/invariant/storage/API owners, routing maps and Freshness policy/plans.
- Updated: current historical fragment/fill contract, typed policy descriptor/digest, proof schema, public coverage units, regression routing and this disposition.
- Not affected: authoritative fixture bytes, Dream proof/receipt authority and physical Archive/vector/checkpoint framing.
- Compliance check: final structural/change-impact policy and patch hygiene results are recorded below.
- Known documentation gaps: N4 clock convergence and N5–N10 retained calibration remain; current fragment behavior is documented.

Architecture impact:
- Standards added or changed: no shared standard; N1 fragment contract implemented in historical adapters.
- Ownership or boundary impact: Archive retains immutable source/vector/activity authority; Freshness retains receipt/score/checkpoint authority. No store, coordinator, migration registry or queue added.
- Pitlord enforcement impact: current refreshed policy evidence is recorded below; no new boundary/rule required.
- Other verification impact: mixed-fragment identity/availability/proof drift, bounded rendering, cohort exclusions and native fill/retry/recovery fixtures.
- Known architectural gaps: accepted chronology remains N4; retained-corpus acceptance remains later gates.

N3 verification:
- Root formatting and locked build passed. Full locked tests passed: 1,260 library tests, 4 ignored measurements/retained diagnostics, 138 integration tests and doctests (0 tests). Both exhaustive native checkpoint/live-use byte-prefix recovery tests passed.
- CLI formatting, locked build and all 19 tests passed. Archive roundtrip passed with two Nodes, one branch and one mixed fragment. All 10 N1 design oracles passed.
- Native regressions pin mixed availability, canonical row/provenance evidence and external-grounding whole-cohort exclusion. A generation refresh changes manifest evidence/bonuses while retaining all fragment event IDs and positions. Invalid retained rows remain untouched; bounded mixed rendering equals ordinary rendering and rejects a one-byte-short limit.
- Verification used repository-local `.t` scratch and process-local debug-disabled build profiles; no committed build profile or retained corpus was changed.
- Architecture refresh/enforcement passed: 75 P4 guard fixtures, 161 classified APIs, 11 external-ownership fixtures and 23 Pitlord rules on 70,802 source nodes / 2,379 relationships; snapshot `f4b35ead77f7860765d9e2d43a8001af12023fbe312b2105ff2ee7f7f3c2259b`. This is scoped static enforcement evidence, not proof of arbitrary dynamic calls.
- Both required documentation-policy modes and patch hygiene passed on the final recorded tree.

## N4 disposition — 2026-10-12

Existing-REL H1 chronology now consumes canonical Archive first-accepted activity positions. Timestamp ties, cross-conversation timestamp order and ancestor/descendant timestamp regressions remain provenance, not another clock or rejection trigger. The read-only projection validates complete identity/count/position evidence, ancestry existence/acyclicity and activity ancestry order. Missing, duplicated, gapped, foreign or reversed activity-position evidence rejects explicitly. Non-activity ancestry remains present without a Freshness tick. The pure unaccepted-source timestamp planner and fresh native importer are unchanged.

Manifest schema remains 2; reconstruction policy version is 3. The `accepted-archive-activity-v1` timeline descriptor freezes policy digest `f72f73b74a30a59de228c3a14a0e9344314c61724116efe3d822f22a25ca1fdb`. Physical CVAFRS03 framing is unchanged. Previous unreleased digests reject without a maintained compatibility or migration framework. Semantic fragment identities and Dream/birth/use proof authority are preserved. Authoritative retained corpus bytes are untouched; normalization/calibration remains N5–N10.

Native fixtures now accept timestamp-regressing chronology, preserve all numerical positions through reconstruction/reopen and resolve exact retry without writes. Pure regressions reject missing/conflicting source evidence, parents/cycles and invalid position maps. The former native timestamp-mismatch guard now verifies that an actually unprocessed Archive tail still rejects, independent of timestamp order.

Documentation impact:
- Inspected: repository/shared guidance and current architecture, invariants, storage, API, maps, limitations, roadmap and H1 plans.
- Updated: accepted-clock API/contract, policy descriptor/digest, regression routing and this disposition.
- Not affected: immutable retained corpus bytes, historical verification records, physical checkpoint/vector framing and fresh native importer ordering.
- Compliance check: final structural/change-impact results are recorded below.
- Known documentation gaps: retained normalization/calibration evidence remains N5–N10.

Architecture impact:
- Standards added or changed: no shared standard; the frozen N1 accepted-clock contract is implemented.
- Ownership or boundary impact: Archive supplies the authoritative positions; Freshness consumes them without rewriting Archive or adding a clock/store/queue.
- Pitlord enforcement impact: fresh static enforcement results are recorded below; no new rule or area required.
- Other verification impact: native chronology/reconstruction/reopen/retry, invalid-map regressions and existing full recovery/transform gates.
- Known architectural gaps: retained fixture birth/Dream/generation normalization and C10 calibration remain later gates.

N4 verification:
- Native accepted-clock fixtures preserve cross-conversation acceptance despite timestamp ordering/ties, retain ancestor timestamp regressions and non-activity context, and verify byte/position/reopen stability. A native grouped import reconstructs with descending source timestamps and resolves exact retry after reopen without writes.
- Invalid evidence fixtures reject missing/conflicting source identities, missing parents/cycles, incomplete counts and missing/duplicate/gapped/foreign/reversed position maps. The unprocessed Archive tail and prior accepted-use rejection matrix passed.
- CLI formatting, locked build and all 19 tests passed on the final source tree. Archive roundtrip passed with two Nodes, one branch and one mixed fragment whose timestamps regress. All 10 N1 design oracles passed.
- An obsolete timestamp-mismatch assertion was corrected to test genuine unprocessed work. A superseded verification run held its Windows executable during rebuild; it was stopped and final verification restarted after both planning and checkpoint-prefix validation were updated. Superseded runs are not acceptance evidence.
- Verification uses repository-local `.t` scratch and process-local debug-disabled build profiles. No authoritative corpus or committed build profile was edited.
- Final root formatting and locked build passed. Full locked tests passed: 1,263 library tests, 4 ignored measurements/retained diagnostics, 138 integration tests and doctests (0 tests). Both exhaustive native checkpoint/live-use byte-prefix recovery tests passed.
- Final architecture refresh/enforcement passed: 75 P4 guard fixtures, 161 classified APIs, 11 external-ownership fixtures and 23 Pitlord rules on 70,871 source nodes / 2,379 relationships; snapshot `f87a2e6bf399e7b694cffe4391856a264252101d1181fc568c13616327a2e6e8`. This is scoped static enforcement evidence, not proof of arbitrary dynamic calls.
- Both required documentation-policy modes and patch hygiene passed on the final recorded tree.

## N5 disposition — 2026-10-13

The disposable `scripts/freshness_fixture_normalizer.py` accepts only the three named authoritative directories, the pinned REL/PHY hashes above and exact typed owner UUIDs. It stages fixed candidates, derives deterministic proposed publication evidence from read-only ordinary-owner inspection, verifies candidates through native Cva/Phylactery and H1 freeze/prepare, and promotes at the same paths only after all gates. No arbitrary source path/hash, public migration API, maintained compatibility chain or fixture-only durable owner record is added. N6/N7 perform the actual candidate owner work; N5 changes no authoritative bytes.

Native proposal derivation binds retained Import Episode/completion, immutable first Memory revision/mutation/body and accepted Episode activity. A conflicting/unproven publication cannot authorize a derived birth. Verification preserves original activity identities/positions, branches, every prior Memory revision/body/provenance, semantic graph authority, Episodes/fragments, actual Dream proof and accepted-use proof. It requires complete selected fragment/current-target vector coverage and native H1 readiness. Derived-birth audit entries name their exact retained proof digest. Legacy publications cannot gain newly fabricated actual-first-cycle records; previously current births and genuinely new publications need genuine completed Dream. Legacy Dream/no-event coverage remains explicitly labelled approximation in the N6/N7 audit, which is not itself scoring authority.

Owner allowlist (REL / PHY):
- 14-day and 28-day: `73eff05f-e65e-4fb4-b430-f9af2237b48b` / `ce8c8fc9-640d-4e76-abd8-dab60424763b`. Path and hash fences distinguish these same-owner snapshots.
- Ellis: `41fd7fac-75d5-4d05-899c-0dce0823aa36` / `4ad56e4a-ecc4-468f-8742-eaa343efd9e7`.

Read-only N5 inspection (ordinary owners on repo-local copies; original pair hashes unchanged):

| Fixture | Archive positions | REL / PHY Memories | Unique proved publication proposals / conflicts | Fragment evidence | Current target bindings | H1 before normalization |
| --- | ---: | ---: | ---: | --- | ---: | --- |
| 14-day | 5,857 | 1,087 / 107 | 1,087 / 0 | 1,056 existing eligible fragments; no generation | 0 | Unprocessed Archive tail/branch |
| 28-day | 16,465 | 2,816 / 244 | 2,816 / 0 | 2,882 existing eligible fragments; no generation | 0 | Unprocessed Archive tail/branch |
| Ellis | 2,972 | 725 / 511 | 725 / 0 | 498/498 usable mixed fragments | 678 | Unverified direct singleton publication |

Actual retained first-cycle proof count is zero for each fixture. Current native publication-birth counts are 0 / 0 / 47 respectively. This proves bounded derivation candidates, not normalized birth/Dream state or C10 acceptance. Ellis's selected retained generation is `ed5e73e0acc433d1f87e3c006ce3763ba929c956ce3aed11aa5d061a8480c629`, profile `2f404c70725ad1162653bf2762c0f5590d0c3f7a1b83c4b9bf87e39091138fa5`; endpoint compatibility is still a separate N7 gate.

Each file replacement is atomic. The four-file pair/metadata operation uses flushed backups and one-wave repo-local transaction state for failure rollback and process-death recovery; it is not an instantaneous multi-file filesystem transaction. A killed process can leave a recorded `promoting` phase, and all other actions refuse it until `recover` restores the original pair/metadata/modes. Unknown target drift or damaged backup evidence rejects before rollback writes. Keep all other fixture users offline during processing/promotion/recovery. No arbitrary power-loss/storage failure guarantee is claimed. Metadata promotion restores read-only modes, publishes verified current hashes/versions separately from verbatim historical marker text, and deletes siblings after success/rollback.

The lasting C10 diagnostic now counts all usable retained fragments rather than accidentally retaining an exact-turn candidate filter after N3. That filter had hidden the real 498-row Ellis coverage; no source model or runtime policy changed in N5.

N10 deletion inventory: both normalizer Python files, both `fixture_normalization_*.rs` files, their test module entry and the cfg(test) Freshness use-proof inspector; remove their repo-local transaction/audit/scratch files only after reviewed normalization/C10 evidence is captured here. Keep the corrected C10 diagnostic and N2–N4 lasting product corrections.

N5 verification:
- Root formatting and locked build passed. Full locked tests passed: 1,264 library tests, 5 ignored environment/measurement fixtures (including the exact-fixture probe), 138 integration tests and doctests (0 tests). Both exhaustive native checkpoint/live-use recovery gates passed.
- Eleven disposable Python transport/guard tests passed, including nine injected before/after/commit failure boundaries, a real process exit after REL/PHY replacement followed by full pair/metadata recovery, original read-only modes, stale candidate/evidence, hardlink refusal, metadata drift, unsafe rollback and forged legacy-birth/actual-Dream guards. Transport tests use mocked semantic snapshots; they do not claim native fixture normalization.
- Native probe acceptance tests use real current owner records and verify unchanged bytes plus H1 rejection of an unprocessed tail. All three exact fixture pairs were inspected with ordinary owners; no candidate sibling was created and no authoritative file was opened for writing. Inspection copies are deleted.
- Architecture refresh/enforcement passed: 75 P4 guard fixtures, 161 classified APIs, 11 external-ownership fixtures and 23 Pitlord rules on 71,068 source nodes / 2,379 relationships; snapshot `afa45f2c502cb6db8e934d1f75aadda6d1d9372a2d64414c8c96bac403605663`. This is scoped static evidence, not proof of arbitrary dynamic calls.
- CLI public/library production behavior and Archive smoke path are unchanged, so no new CLI/smoke acceptance claim is made. All Rust additions are test-only; the C10 correction is in the existing ignored diagnostic. Verification uses repo-local `.t` scratch and process-local debug-disabled build profiles.

Documentation impact:
- Inspected: repository/shared guidance, current owner docs, fixture markers/hashes, H1/C10 plans and current normalization requirements.
- Updated: developer staging/recovery workflow, temporary boundary/invariants, maps/coverage/contracts, limitations and N5 disposition.
- Not affected: authoritative corpus bytes/metadata, public APIs, production migrations, storage framing and prior release evidence.
- Compliance check: final structural/change-impact documentation and patch hygiene results are recorded below.
- Known documentation gaps: actual normalization rules/results and C10 evidence remain N6–N10.

Architecture impact:
- Standards added or changed: no shared standard or repository rule.
- Ownership or boundary impact: temporary Python orchestration and read-only cfg(test) owner probes only; current owners retain storage/publication/scoring authority.
- Pitlord enforcement impact: fresh policy evidence passes; no new owner area/rule.
- Other verification impact: native standard-owner probes plus bounded transport/fault/recovery tests and full native regressions.
- Known architectural gaps: actual candidate processing/legacy approximation remain N6/N7; full retained-corpus acceptance remains N8.
- All ten N1 design oracles passed. Final source-pair rehash confirms all six pinned hashes unchanged and zero candidate siblings. Both required documentation-policy modes and patch hygiene passed on the final recorded tree.

## N6–N10 disposition — 2026-10-13

All three authoritative REL/PHY pairs are normalized in place, retain their typed owner UUIDs, reopen through ordinary current owners, and passed full native preservation/H1 acceptance. The [verification record](freshness-retained-normalization-verification-2026-10-13.md), [before/after evidence](../benchmarks/freshness-retained-normalization-2026-10-13.json) and [C10 measurements](../benchmarks/freshness-historical-corpus-2026-10-13.json) retain exact hashes/byte lengths, activity/branch/history proof, derivation claims and actual versus approximation coverage.

N6 completed missing all-branch Import coverage through current Insomnia: 14-day is 456/456 complete with 1,123 REL Memories; 28-day is 1,344/1,344 with 2,847. Unique legacy publication evidence authorized 1,087 / 2,816 current birth reencodings without changing original Memory revisions/bodies/provenance or accepted positions. Genuine current Dream completed 36 / 31 new publications. N7 retained Ellis's 143/143 baseline and 725 REL Memories, proved 678 legacy births and genuinely completed its 47 current first-cycle Dream passes. Selected real compatible fragment/target vector coverage is complete for all three; no exact-turn fragments or fabricated vectors were created.

Legacy Dream approximation derives only from accepted direct Graph publication/evaluated-pair evidence and is labelled for 1,087 / 2,816 / 678 legacy Memories, including justified no-event coverage. Actual pass records remain distinct. The normalizer never converts approximation into actual first-cycle proof. Native checkpoint acceptance, byte-stable exact retry/reopen, and subsequent accepted-use preservation passed; synthetic use receipts were tested only on disposable copies.

N8 measured new production-native checkpoint acceptance from each normalized authoritative checkpoint's hash-verified input prefix on disposable copies. Nonempty recipient contributions were 13,377 / 21,346 / 29,463. The authoritative stored legacy approximation checkpoint remains unchanged; native-only remeasurement and stored-checkpoint Fresh/Stale/Dormant distributions are reported separately. CPU/peak working set/pagefile, semantic mixed-fragment coverage/matches, graph traversal and checkpoint append/reopen/retry are captured. C10 uses zero inference. Allocator-only temporary peak and spill high-water were not instrumented and are not claimed.

N9 found a genuine current authority bug: a duplicate alias could not resolve when its representative was later superseded. The lasting Dream resolver traverses duplicate components and supersession chains together, requiring agreeing unique accepted edges and proved owner-local births. Cycles/conflicting pointer/edge evidence still reject. Thirteen native matrix tests pass, including combined-chain reopen and immutable-history preservation. The ordinary import path retains actual Dream proof; missing proof remains unsupported. This correction introduces no migration registry, compatibility chain, new owner or format.

N10 deleted both temporary normalizer Python files, all four fixture-only Rust probe/work files, their module entry, the Freshness use-proof inspector, temporary diagnostic example, three fixture-only cache branches, temporary credential copies, transaction/audit scratch and the recorded C10 copies. No candidate/rollback sibling remains after promotion. The lasting generic C10 accepted-prefix measurement and mixed-fragment diagnostic remain. Current API/architecture/invariant/storage/development/routing/limitation docs now describe the final behavior; historical N1–N5 evidence above remains dated evidence. C11 full release verification resumes on the cleaned tree; C12 production disposition is still open.

Documentation impact:
- Inspected: repository/shared guidance; architecture, invariants, storage/API, development, maintainer/coverage/behavior maps, limitations, roadmap and active plans.
- Updated: current fragment/authority semantics, deleted-tool guidance, retained before/after record, current fixture hashes and N6–N10/C10 disposition.
- Not affected: public option shapes, default activation, mounted historical management, physical format framing and dated earlier release evidence.
- Compliance check: final structural/change-impact and patch hygiene results are recorded with C11 below.
- Known documentation gaps: C11 final verification and C12 production disposition remain until their explicit gates pass; allocator/spill measurement scope is explicit.

Architecture impact:
- Standards added or changed: no shared standard, format, migration registry or semantic owner.
- Ownership or boundary impact: Archive retains source/activity/vector authority; Dream retains canonical/proof authority; Freshness retains receipt/checkpoint/scoring authority. Conversion code is removed.
- Pitlord enforcement impact: final source refresh/enforcement is part of resumed C11; no new owner area/rule.
- Other verification impact: all three real authoritative candidates/native C10 measurements, composed-authority regressions and existing recovery/history gates.
- Known architectural gaps: mounted historical management/default activation and C12 production disposition remain out of scope.

### Final N10/C11 verification

Subsequent disposition: [C12 is complete](freshness-historical-production-disposition-2026-10-13.md) for supported explicit offline REL reconstruction. Earlier C12-open statements in the dated N1–N10/C11 history are superseded; default activation and PHY remain outside the release.

Root formatting/locked build and the complete locked suite passed: 1,266 library tests, four ordinary ignored measurements/environment fixtures, 138 integration tests and doctests. CLI formatting/locked build and all 19 tests passed. Both exhaustive native checkpoint/live-use byte-prefix recovery fixtures passed. All 10 N1 design oracles and the two-Node/one-branch/one-mixed-fragment Archive roundtrip passed. Final structural/change-impact documentation checks and staged/unstaged patch hygiene passed.

Fresh architecture enforcement passed: 75 P4 fixtures, 161 classified APIs, 11 external-ownership fixtures and 23 Pitlord rules; 70,932 source nodes / 2,379 relationships, snapshot `dd2d545418a3103ed844209427941ba294ff4fde8eee39c6a29267ff01993b99`. This is scoped static enforcement evidence. Repository-local test scratch is removed. No normalizer implementation, fixture-only hook, credential copy or promotion sibling remains. All N6–N10 gates are complete and resumed C11 source verification is green; C12 production disposition remains open. See the dated [verification record](freshness-retained-normalization-verification-2026-10-13.md) for exact commands and complete impact fields.
