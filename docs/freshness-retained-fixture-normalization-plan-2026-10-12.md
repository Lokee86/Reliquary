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
| **N3** | **Luna** | Refactor historical manifest/event planning and C7/C8 embedding fill to the same fragment-source model. Remove exact-turn fragment creation/fill requirements. | H1 manifest proves fragment/generation/profile/availability evidence; historical semantic fixtures use mixed fragments deterministically. |
| **N4** | **Sol** | Change existing-REL H1 chronology validation to consume canonical Archive activity order. Keep timestamps as provenance evidence and preserve fresh-import deterministic order. | 14-day order mismatch and 28-day timestamp regressions no longer fail solely because timestamp sort differs from accepted Archive order. |
| **N5** | **Sol** | Implement the disposable fixture normalizer with exact owner/hash allowlisting, current-standard output only, candidate verification and atomic same-path promotion. No public migration API changes. | Tool refuses non-allowlisted files and cannot leave a partially promoted authoritative fixture. |
| **N6** | **Luna** | Normalize 14-day and 28-day in place: complete genuinely missing H1-required generation work, establish current fragment/target vector state, normalize provable pre-Freshness birth evidence, and materialize the accepted Dream legacy approximation where actual proof is absent. | Both authoritative fixtures reopen under ordinary current code, preserve owner/Archive/Memory identity invariants, have updated hashes, and pass historical preflight. |
| **N7** | **Luna** | Normalize Ellis in place from the new 143/143 baseline: complete compatible current vector state, required Dream work for newly published Memories, and older pre-Freshness normalization. | Same authoritative REL/PHY paths reopen and pass H1 generation/vector/Dream preflight; hashes/marker are updated. |
| **N8** | **Luna** | Rerun C10 retained-corpus acceptance/measurements on all normalized fixtures. Capture CPU/memory, semantic fragment coverage/matches, graph traversal, checkpoint cost/reopen and final Fresh/Stale/Dormant distribution. | At least one—and preferably all three—authoritative retained corpora complete the production historical path with nonempty justified events and reproducible hashes. |
| **N9** | **Sol** | Review normalization output for any rule that accidentally became required for normal post-release operation. Promote only genuine product behavior (mixed fragments, accepted clock); keep fixture conversion itself non-product. | No unreleased-version compatibility subsystem has leaked into the maintained runtime. |
| **N10** | **Luna** | Delete the fixture normalizer and any hook used only by it. Update current docs/API to the implemented fragment-source semantics, retain before/after hash/evidence report, then resume C11. | Final tree contains no temporary fixture migration implementation; C10 evidence remains reproducible from the retained verification record. |

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
