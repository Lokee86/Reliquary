# Ego status and decision inventory — 2026-10-14

Parent: [Documentation index](INDEX.md). This is the entry point for **implementation state, decisions, and remaining work**, not an alternative behavioral specification.

## Evidence and scope

- Baseline: consolidated `main`, including agentic Memory retrieval, live semantic Freshness (F8), and explicit offline historical Freshness reconstruction.
- The former `feat/freshness-semantic-historical-linear` and agentic retrieval feature lines have been merged into `main`; their separate worktrees/branches are retired.
- Evidence: Rust source, existing tests and retained verification records. A focused `cargo test --lib ego_` was started but cancelled while compiling dependencies, before any Ego tests ran. No fresh test pass is claimed.
- **Classification:** *Implemented* = present in production source with source/tests or retained verification evidence; not equivalent to freshly retested or deployed. *Partial* = implemented substrate does not fulfill the named target behavior. *Designed* = agreed behavioral requirements without completed consumer. *Open* = design questions remain. *Calibration* = values/operational thresholds still to measure or choose.

## Capability status matrix

| Capability | Status | Evidence / dependency |
| --- | --- | --- |
| PHY Identity (multiple selectable documents) | Implemented | `src/ego/ego_model.rs`, `ego_store_identities.rs`, `ego_identity_tests.rs`, `src/facade/phylactery_ego.rs` |
| PHY Personality document | Implemented storage only | `ego_store_documents.rs`, `ego_persona_tests.rs`; synthesis and editing policy open |
| REL and PHY Anchors | Implemented storage | `ego_store_anchors.rs`, `ego_anchor_tests.rs`, REL/PHY facades |
| Owner-local cached Web synthesis | Partial target | `EgoWebSynthesis` in `ego_model.rs`; singleton text/revision/watermark, not PHY-owned per-source-REL three-size bundle |
| REL accepted-turn activity | Implemented dependency | `src/archive/archive_activity.rs`, `src/facade/cva_activity.rs`, `tests/rel_activity_clock_contract.rs` |
| Signed REL Memory Freshness and historical cut reads | Implemented dependency | `src/freshness/`, `src/facade/cva_freshness.rs`, Freshness tests; not an Ego audit controller |
| Live semantic reinforcement (F8) | Implemented dependency | `src/facade/cva_semantic_live.rs`; intrinsic eligible REL behavior on `main` |
| Explicit offline historical reconstruction | Implemented dependency | `src/facade/cva_historical_import.rs` and C12 disposition; explicit offline/exclusive operation on `main` |
| Memory search and retrieval | Implemented dependency | `src/retrieval/memory_search.rs`, `memory_retrieval*.rs`; mandatory Ego audit integration missing |
| Dream duplicate/lifecycle relationships | Implemented dependency | `src/runtime_inference/dream_*.rs`; lifecycle admission adapter missing in Ego |
| Insomnia settled-source boundary | Implemented upstream processing; Ego coordination missing | `src/runtime_inference/insomnia/`; Ego refresh queue not implemented |
| Conversation compaction records | Implemented dependency | `src/archive/conversation_compaction_*.rs`, `Cva::conversation_compactions` |
| Authorized scope and PHY host | Implemented dependency | `src/runtime_inference/runtime_host_gateway*.rs`; Ego consumption not implemented |
| Working-Web cleanup/admission and Freshness overflow pruning | Designed; unimplemented Ego consumer | [Memory-Web design](ego-web-synthesis-plan.md), `src/runtime_inference/freshness_access.rs` low-level helper only |
| Mandatory broad audit and coverage | Designed; unimplemented | Whole admitted Web exposed; full-REL `memory_search` permitted |
| Candidate selection | Open | Stage and output purpose decided; criteria, overlap and detailed mechanics unresolved |
| Provenance-linked candidate bullets and mean-source Freshness | Designed; unimplemented | [Memory-Web design](ego-web-synthesis-plan.md) |
| Candidate-summary reconciliation | Partially designed | Duplicate coverage avoidance, subject distinction and provenance decided; contradiction behavior open |
| Three fixed stored summary sizes | Designed; unimplemented | Needs size calibration and PHY/source-REL persistence contract |
| REL and PHY refresh | Designed; unimplemented Ego scheduler | REL bootstrap 25 Memories, PHY first useful mutation, queued settlement boundary; cadence values open |
| Personality synthesis and refresh | Open | PHY-behavior-only evidence ownership decided; generation/update contract undecided |
| Cross-chat context | Designed; unimplemented selector | [Cross-chat plan](ego-cross-chat-context-plan.md); consume existing current-REL compactions |
| Cross-REL summary inclusion | Designed; unimplemented | [Cross-REL plan](ego-cross-rel-summary-inclusion-plan.md); requires per-REL stored summaries and host authorization |
| Final Ego context assembly/projection | Partial design; unimplemented | ADR 0037 owner boundary, priority/budget outline; concrete runtime composition and host integration missing |

## Decided behavior — authoritative references

- **Memory-Web synthesis:** [Ego Memory-Web Summary design](ego-web-synthesis-plan.md). Objective lifecycle cleanup; whole eligible Web admitted while it fits; only then Freshness-ordered pruning to a Fresh (>0) mandatory audit floor; all retained Memories receive mandatory audit exposure, possibly in controlled batches; full authorized Web remains searchable. Audit proposes broad subjects, while a separate stage selects source sets. Candidate summaries are grounded bullets with source provenance and arithmetic-mean source-Memory Freshness. Reconcile then produce three fixed stored sizes, never truncating or padding to reach a token target. At runtime select an existing fitting size without startup synthesis.
- **Refresh:** first REL eligibility at 25 eligible Memories; PHY at first useful Memory mutation; percentage-plus-cap source-progress rules (numeric values open), queued behind required Insomnia/Dream settlement at a frozen source boundary; no wall-clock backstop. [Memory-Web design](ego-web-synthesis-plan.md).
- **Cross-chat:** reuse current REL's conversation compaction records, ordered by most recent activity, within remaining Ego budget; current-session compaction is distinct. [Cross-chat plan](ego-cross-chat-context-plan.md).
- **Cross-REL:** PHY-owned per-source-REL summaries, underlying evidence remains REL-owned; only active PHY's existing summaries from host-authorized dependency or separately authorized reverse/related scopes; no default sibling access, mega-summary, or second unlimited budget. [Cross-REL plan](ego-cross-rel-summary-inclusion-plan.md).
- **Ownership:** Reliquary owns context semantic policy and source authority, Warlock coordinates integration; PHY Identity/Personality/Anchors are separate from REL-owned evidence. [ADR 0037](decisions/0037-reliquary-context-memory-project-environment.md), [architecture](architecture.md).

## Open decisions (do not treat as approved designs)

| Topic | Still open |
| --- | --- |
| Temporal exemptions | Whether any lifecycle/temporal category bypasses Freshness pruning; **no accepted generic protected keep lane** |
| Audit operation | Exact batch size, overlaps and retained context between batches |
| Candidate selection | Selection criteria, source-set schema beyond subject + exact membership, overlap policy |
| Reconciliation | Handling of unresolved contradictions |
| Bullet selection | Tie-breakers or other secondary pruning criteria beyond mean source Freshness |
| Persistence | Final binary format and identity/keying of PHY-owned source-REL summary bundle |
| Personality | Evidence selection details, generation, edits, refresh, shape and budget |
| Context assembly | Final allocation/projection choices and interactions among layers |
| Cross-chat | Uncompacted tail handling, oversize compaction behavior and concurrently open chats |

**Explicit non-decisions:** no prescribed `CandidateResolver` merge/split/reject engine, `SummaryUnit` or `ReconciledSummaryLedger` types, mandatory per-Memory dispositions, `Essential/Core/Context/Detail` categories, nested retention tiers, or invented contradiction-resolution semantics. See [the source design's explicit non-decisions](ego-web-synthesis-plan.md).

## Calibration (not license to invent behavioral policy)

- Three stored summary token targets and hard ceilings.
- REL refresh growth fraction and absolute accepted-turn cap.
- PHY refresh growth fraction and absolute eligible mutation cap.
- Audit batch/overlap sizing once a correct mandatory-coverage contract is established.
- Ego input allocation around the approximately 20% usable-window guidance and Cross-chat's residual allocation.

## Dependency order and actionable next work

1. Preserve the current design/implementation distinction and close the listed open Memory-Web design questions before claiming a finished end-to-end implementation specification.
2. Specify the PHY-owned, source-REL-keyed three-size bundle persistence contract over the existing owner-local Ego substrate.
3. Specify/implement the staged Memory-Web controller: lifecycle cleanup → whole-Web admission / Freshness overflow pruning → mandatory audit → separately designed candidate selection → grounded bullet synthesis → reconciliation → size materialization.
4. Implement REL/PHY source-progress refresh coordination, durable queue semantics and frozen Insomnia/Dream completion boundary; reuse the existing REL clock rather than rebuilding it.
5. Implement Cross-chat selection and separate Personality synthesis once their contracts are settled.
6. Implement authorized Cross-REL summary selection and final host-facing Ego assembly/projection after summary persistence and source visibility contracts exist.

**Not part of this step:** new behavioral decisions, implementation code, or a claim that feature-branch Freshness work is merged/released. [Roadmap](roadmap.md) tracks scheduling; the three linked Ego plans own their behavioral designs.
