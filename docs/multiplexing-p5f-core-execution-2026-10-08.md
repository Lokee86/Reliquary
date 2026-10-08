# Multiplexing P5-F-core workspace management — 2026-10-08

Parent: [P5-F specification](multiplexing-phase5f-workspace-management-2026-10-06.md)

Wave: [Next parallel execution plan](multiplexing-phase5-next-parallel-execution-plan-2026-10-07.md)

Branch: `feat/multiplexing-p5-management`

Base: `999fbf7d85c9f892a1bf979293588033bcc1484b`

## Purpose

Record the F0 capability matrix, F1–F4 concrete management composition, verification and remaining owner/convergence dependencies. F5 selective import/export remains outside this core cut.

## Overview

`GatewayRuntime::workspace_management` binds a shared gateway and an already verified instance. `WorkspaceManagement` stores no semantic data, queue, selection copy, recipient or cache. Its private handles only forward to existing Gateway/P3/P4/A/B authority. DTOs are immutable owned values. Named root re-exports remain a single convergence-owned cut; public consumers can already call the factory and inspect inferred results.

The implementation is split into the facade/DTO module plus conversation, file/project, knowledge and health composition modules. It introduces no stored record, codec, transport dependency or alternative worker.

## F0 capability matrix

| Product need | Existing primitive / authority | Grant and scope | Bounds / publication / live safety |
| --- | --- | --- | --- |
| REL and personal owner listing | viewable_rel_owners, exact semantic_owner | independently granted mounted RELs; own bound UseOwnPhy only | stable ID ordering; attachment delivery fence; fresh pages |
| Conversation listing and transcript | conversation_summaries_for, conversation_transcript_for | exact REL ViewRel | 1–128 rows; transcript chronology; synchronized native prefixes |
| Session/branch inspection | conversation_observation_snapshot, instance_managed_state | exact REL ViewRel | P3 bounded durable source; paged branch DTOs; no writer admission |
| Open/select/history/clear | select_managed_conversation, select_managed_conversation_leaf, clear_managed_conversation_selection | exact REL ViewRel | instance-local only; no semantic publication |
| Explicit writer branch preparation | prepare_conversation_branch_at_leaf | exact REL SubmitConversation | existing durable Archive branch and P4 invalidation; independent of selection |
| Title and policy | set_conversation_title_for, set_conversation_policy | exact REL ManageRel | existing owner validation, synchronization and P4 publication; no new policy revision scheme |
| Source file inventory and refs | files_for_source_for, project_file_ref_for | exact REL ViewRel | source-turn-qualified; sorted FileId page; no invented whole-file inventory |
| Resolved file read | resolved_file_bytes_for | exact REL ViewRel | native or exact project revision; original-token delivery; 8 MiB returned-byte cap |
| Project correlation/read/validate/checkpoint/upload | latest_project_revision_correlation_for and project operations | ViewRel correlation; ManageProject mutation | existing filesystem/Git work outside semantic locks; existing sync/P4 publication |
| Memory inventory and exact read | read_knowledge_for_owner, knowledge_memory_for_owner | ViewRel or exact bound UseOwnPhy | stable MemoryId page; lifecycle/successor inspection; owner controls semantics |
| Provenance | resolve_knowledge_provenance_for_owner | original semantic owner plus independently authorized source REL | unavailable sources stay unresolved; a source reference grants no access |
| Entity/Graph/Community inspection | entities_for_owner, semantic_graph_relations_for_owner, read_knowledge_for_owner | ViewRel or exact UseOwnPhy | paged existing data; reserved principal Entities excluded from generic inventory |
| Create/replace Memory, rename Community, change relation | A concrete owner-qualified mutators | ManageRel or exact UseOwnPhy | synced existing successor/retirement/relation semantics; REL P4 invalidation |
| Exact owner statistics | B background counters and A EntityStats | ViewRel or UseOwnPhy | current owner samples, never mixed REL/PHY state |
| Live REL health | existing P4 source reader under sequencer gate, B workers, capability state | exact REL ViewRel | body-free clocks/counters; coherent P3/P4 source cut; sampled runtime fields |
| Quiesce/reconcile/reopen | existing gateway maintenance | exact REL ManageRel | reconciliation requires closed/drained/stopped owner; reopen rotates epoch |
| Rename/move/unlink, Memory deletion | missing retention/lifecycle contract | authorized explicit unsupported result | no destructive best effort |
| Standalone file inventory/import | missing A gateway primitive | authorized explicit unsupported result | owner request below; no raw-store escape |
| Selective semantic import/export | G1/F5 | explicit unsupported result in core | separate acceptance; no ad hoc codec |

Every request validates page bounds before reading. Offset addition overflow is rejected. Pages return total count and next offset; they are fresh deterministic reads, not durable cursor/snapshot promises. Inventory mutations between requests require restarting pagination. Primary browsed text is capped at 1 MiB; exact file bytes at 8 MiB. Existing knowledge/transcript/file primitives can allocate the complete source before slicing/capping. These response bounds are not a claim of bounded owner I/O or allocation.

## F1–F4 behavior

Conversation selection belongs only to the initiating instance; listing/inspection opens no journal entry, branch or generation. Writer branch preparation is a separate permitted action. Memory replacement deliberately uses A's existing successor plus retirement semantics, rather than silently selecting the different in-place editor.

Composite read results retain an original owner token across component reads and revalidate it at delivery. Delivery-only REL/PHY closures cannot inspect or mutate owner state. Grant withdrawal/regrant, detach and remount invalidate that token. No admission lease spans external filesystem/provider calls. Mutations delegate the complete existing owner operation and its grant/durability/publication rules.

Health includes exact REL source clocks, conversation/branch/pending/held/cancellation counters and worker/capability facts. It includes no transcript, checkpoint text, payload digest, provider error, private Memory body or dependency inventory. Health does not claim a filesystem integrity audit or trigger repair. Ordinary inspection rejects quiesced owners; explicit reconciliation supplies the existing closed-owner verification/repair workflow. Worker/capability fields are fresh samples rather than an atomic cross-domain snapshot.

E's final DTO shape is not frozen, so this core composes existing N0 P3/P4/B facts directly. It defines no alternate generation status enum, live session or capability authority. N3 can consume E where useful without retaining duplicate DTO definitions.

## Owner requests and remaining integration

- A: add exact-grant whole-file inventory and standalone native import primitives, with original-token delivery and canonical P4 mutation admission. Until then source-qualified inventory and project upload are supported; standalone capabilities explicitly report unsupported.
- A: provide truly bounded Memory/Community/transcript/file reads if source-allocation guarantees are required. Current facade paging bounds returned primary text and item count only.
- G/F5: freeze G1 source/message/artifact identity and import retry/reopen semantics before selective import/export.
- Convergence/D: export `WorkspaceManagement`, `ManagementPage`, `ManagementError`, `UnsupportedManagementOperation`, `ManagedBranch`, `ConversationInspection`, `SourceFile`, `MemorySummary`, `CommunitySummary`, `ManagementVersions`, `RelManagementHealth`, `SemanticOwnerStatistics` and the bounds once the contract is accepted.
- C: switch real product management consumers to this surface at N3. This isolated lane does not edit Warlock.

F0 is complete. F1–F4 composition is implemented for the capability matrix above; standalone file primitive gaps remain explicit. This record does not claim full F5/F6, final exports, consumer convergence or complete roadmap management acceptance.

## Verification

| Check | Result |
| --- | --- |
| Root formatting and locked check | passed |
| Focused management unit scenarios | 11 passed, 0 failed |
| Public management consumer | 1 passed, 0 failed |
| Full root library | 1,110 passed, 0 failed |
| Full root integration | 129 passed, 0 failed (includes the public consumer) |
| Root doc tests | 0 tests; command passed |
| CLI formatting, locked check and tests | passed; 19 tests, 0 failed |
| Management ownership guard | passed; 25 negative/positive fixtures |
| P4 API/coverage guard | passed; 42 fixtures, 149 APIs classified |
| P2/P3 ownership enforcement | passed; no violations |
| Pitlord policy validation | passed; 15 areas, 13 rules |
| Documentation structural/change-impact checks | passed, including changed-from origin/main |
| Fresh architecture refresh | timed out after 240 seconds during Lexicon Rust/Python indexing; P4 source guard passed |

Commands used: cargo fmt --all -- --check; cargo check --locked -j4; cargo test --locked -j4 -- --test-threads=4; focused cargo test --locked -j4 management::tests --lib; cargo test --locked -j4 --test workspace_management; CLI fmt/check/test with --manifest-path cli/Cargo.toml and locked dependencies; management/P4 self-test guards; P2/P3 --enforce; Pitlord validate; documentation policy structural and changed-from origin/main checks; check_architecture.py --refresh.

The final runtime matrix uses job_HvFkok-cXygM4aUkOC28pF75. Focused final acceptance uses job_2Rt6eJjA6WFKKkoPux1LWl4j; direct ownership/policy acceptance uses job_ujkuTh_3gY0jO72rRktXG4QR. Final source/coverage/documentation/policy checks use job_mRIDuRQmaAG-PrBYCg28K2f2. The bounded fresh-index attempt is job_IeyubbysiRwyuW67m6Wavizh. No stale architecture graph is substituted for a fresh acceptance result.

Inherited warnings include unused items and MSVC LNK4098. An initial test filter matched zero unit tests and was corrected to management::tests; the subsequent two fixture failures were repaired to use the existing principal binding and SubmitConversation grant. The final focused and full matrices above passed. No product-source failure was waived.

## Documentation impact

- Inspected: architecture, invariants, storage format, maintainer map, limitations, roadmap, API, ADR 0016 and F/next-wave specifications.
- Updated: this record, index, F/next-wave lane status and branch-scoped architecture/invariants/API/maintainer/limitation/roadmap notes.
- Not affected: persistent REL/PHY/journal formats, source identity, configuration, CLI behavior and historical rollback.
- Compliance check: structural and changed-from origin/main checks passed again after final evidence updates.
- Known documentation gaps: whole-file/standalone primitive contracts, F5 and final C/D convergence remain listed above.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: F owns management composition/DTOs only; original semantic, journal, selection, worker and distribution owners remain.
- Pitlord enforcement impact: add deterministic management no-store/no-journal/no-second-gateway rules; existing runtime-inference owner remains.
- Other verification impact: focused management authorization/privacy/bounds/lifecycle/P4 and public-consumer tests, full library/CLI and source guards.
- Known architectural gaps: fresh architecture indexing timed out after 240 seconds; direct ownership/coverage checks passed. Missing A primitives and final E/G/C/D convergence are explicit.

## Related docs

- [Architecture](architecture.md)
- [Invariants](invariants.md)
- [Rust API](api.md)
- [Current limitations](current-limitations.md)
- [Maintainer map](maintainer-map.md)

## Notes

Core composition does not imply selective import/export, destructive retention policy, bounded source allocation or final product integration.
