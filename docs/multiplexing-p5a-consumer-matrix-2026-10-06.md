# P5-A source-backed REL consumer matrix — 2026-10-06

Parent: [P5-A execution plan](multiplexing-phase5a-rel-consumers-2026-10-06.md). This is the **in-process source and API contract**, not a claim that Warlock has migrated. P5-C owns external caller changes; P5-D owns public exports, root wiring and deletion. Inspected the existing gateway implementation, legacy host implementations, and actual sibling `Warlock-v2/src-tauri/src` callers.

## Real caller → canonical contract

| Actual caller / old host API | Canonical gateway contract | Authority and durability | P5-A disposition |
| --- | --- | --- | --- |
| `workspace_archive_search.rs` → `search_visible_archive` | `search_visible_archive(instance, query, limit)`; `VisibleArchiveSearchResult` with owner-tagged branch hits | Instance-selected REL and **each separately ViewRel-granted dependency**; branch search is read-only, no event | **Added** all-leaf search/dedupe/sort; **not interchangeable** with existing CVA `search_archive` / `search_dependency_archive` (different hit types and search scope) |
| `workspace_archive_search.rs` → `search_archive_owner` | `search_archive_owner(instance, exact_owner, query, limit)` | Exact ViewRel on requested REL; no ambient fallback | **Added** |
| `workspace_memory_search.rs` → `search_visible_memories` | Existing `search_visible_memories(instance, query, limit, depth)` | ViewRel for each REL and UseOwnPhy only on verified attached PHY; capture grants before embedding and revalidate after; retrieval read-only | Existing; preserve private result lane |
| `workspace_knowledge_mutation.rs` → `create_knowledge_memory_for_owner` | `create_knowledge_memory_for_owner(instance, owner, title, content, category, kind, now_ns)` | REL ManageRel → synced P4 Semantic/Retrieval notification; private PHY UseOwnPhy → PHY sync, no REL notification | **Added** owner-polymorphic mutation; P5-C must supply timestamp |
| `workspace_knowledge_mutation.rs` → `replace_knowledge_memory_for_owner` | `replace_knowledge_memory_for_owner(instance, owner, memory_id, title, content, now_ns)` | Same grants/durable boundaries as create; successor memory, relation inheritance, Supersedes edge and old-memory retirement | **Added**; intentionally **not** the existing in-place revision-checked `replace_knowledge_memory_for` |
| `workspace_knowledge_mutation.rs` → `mutate_knowledge_relation_for_owner` | `mutate_knowledge_relation_for_owner(instance, owner, source, target, old_kind, new_kind)` | REL ManageRel or exact UseOwnPhy; REL P4 Semantic/Retrieval; PHY private sync only | **Added** |
| `workspace_knowledge_mutation.rs` → `rename_knowledge_community_for_owner` | `rename_knowledge_community_for_owner(instance, owner, community, name)` | REL ManageRel via existing committed rename; PHY UseOwnPhy private sync | **Added** |
| `workspace_knowledge_provenance.rs` → `resolve_knowledge_provenance_for_owner` | Same named gateway API, with `instance` argument | REL ViewRel; private PHY UseOwnPhy; separately ViewRel-authorize source REL; no source-ref implicit grant | **Added** None/Unresolved/Resolved owner-scoped resolution |
| `workspace_memory_provenance.rs` → `read_memory_provenance_for_owner` | Same named gateway API, with `instance` argument | REL ViewRel only, explicit error for PHY Archive provenance | **Added** |
| `workspace_knowledge*` → read memory/knowledge/entities/relations/stats | `knowledge_memory_for_owner`, `read_knowledge_for_owner`, `entities_for_owner`, `semantic_graph_relations_for_owner` | REL ViewRel, or verified PHY UseOwnPhy; read-only, never PHY through REL snapshot | Existing |
| `workspace_echo.rs`, `workspace_memory_provenance_view.rs` → Echo/file/source views | `read_echo_source_for`, `echo_events_for`, `files_for_source_for`, `resolved_file_bytes_for`, `resolve_external_memory_provenance_for` | Exact REL ViewRel for source and file, checked at delivery after I/O | Existing |
| `workspace_open.rs` / project/reconcile helpers → `validate_project_environment_for`, `project_repository_root_for`, checkpoint/upload | Same-name `GatewayRuntime` methods with `instance` and exact owner | ManageProject for external Git/filesystem operations; owner fencing and post-sync P4 project/archive events for changes; file *reads* require ViewRel | Existing |
| `workspace.rs`, `workspace_open.rs` → host global selection/status/session | Instance attach/select, exact owner status, P3 conversation coordinator and P4 stream APIs | Per-instance attachment and shared one-REL ownership; never host-global `active_rel_id`, `active_execution` or `phylactery_owner_id` | Already canonical; C owns external rewiring |
| `workspace_project_create.rs` / `workspace_project_adoption.rs` → `create_project_environment_rel`, `inspect_project_adoption_source`, `adopt_project_environment_rel` | New owner creation/adoption is a *trusted quiesced* lifecycle path, not an in-place caller-supplied mutable REL operation | No active owner grant can exist before creation; owner mount and provenance must be validated prior to attachment | **P5-C/D contract decision required**, not incorrectly marked covered by project maintenance APIs |

`workspace_archive_search.rs` additionally calls `search_archive_owner`; old `search_visible_archive_owner` has no observed direct Warlock caller. Existing canonical transcript/page/search, summaries/page, `search_dependency_archive_for`, metadata/dependencies, reconciliation, status and quiesce/reopen methods remain usable; no duplicate gateway wrappers were added.

## Authorization, publication and negative gates

A1: exact selection and ViewRel required; each dependency grant is independent. No dependency closure grants permission. Branch/archive hits are owner-tagged and no PHY lane is searchable as Archive.

A2: writes first authorize ManageRel or verified instance-bound UseOwnPhy; REL writes use the **existing** `with_rel_mutation` for commit/sync and P4 publication; PHY writes use `with_phy`/private sync and never produce a REL feed event. Reads and provenance preserve separation. Replacement is successor-based (including source-reference preservation on private successor), not in-place; semantic edit does not create a compatibility profile or background inference route.

A3: use already-present project/environment, exact owner status, quiesce/reopen and safe reconciliation methods. External I/O remains outside semantic-owner locks. Do not reopen closed owners via a new convenience wrapper.

A4: classify public P5-A APIs in `scripts/check_multiplexing_phase4_coverage.py`; run P2/P3/P4 source guards and focused runtime negatives. Textual enforcement is supporting evidence, not a runtime safety proof.

## P5-C consumer handoff

Warlock is **not migrated on this branch**. P5-C supplies per-attachment `InstanceRuntime` context, explicitly passes REL/PHY IDs and two mutation timestamps, and routes frontend envelopes onto the canonical response types. P5-C must prove two simultaneously attached principals viewing one REL and separated private PHY results, subscription/cancellation stream behavior and upgrade/pin alignment; do not assert that existing Warlock compiles with these new methods. P5-C should treat Archive all-leaf search and CVA Archive index search as separate semantics.

## P5-D root/type relocation and deletion manifest

`VisibleArchiveSearchOwner`, `VisibleArchiveSearchResult` and `RuntimeKnowledgeProvenanceResolution` are presently publicly named by the **legacy host source modules**. P5-D must move their **type definitions** to canonical public modules before deleting those legacy files; canonical gateway methods depend only on their data shapes, not the legacy runtime owner. P5-D also owns root exports and P4 guard integration reconciliation.

Re-audit and remove once P5-C migrated all calls and D tests pass: `runtime_host_archive_search.rs`, `runtime_host_owner_archive.rs`, `runtime_host_knowledge*.rs`, `runtime_host_memory_*.rs`, `runtime_host_semantic_*.rs`, `runtime_host_project_environment.rs`, `runtime_host_status.rs`, `runtime_host_interaction.rs`, `runtime_host_session.rs` and old-only characterization tests; do not delete shared types/functionality prematurely. Worker owners are P5-B and are **not** deletion candidates from this lane.

## Limits

A source-backed first-party audit is not evidence of all unknown external consumers. Final root exports, Warlock integration and release verification remain P5-C/D/P6 and must not be labeled accepted by P5-A.
