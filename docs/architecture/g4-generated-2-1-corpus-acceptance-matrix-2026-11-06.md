# G4 generated native 2.1 corpus — fixture contract and independent acceptance matrix

**Status:** prescriptive acceptance contract, not evidence of an executed run.
**Parent:** [G4 complete-product specification](g4-complete-product-acceptance-spec-2026-11-06.md).
**Related:** [G4 execution tickets](g4-product-acceptance-execution-plan-2026-11-06.md), F03 reference ID goldens, G2/G3/GM/P01 accepted evidence.
**Acceptance rule:** Every **mandatory** row below needs a passing executable test and *independent observed evidence*. A green workspace suite does not substitute for an unexecuted G4 scenario.

## 1. Generated cohort v1

Manifest name: `g4-native-rel21-corpus-v1`. Fixed fixture seed: `g4-rel21-v1`. Source-controlled, versioned text/JSON scenario manifest; no mutable corpus binaries, authentic user data or credentials. Generate a fresh cohort under a disposable path for each run and keep an output-free dry-run mode.

| Alias | Kind | Purpose / invariant |
| --- | --- | --- |
| `rel-lantern` | REL, Project | Eight conversations; project-native Archive, Episode, Insomnia, Dream, Entity, retrieval and history authority |
| `rel-cascade` | REL, Connection | Four conversations; nonoverlapping owner and grant scope, native import/alternate branch and VCS evidence |
| `phy-reader-a` | PHY | Principal A's independently routed private Memories, identity/anchors where supported and history |
| `phy-reader-b` | PHY | Principal B's private evidence; never visible from A or either REL without exact authorized routing |

Principals: `principal-a`, `principal-b`, `operator`, `viewer`, `outsider`. All tokens are local test-only generated opaque strings. **No guessed or broad grant**: host admission explicitly grants owner/kind plus view/manage/operate as appropriate.
Capture actual randomly issued product owner IDs in the manifest **result**, then verify their equality across all reopen and cross-record references; stable IDs selected by a dedicated existing fixture builder may be fixed only when that owner API officially supports them.

### Natural-language script catalog (test data, not model-generated assertions)

These are inputs for deterministic end-to-end scenarios. Every conversation includes at least four accepted turns; add follow-up and assistant context naturally without changing the asserted source clauses. The stable `conversation_id` and `node_id` are authored fixture identifiers.

| Conversation | Owner | Ground truth / deliberate variation |
| --- | --- | --- |
| `L-01` release | Lantern | Project release label **Lantern**; project-owned fact grounded in a user turn |
| `L-02` schedule | Lantern | `principal-a` says preferred meeting window **afternoons**; user-owned Memory must be routed to `phy-reader-a` |
| `L-03` branches | Lantern | One accepted parent with **two different assistant replies** and a canonical branch; noncanonical branch preserved |
| `L-04` correction | Lantern | Earlier deployment target replaced by later explicit correction on a **forked reply**; retained revision/canonical policy exercised |
| `L-05` names | Lantern | Synthetic person `Mira` appears as Entity mention; distinguish existing, new and ambiguous candidate |
| `L-06` documents | Lantern | VCS file reference `docs/release-notes.txt` at one pinned Git/Lore revision, later working-tree edit must not alter original evidence |
| `L-07` retrieval | Lantern | Distinct project Memory with explicit lexical phrase and exact deterministic vector; retrieval and confirmed use |
| `L-08` terminal | Lantern | Tombstoned conversation and superseded Memory; history remains but terminal payload has typed availability |
| `C-01` notes | Cascade | Different project's versioned operating plan and source-grounded fact, no Lantern graph bleed |
| `C-02` privacy | Cascade | `principal-b` preference belongs only in `phy-reader-b` even when original message lives in Cascade REL |
| `C-03` import | Cascade | Fresh supported **native input** normalization and idempotent import epoch; no 2.0 owner file reading |
| `C-04` host | Cascade | **Third fork:** two competing host continuations; provisional generation, commit/cancel/held ambiguity and reconnect |

Target: **12 conversations, ≥40 accepted native turns, ≥3 branch forks across the cohort**, at least one completed Episode per REL, and at least one user-directed and one project-directed Insomnia publication. Manifest records the actual exact counts, branch IDs and source-node references; tests assert the named clauses and exact source match, not arbitrary free-form model interpretation. Add multiple confirmed and rejected Memory use attempts, Dream first-pass/duplicate/supersession, Entity match/ambiguity, vector profile, and a tested historical cut before and after mutation. Non-ASCII and multiline content must be included in at least one source and must roundtrip unchanged.

### Deterministic external decisions

- **Insomnia**: scripted candidate extraction anchored to the exact quoted clauses above; ownership classification, source-node binding and rejection path are expected from a fixed source manifest, not inferred by loose keyword checks.
- **Dream/Entity Resolution**: frozen decision responses for relation/canonical/duplicate and known/new/ambiguous outcomes, including an explicit rejection case; evidence exactly identifies the source Memory/body/owner.
- **Vectors**: fixed finite small-dimensional `f32` arrays and compatibility profile; consistent embeddings across restarts, one deliberately invalid shape and stale profile response.
- **Host**: durable idempotent mock implementing the **real negotiated contract**; fixed accepted/unknown/rejected status and exact manifest digest, plus interruption points.
- **Repository**: disposable Git or Lore history with two revisions and exact known bytes; neither a model output nor mutable working-tree state may replace historical content.
- **No live external network** and no opportunistic future model/provider or agentic inference changes. The semantic testing objective is deterministic **control flow, authority and publication**, not a numerical LME result.

## 2. Witness and observation model

Each scenario produces (as applicable) source input IDs, accepted Archive turn/branch/episode IDs, Memory revision/body and owner IDs, Dream/Entity/Graph changes, vector/profile IDs, Freshness event cut and receipt, host queue state/digest and observed history proof. Capture:
1. **Expected witness** authored from the source input and accepted contract **before** the test; independently normalized by alias where UUIDs are minted.
2. **Actual witness** from the public API and/or Gateway after the workflow; no direct store patching.
3. **Independently reopened witness** from a new reader or fresh external process on the committed 2.1 bytes.
4. **Negative witness**: error type, unchanged source/output hashes, absence of unauthorized receipt/record, held-vs-committed state.

Compare typed values and canonical versioned serialization (sorted IDs and stable field encoding) for semantic/history proofs. Do **not** compare arbitrary debug strings or random UUID bytes as expected goldens. A post-run digest is evidence only; expected truth must not be auto-populated from the same implementation under test. Preserve physically exact hashes for **source nonmutation** and **repeat-copy equality where GM explicitly requires byte equality**. Fresh independent close/reopen of a poisoned or incomplete owner must fail typed without mutation.

## 3. Acceptance matrix

Use the case IDs unchanged in code and the final report. All below are **mandatory**; existing tests cited are a starting seam, not proof that the composed G4 case has already run.

| ID | Scenario and test seam | Independently asserted positive result | Negative / fault result |
| --- | --- | --- | --- |
| `G4-01` | Build cohort using native 2.1 product create/open and accepted workflow builders | Four distinct correct-kind owners, stated corpus shape, source script hashes and stable identities; reader in new process verifies each | Wrong kind / malformed or 2.0-only header refuses with unchanged bytes |
| `G4-02` | Authenticated native conversation append/finalize, public management and Gateway | Parentage, 3+ forks/canonical branch, exact source content, clock/branch head and transcript after reopen | Provisional text and rejected finalize never become native Archive turns |
| `G4-03` | Insomnia Episode claim/extract/publish via Gateway + Application | Source-quoted candidates produce project Memory in REL and user Memory in specified PHY; exact provenance and terminal work receipt | Unaccepted Episode, forged span, wrong-owner route and bad claim fail; no orphan PHY/REL Memory |
| `G4-04` | Dream initial/canonical/duplicate/supersession via existing worker and owner proof | Exactly one accepted population/first pass and expected graph/canonical state; idempotent retry after restart | Zero-link and duplicate policy correct; no phantom edge or repeated Freshness birth |
| `G4-05` | Entity Resolution on accepted Memory mention, REL and PHY | Known/new/ambiguous outcomes and history tied to exact Memory revision/body and owner | Foreign target, stale body or unresolved ambiguity publishes no invalid Entity |
| `G4-06` | Source-bound Memory/Archive vectors | Exact profile IDs (including separate F03 golden-vector checks), vector rows, generation cut, consistent reopen; no duplicate embedding on retry | Wrong dimensions, profile drift and stale source cut reject and leave earlier mappings intact |
| `G4-07` | Bounded context + repository attachment with pinned Git/Lore revision | Included allowed REL/PHY Memory and exact historical repository bytes under configured budget | Foreign PHY, forged VCS revision, project grant revocation or budget overflow prevents leakage/commit |
| `G4-08` | Passive hybrid + community-aware agentic retrieval via Gateway | Expected candidate IDs/order, body revisions, canonical/active filtering; semantic/lexical paths preserve required source | Foreign/retired/pruned candidates excluded; stale community/graph handled by documented policy |
| `G4-09` | Host prepare/confirmed Memory delivery and Freshness | No boost on internal read; exactly one accepted-use receipt after confirmed consumed manifest; stable retry across owner/Gateway reopen | Unknown/rejected host, mismatched content, revoked grant or stale prepared result produces no credit |
| `G4-10` | Native 2.1 import into completed REL with exclusive epoch | Supported format normalized exactly, committed receipt/dedup, correct Archive topology, clocks/owner and independent reopen | Staged/import-locked, invalid profile, partial settlement or conflicting retry fails closed |
| `G4-11` | Durable Gateway native queue/fork, generation checkpoints/stream | One submitter-bound accepted submission, ordered checkpoint prefix and branch/receipt after restart | Cancel/finalize race, cursor replay, corrupted tail or unknown host state never publishes private uncommitted output |
| `G4-12` | Host authority/worker restart and authorization | Worker kind uniqueness, quiescence, durable pending work/retries, new valid principal resumes exact authority | Unsupported host contract held, foreign Instance/revoked token/wrong-kind owner cannot mount/execute/read |
| `G4-13` | Maintenance fresh-history rewrite REL and PHY on copies | NewOwner and PreserveOwner policies independently reopen, selected provenance closure/history and correct identities/generation | Stale plan, missing dependent evidence and no-replace collision reject; original cohort bytes unchanged |
| `G4-14` | Terminal Memory/Archive/Entity/Relationship pruning and profile repair | Eligible terminal payload removed, tombstone/history retained, active references preserved, independently reopened owner readable | Active/referenced payload and ambiguous carrier refuse; no source mutation or unreachable asserted retained output |
| `G4-15` | Proven unreachable-space reclaim on transformed copy | Supported orphan backing removed, all live mappings intact, independent reopen; second reclaim **exact no-op** | Incomplete or unproven references not reclaimed; failures publish no candidate |
| `G4-16` | Same-owner REL compare/prefix/divergent reconciliation | Supported replay preserves both branches, import/Dream/Insomnia/Freshness authority, stable history and byte-identical retry | Wrong owner, conflicting work/Dream history or unsupported divergent state fails without output or changing either source |
| `G4-17` | Historical cut/proof REL and PHY before/after mutations | Full retained revision/evidence streams, canonical semantic proof consistent across separate reopen, typed unavailable payloads | Future/foreign cut, modified revision stream or wrong owner proof cannot pass |
| `G4-18` | REL-only CLI against generated cohort in fresh OS process | Create/info/verify, bounded archive/history proof, policy dry-run, supported copy operations and JSON reports match public API | `cva`, `phy`, unknown/duplicate flag, 2.0 source, colliding output return nonzero JSON errors; sources unchanged |
| `G4-19` | External Rust consumer and public end-to-end management | Only root crate imported; bounded page, title, direct Memory, provenance, quiesce/verify/reopen, revocation and PHY privacy verified | Viewer cannot write, revoked principal cannot read or reopen, lost queue cannot be silently inferred |
| `G4-20` | Architecture/whole-repo integration gate | All locked/offline workspace/CLI/docs/consumer, current package DAG, formatting and approved source formats pass | No Root back-edge, mutable store exposure, 2.0 runtime reader, phantom pass or skipped mandatory G4 case |

### Priority and omission rules

- **Do not** mark a case passed because a prior gate ran its isolated unit test. The **specific joined G4 scenario** must observe expected outputs on the generated cohort.
- A prior accepted suite may be invoked as supporting **additional** evidence, particularly deep M03 completed-import/Dream/Insomnia history tests. If a complex branch cannot be composed into G4 while retaining the accepted interface, record the exact missing integration seam and fix the owner/workflow boundary rather than replacing the case with a one-assertion stub.
- For typed unsupported paths, assert **no publication**, unchanged source and clear error. Unsupported is acceptable only if it was already excluded by the accepted GM/P01 scope; it is **not** a license to omit supported scenarios.
- Duplicate cases do not require duplicate test code: a single fully scoped test may satisfy multiple IDs if its exact witnesses and negative assertions are individually recorded.

## 4. Artifact and pass/fail manifest

Recommended test files, subject to proof of the existing code seams:

- `tests/g4_public_product.rs` and/or `tests/g4_public_product/...` (public-only imports).
- Existing Gateway owner-worker, native queue, retrieval, Entity, vector and Dream **test modules**, with a bounded shared cohort fixture.
- `cli/tests/g4_corpus.rs` run against a generated file in a **fresh OS process**.
- `tests/external-consumer/src/main.rs` or a new small bin for independent open/inspect.
- A source-controlled scenario specification, e.g. `tests/fixtures/g4-native-rel21-v1.json`, with the authored expected aliases/source IDs. Tests regenerate all binary owners; never commit time-dependent output.

For each matrix ID, the final evidence table must record **test binary / function, fixed fixture seed + digest, fresh reader/process evidence, key positive assertions, negative assertions, result, and any correction commit**. For G4-20 record exact command, exit status, test totals, source commit, architecture edge counts and unexplained warnings separately. Each mandatory row is either `PASS` or `BLOCKED` with an evidenced reason. **No `PARTIAL` counts as acceptance.**

## 5. Boundary with archived corpus and release

F03's six old owner files and historical FRS07 decoder are **not input data** for this matrix. Their SHA-256 hashes and historical source heads are constraints for D01/D02, not synthetic G4 acceptance. The six F03 hash-derived identity goldens can be rerun unchanged as independent deterministic unit assertions. G4 proves the **new software** and its native 2.1 owners; D01–D03 prove conversion of *old* owners; G5 proves deployable performance and activation.