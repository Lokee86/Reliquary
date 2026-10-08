# Multiplexing P5-D0 — legacy-host deletion manifest

Parent: [P5-D specification](multiplexing-phase5d-legacy-deletion-2026-10-06.md)

## Purpose

Account for the legacy host's fields, methods, module registrations, source references, consumers, persisted touchpoints and final owners before D1–D4 removal. The filename retains the date requested by the D specification; the source audit was performed on 2026-10-08 UTC.

## Overview

D0 is an inventory and executable drift gate. It removes no Rust source, export, decoder, worker or stored data. Branch: `feat/multiplexing-p5d-inventory`; exact N0 base: `999fbf7d85c9f892a1bf979293588033bcc1484b`. Main and C/E/F/G worktrees remain untouched.

[The machine-readable inventory](multiplexing-p5d-source-inventory-2026-10-08.json) owns exact file hashes, registrations, lexical type/module references, every legacy-host method definition and candidate receiver call, all host fields and Shared/OwnerExecution/managed-session fields, test function names and consumer import evidence. Tables below own the reviewed disposition. Candidate references deliberately include ambiguous same-name calls; they are not a resolved Rust call graph. No stale Lexicon/Arcana graph is used.

**D0 completion does not release deletion.** E is an incomplete draft, C-close remains open, F-core and G-import are accepted only on isolated branches, and N3 has not integrated the final consumer surface.

## Audited consumers and integration frontier

| Checkout / exact revision | Observed source | Deletion consequence |
| --- | --- | --- |
| Reliquary N0 `999fbf7d85c9f892a1bf979293588033bcc1484b` | Accepted A/B plus C API repairs; old host still exported alongside canonical Gateway/Instance authority | D1 must relocate shared definitions and root exports before D2 |
| Warlock C-close `a65ee92c8b7f2c58cc274fc770c59918f196de56` | Clean; no concrete old-host references; local N0 verification only; production pins remain historical | C's final E/P4/provider/tool/retrieval and locked N3 acceptance remain open |
| Warlock final-cut `3a6247061e64422821f676c5bb9e7af8cba17c48` | Clean migrated snapshot; no concrete old-host references | Historical accepted consumer reference, not final C closure |
| Warlock main `4da92b3049cb7d88ea869d299cbe4226411e371c` plus dirty working tree | Five source files still refer to the old host; unrelated graph/UI changes retained | Do not certify main as migrated, overwrite it or prune it; integrate accepted C work deliberately |
| E `79a2d3424d821fed85ced6a833bb70e7c6677457` | Verified recipient-adapter draft; E5/N1 incomplete, final migration explicitly prohibited by its record | Durable session creation, external cancellation, typed tool/artifact receipts and final contract are blockers |
| F-core `13df1ff14ea8d7bfb1777289968ff7d5c87b3998` | F0–F4 supported composition accepted separately; F5 and standalone file primitives open | Final management exports and actual C consumers must converge |
| G-import `99b9fa5e83f9c0052d5ceef0952a47275b3903cc` | G0–G2 supported native profiles accepted separately; offline Cva composition, no legacy host dependency | Integrate applicable framework-neutral contracts; mounted F5 remains separate |
| P6-H `81a64b679ce8ecd650b649cf2cd419c4c6d62fad` | Accepted independent harness; H5 instrumentation tail | Preserve fixtures for post-deletion P6-R |

Warlock's audited manifests still pin Reliquary `83b3ac87a80a4221039fa07d9722f5f8076464ba` and direct Arcana `66b4e96ecf18cc79dfd6b1ea30232bef7c86bba3`. Local upgraded wrappers do not establish locked production identity. C must repin both to accepted N3, regenerate the lockfile and prove one Arcana package identity. D0 does not rerun or claim Warlock compilation against those historical pins.

Warlock main's concrete old-host files are `conversation.rs`, `workspace.rs`, `workspace_memory_provenance_view.rs`, `workspace_project_adoption.rs` and `workspace_project_create.rs` under `src-tauri/src`. The inventory records 16 concrete references across these five files.

CLI source has no concrete old-host references. Its configured finite runtime commands still use library-owned configuration/InteractionRuntime/Cva operations. Those standalone maintenance flows are not permission to expose raw owner handles through E/F. CLI is source-unaffected by D0; D1–D4 public library changes still require locked CLI checks/tests.

## Concrete ownership and removal rules

| Legacy authority | Final owner / disposition | Required evidence |
| --- | --- | --- |
| `ReliquaryRuntimeHost`, constructors/Drop, execution map, active key | Delete; `GatewayRuntime` keyed REL/PHY executions remain | No constructor/type imports or forwarding alias in source/consumers |
| `OwnerExecution` and mixed `Shared` worker set | Delete; `RelRuntime/RelShared` and `PhyRuntime/WorkerShared` retain separate B authority | One worker set per durable owner, restart/quiesce tests |
| Mutable global PHY slot, enable flag and mixed project/user profiles | Delete; gateway's verified principal/PHY registry and concrete owner profiles remain | Same-principal sharing, different-principal isolation, token revocation |
| Legacy managed session map and ambient active-session APIs | Delete; P3 coordinator/journal plus instance-local selection and accepted E adapter remain | Durable session/branch/resume, detach/restart, original authority |
| Legacy mount claims | Remove legacy claim holders only; retain one canonical MountClaim registry/fence | Alias/UUID admission and shared-process exclusion survive |
| Legacy queue/worker/router policy wrappers | Delete after B canonical replacements pass | No second Insomnia queue, journal writer or publication coordinator |
| Archive/file/project/Memory/Knowledge/Echo wrappers | Delete host methods; A concrete exact-owner primitives plus F/E composition remain | Exact grant, owner identity, synchronized publication and P4 admission |
| Shared error, routes, DTOs, bounds and synchronization helpers | Keep in narrow canonical runtime support/query modules, outside a surviving legacy host | Direct canonical callers and actual consumer imports recorded |
| Standalone closed-owner reconciliation | Keep under maintenance owner; remove legacy entry point | Closed/drained/stopped authorization, preservation/reopen tests |
| Persisted readers and semantic owners | Keep; runtime deletion does not authorize format removal | Existing legacy/current decode, recovery, reconciliation and reclamation tests |

`ReliquaryRuntimeHostError` is used by canonical Gateway/REL/PHY code and is not the concrete old-host type. Relocate and rename it in the single convergence export cut; migrate consumers directly rather than adding a forwarding alias. `ReliquaryRuntimeRoutes` remains a reusable endpoint configuration type. The canonical REL execution directly uses root `Control`, `IDLE_POLL`, `DREAM_RETRY_POLL`, `VECTOR_RETRY_POLL`, `operation`, `stopped`, `notify_work`, `wait_for_work` and `wait_for_work_timeout`; relocate these before deleting the old root. `RuntimeMemoryProfiles` and `owner_key` are old host state/helpers, not replacements for concrete owner profile/identity authority.

Shared DTOs to preserve include `RuntimeSemanticOwnerKind/RuntimeSemanticOwner`, `RuntimeKnowledgeState`, `RuntimeEchoSource/RuntimeEchoTurn`, `RuntimeBackgroundStatus`, reconciliation reports and Memory/Archive search result/default/bound contracts. Split DTOs and reusable scoring helpers from old host implementations. Canonical gateway source already builds `RuntimeBackgroundStatus` and uses the common query shapes. Root re-export paths alone do not justify keeping host methods.

## Persisted data and recovery obligations

| Touchpoint | Current concrete owner / reader | D disposition |
| --- | --- | --- |
| REL/PHY headers, current and concrete legacy owner identities | Container plus Cva/Phylactery lifecycle/migration | Keep decoders and typed admission; no data migration implied |
| Archive Nodes, branch revisions, source attachments, files, compaction and stream checkpoints | Archive/Cva and InteractionRuntime completed-turn/checkpoint machinery | Keep source authority, exact ancestry and sync-before-ack |
| Insomnia queue, extracted receipts, Memory bodies/vectors, Dream cooldown, Entity resolution, Graph/Freshness | Existing semantic stores and concrete REL/PHY workers | Keep durable state and restart rebuilding; remove duplicate legacy loops only |
| Conversation queue journal/snapshot `RQMUXJ01/RQMUXS01`, frozen host payload `RQMUXH01`, staged content-addressed blobs | `multiplexing_journal.rs`; P3 registry under `conversation-queues` | Keep decode/compaction/staging and pending/cancel/finalize/Unknown debt |
| Attribution/publication journal `RQOP0001` under `owner-operations` | `runtime_host_owner_operation_journal.rs` and publication codec/coordinator | Retain/rehome; these files are canonical despite their names |
| Project repository correlation, native file bindings, historical Lore/Git revisions | Archive/project environment; A gateway admission and closed-owner maintenance | Keep provenance, rollback and exact historical reads |
| P4 sequencer/subscribers/stream hub and disposable search indexes | P4 and retrieval owners | Keep canonical derived state; no new durable transcript owner |
| Native import receipt `CVAIMPT1` on G-import branch | Archive import validation/replay/reclamation | Preserve when integrated; absent at D0 N0 base, not an old-host decoder |
| Legacy execution/profile/session structs | In-memory old host | Delete after migration; no independent legacy-host serialized struct decoder found |

The old host's wrappers touch real semantic data; that is not evidence that the wrapper is a persisted reader. Retained reader implementation stays with its existing storage owner. The narrow unmounted reconciliation helper is directly called by gateway project lifecycle; deleting the entire legacy-named reconciliation file would remove a legitimate canonical caller.

## Test disposition and acceptance sequence

Retain all gateway, REL, PHY, instance, operational-journal and codec tests. Replace old-host worker, session, topology, semantic/query, project and privacy scenarios with equivalent exact-instance/owner cases where not already covered; remove a characterization only when its obsolete ambient behavior and replacement are explicitly identified. Legacy/gateway collision tests must become canonical alias/UUID exclusion tests once no old host can execute. `runtime_host_test_support.rs` is a split file: retain/relocate `test_path` and endpoint fixtures imported by canonical gateway tests, and remove old-host-specific wait helpers. The old dependency-graph helper has only legacy topology callers and is deleted with that wrapper; canonical authorized dependency composition remains in A. The semantic-graph runtime-host fixture also requires a canonical Graph mutation replacement.

D1 must re-audit this baseline on the accepted N3 tree, relocate common definitions and wire final E/F/G exports once, without compatibility forwarding. D2 then removes old authority and old-only helpers. D3 enforces the hard cut and verifies every surviving consumer. D4 updates current architecture/API/contracts/limits/ADR status after source removal. D0 deliberately retains current descriptions of the still-existing legacy API.

The no-unaccounted-callers gate requires:
1. Every affected root module has an explicit reviewed action, registration, type references, method definitions/candidates and test disposition in this manifest/inventory.
2. Exact source hashes and source/import inventory match `--check`; any drift triggers renewed review, not automatic regeneration.
3. `--hard-cut --warlock <final migrated checkout>` passes after D2; it must currently fail on the old host. Exact legacy identifiers are distinct from the shared error/routes names.
4. Full locked library/integration/examples/CLI builds/tests, external locked Warlock acceptance and actual provider/P4/lifecycle tests pass at the same converged revision.
5. Shared decoders and restart/publication/stream/privacy behaviors remain protected. Current fresh graph evidence can supplement source/compiler proof; an unavailable/stale graph cannot replace it.

The scanner masks comments/strings using the existing P3 lexical helper. It is not a full Rust parser, can over-report same-name module/type/method candidates and cannot prove macro expansion, aliases through arbitrary generated code, nested conditional semantics or unknown out-of-tree consumers. Concrete known consumer scans and compilation remain mandatory. `--hard-cut` is an identifier gate, not automatic permission to delete files.

## Verification commands

```text
python scripts/check_multiplexing_phase5d_inventory.py --self-test --check
python scripts/check_multiplexing_phase5d_inventory.py --check --warlock ../Warlock-p5c-close --warlock ../Warlock-p5c-final-cut --warlock ../warlock-v2
python scripts/check_multiplexing_phase2_ownership.py --enforce
python scripts/check_multiplexing_phase3_ownership.py --enforce
python scripts/check_multiplexing_phase4_coverage.py --self-test
python ../engineering-standards/tools/docs_policy/check.py --repo .
python ../engineering-standards/tools/docs_policy/check.py --repo . --changed-from origin/main
git diff --check
```

The later deletion gate is `python scripts/check_multiplexing_phase5d_inventory.py --hard-cut --warlock <accepted migrated checkout>`. `--write-baseline` is a deliberate reviewer operation; never run it automatically to hide drift.

Final D0 verification results are recorded in the completion section below. No Rust production source, dependency, schema or ownership changes occur in D0, so its acceptance is the source inventory/guard/documentation matrix rather than another inherited Cargo matrix.


## Per-module disposition

All 134 prefixed files are listed. Each exact JSON path records registrations, lexical module/type callers, test names and immutable source hash. Paths below are relative to `src/runtime_inference/`. The action is conditional on the D1–D4 gates, never filename-only deletion.

| File | Action | Final destination |
| --- | --- | --- |
| `runtime_host.rs` | split-root | canonical gateway root and runtime support; remove legacy host authority |
| `runtime_host_archive_search.rs` | split-common | Archive search result/default contract; delete old host methods |
| `runtime_host_backpressure_tests.rs` | replace-legacy-tests | equivalent Gateway/REL/PHY scenarios; remove obsolete ambient fixtures |
| `runtime_host_dependency_graph.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_dream.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_dream_owner.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_files.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_gateway.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_admission.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_admission_tests.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_ambient.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_ambient_archive.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_ambient_extended.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_ambient_extended_tests.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_ambient_project.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_ambient_tests.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_attribution_tests.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_background.rs` | retain-canonical | B concrete workers and host router |
| `runtime_host_gateway_background_tests.rs` | retain-canonical | B concrete workers and host router |
| `runtime_host_gateway_convergence.rs` | retain-canonical | P4 distribution; retain authoritative P3 reader |
| `runtime_host_gateway_convergence_deltas.rs` | retain-canonical | P4 distribution; retain authoritative P3 reader |
| `runtime_host_gateway_convergence_race_tests.rs` | retain-canonical | P4 distribution; retain authoritative P3 reader |
| `runtime_host_gateway_convergence_tests.rs` | retain-canonical | P4 distribution; retain authoritative P3 reader |
| `runtime_host_gateway_conversation.rs` | retain-canonical | P3 coordinator/journal and Gateway |
| `runtime_host_gateway_conversation_auto_branch_tests.rs` | retain-canonical | P3 coordinator/journal and Gateway |
| `runtime_host_gateway_conversation_branch_host_tests.rs` | retain-canonical | P3 coordinator/journal and Gateway |
| `runtime_host_gateway_conversation_branch_tests.rs` | retain-canonical | P3 coordinator/journal and Gateway |
| `runtime_host_gateway_conversation_continuation.rs` | retain-canonical | P3 coordinator/journal and Gateway |
| `runtime_host_gateway_conversation_finalization.rs` | retain-canonical | P3 coordinator/journal and Gateway |
| `runtime_host_gateway_conversation_finalization_tests.rs` | retain-canonical | P3 coordinator/journal and Gateway |
| `runtime_host_gateway_conversation_generation.rs` | retain-canonical | P3 coordinator/journal and Gateway |
| `runtime_host_gateway_conversation_generation_tests.rs` | retain-canonical | P3 coordinator/journal and Gateway |
| `runtime_host_gateway_conversation_metadata.rs` | retain-canonical | P3 coordinator/journal and Gateway |
| `runtime_host_gateway_conversation_observation.rs` | retain-canonical | P3 coordinator/journal and Gateway |
| `runtime_host_gateway_conversation_observation_tests.rs` | retain-canonical | P3 coordinator/journal and Gateway |
| `runtime_host_gateway_conversation_policy_tests.rs` | retain-canonical | P3 coordinator/journal and Gateway |
| `runtime_host_gateway_conversation_recovery.rs` | retain-canonical | P3 coordinator/journal and Gateway |
| `runtime_host_gateway_conversation_recovery_tests.rs` | retain-canonical | P3 coordinator/journal and Gateway |
| `runtime_host_gateway_conversation_scheduler_tests.rs` | retain-canonical | P3 coordinator/journal and Gateway |
| `runtime_host_gateway_conversation_selection.rs` | retain-canonical | P3 coordinator/journal and Gateway |
| `runtime_host_gateway_conversation_tests.rs` | retain-canonical | P3 coordinator/journal and Gateway |
| `runtime_host_gateway_events.rs` | retain-canonical | P4 distribution; retain authoritative P3 reader |
| `runtime_host_gateway_events_tests.rs` | retain-canonical | P4 distribution; retain authoritative P3 reader |
| `runtime_host_gateway_host.rs` | retain-canonical | B concrete workers and host router |
| `runtime_host_gateway_host_cancellation_tests.rs` | retain-canonical | B concrete workers and host router |
| `runtime_host_gateway_host_operation_tests.rs` | retain-canonical | B concrete workers and host router |
| `runtime_host_gateway_host_tests.rs` | retain-canonical | B concrete workers and host router |
| `runtime_host_gateway_management_tests.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_mount.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_mutation_events.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_mutation_events_tests.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_observation.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_pending_publication_tests.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_phy_lifecycle.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_phy_tests.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_project_lifecycle.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_project_tests.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_publication_tests.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_reconnect.rs` | retain-canonical | P4 distribution; retain authoritative P3 reader |
| `runtime_host_gateway_reconnect_tests.rs` | retain-canonical | P4 distribution; retain authoritative P3 reader |
| `runtime_host_gateway_semantic.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_semantic_mutation.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_semantic_provenance.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_semantic_search.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_semantic_tests.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_gateway_snapshot_reader.rs` | retain-canonical | P4 distribution; retain authoritative P3 reader |
| `runtime_host_gateway_snapshots.rs` | retain-canonical | P4 distribution; retain authoritative P3 reader |
| `runtime_host_gateway_snapshots_tests.rs` | retain-canonical | P4 distribution; retain authoritative P3 reader |
| `runtime_host_gateway_stream_registry.rs` | retain-canonical | P4 distribution; retain authoritative P3 reader |
| `runtime_host_gateway_stream_validation.rs` | retain-canonical | P4 distribution; retain authoritative P3 reader |
| `runtime_host_gateway_streams.rs` | retain-canonical | P4 distribution; retain authoritative P3 reader |
| `runtime_host_gateway_streams_recovery_tests.rs` | retain-canonical | P4 distribution; retain authoritative P3 reader |
| `runtime_host_gateway_streams_tests.rs` | retain-canonical | P4 distribution; retain authoritative P3 reader |
| `runtime_host_gateway_subscriptions.rs` | retain-canonical | P4 distribution; retain authoritative P3 reader |
| `runtime_host_gateway_tests.rs` | retain-canonical | Gateway/A exact-owner operations and admission |
| `runtime_host_insomnia.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_instance_tests.rs` | retain-canonical-tests | instance/routes/operational journal/codec protection |
| `runtime_host_instances.rs` | retain-relocate | verified instance authorization and instance-local selection |
| `runtime_host_interaction.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_knowledge.rs` | split-common | knowledge DTOs; delete old host mutations |
| `runtime_host_knowledge_memory.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_knowledge_relation.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_knowledge_tests.rs` | replace-legacy-tests | equivalent Gateway/REL/PHY scenarios; remove obsolete ambient fixtures |
| `runtime_host_legacy_isolation_tests.rs` | replace-legacy-tests | equivalent Gateway/REL/PHY scenarios; remove obsolete ambient fixtures |
| `runtime_host_lifecycle.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_memory_provenance.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_memory_search.rs` | split-common | bounded retrieval DTOs/scoring helpers; delete mixed-profile host methods |
| `runtime_host_mount.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_owner_access.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_owner_archive.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_owner_execution.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_owner_operation_journal.rs` | retain-relocate | REL attribution/publication operational journal and decoder |
| `runtime_host_owner_operation_journal_tests.rs` | retain-canonical-tests | instance/routes/operational journal/codec protection |
| `runtime_host_owner_publication.rs` | retain-relocate | canonical recoverable REL-to-PHY publication coordinator |
| `runtime_host_owner_publication_codec.rs` | retain-relocate | frozen publication encoding/decoding/verification |
| `runtime_host_owner_publication_codec_tests.rs` | retain-canonical-tests | instance/routes/operational journal/codec protection |
| `runtime_host_perception.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_perception_api.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_perception_flow.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_perception_owner.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_perception_queue.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_perception_reconciliation.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_perception_test_support.rs` | replace-legacy-tests | equivalent Gateway/REL/PHY scenarios; remove obsolete ambient fixtures |
| `runtime_host_perception_tests.rs` | replace-legacy-tests | equivalent Gateway/REL/PHY scenarios; remove obsolete ambient fixtures |
| `runtime_host_phy_dream.rs` | retain-canonical | B concrete workers and host router |
| `runtime_host_phy_execution.rs` | retain-canonical | B concrete workers and host router |
| `runtime_host_phy_execution_tests.rs` | retain-canonical | B concrete workers and host router |
| `runtime_host_phy_perception.rs` | retain-canonical | B concrete workers and host router |
| `runtime_host_phy_vectors.rs` | retain-canonical | B concrete workers and host router |
| `runtime_host_phylactery.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_project_environment.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_reconciliation.rs` | retain-maintenance | closed-owner reconciliation and reports; delete old host entry point |
| `runtime_host_reconciliation_tests.rs` | replace-legacy-tests | equivalent Gateway/REL/PHY scenarios; remove obsolete ambient fixtures |
| `runtime_host_rel_dream.rs` | retain-canonical | B concrete workers and host router |
| `runtime_host_rel_execution.rs` | retain-canonical | B concrete workers and host router |
| `runtime_host_rel_insomnia.rs` | retain-canonical | B concrete workers and host router |
| `runtime_host_rel_perception.rs` | retain-canonical | B concrete workers and host router |
| `runtime_host_rel_vectors.rs` | retain-canonical | B concrete workers and host router |
| `runtime_host_route_tests.rs` | retain-canonical-tests | instance/routes/operational journal/codec protection |
| `runtime_host_routes.rs` | retain-relocate | shared endpoint route configuration |
| `runtime_host_semantic_access.rs` | split-common | semantic owner DTOs; delete ambient host selection |
| `runtime_host_semantic_access_tests.rs` | replace-legacy-tests | equivalent Gateway/REL/PHY scenarios; remove obsolete ambient fixtures |
| `runtime_host_semantic_echo.rs` | split-common | Echo DTOs; delete host wrapper |
| `runtime_host_semantic_knowledge.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_semantic_provenance.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_session.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_session_tests.rs` | replace-legacy-tests | equivalent Gateway/REL/PHY scenarios; remove obsolete ambient fixtures |
| `runtime_host_status.rs` | split-common | background status DTO; delete old host sampling, retain independent eligibility tests |
| `runtime_host_test_support.rs` | split-test-support | retain test_path/endpoint fixtures used by canonical tests; remove old-host wait wrappers |
| `runtime_host_tests.rs` | replace-legacy-tests | equivalent Gateway/REL/PHY scenarios; remove obsolete ambient fixtures |
| `runtime_host_topology.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |
| `runtime_host_topology_tests.rs` | replace-legacy-tests | equivalent Gateway/REL/PHY scenarios; remove obsolete ambient fixtures |
| `runtime_host_vector_tests.rs` | replace-legacy-tests | equivalent Gateway/REL/PHY scenarios; remove obsolete ambient fixtures |
| `runtime_host_vectors.rs` | delete-legacy | A/B/E/F canonical owner APIs; no forwarding host |

Additional reverse caller: `src/semantic_graph/semantic_graph_runtime_host_tests.rs`, registered in `src/lib.rs`, requires a canonical A Graph-mutation replacement. Its exact concrete-host references are in the JSON root-reference inventory. All table rows inherit the persisted-touchpoint obligations above: tests have only fixture bytes; removed host wrappers retain their underlying Archive/Memory/Graph/file readers; canonical conversation/journal/worker/publication readers survive.

## Legacy field disposition

| Struct / field | Current type | Disposition |
| --- | --- | --- |
| `ReliquaryRuntimeHost.executions` | `BTreeMap<String, OwnerExecution>` | Delete old authority; retain canonical owner state |
| `ReliquaryRuntimeHost.legacy_rel_claims` | `BTreeMap<String, MountClaim>` | Delete old authority; retain canonical owner state |
| `ReliquaryRuntimeHost.legacy_phy_claim` | `Mutex<Option<MountClaim>>` | Delete old authority; retain canonical owner state |
| `ReliquaryRuntimeHost.active_key` | `Option<String>` | Delete old authority; retain canonical owner state |
| `ReliquaryRuntimeHost.routes` | `Arc<RwLock<ReliquaryRuntimeRoutes>>` | Remove old holder; retain shared configuration/synchronization support |
| `ReliquaryRuntimeHost.phylactery` | `Arc<Mutex<Option<Phylactery>>>` | Delete old authority; retain canonical owner state |
| `ReliquaryRuntimeHost.config` | `InsomniaWorkerConfig` | Remove old holder; retain shared configuration/synchronization support |
| `ReliquaryRuntimeHost.episode_policy` | `EpisodePolicy` | Remove old holder; retain shared configuration/synchronization support |
| `ReliquaryRuntimeHost.active_insomnia_enabled` | `AtomicBool` | Delete old authority; retain canonical owner state |
| `ReliquaryRuntimeHost.managed_sessions` | `Mutex<BTreeMap<String, LegacyManagedSessionState>>` | Delete old authority; retain canonical owner state |
| `LegacyManagedSessionState.active_session_id` | `Option<String>` | Delete old authority; retain canonical owner state |
| `LegacyManagedSessionState.reopened_session_pending` | `bool` | Delete old authority; retain canonical owner state |
| `Shared.runtime` | `Arc<Mutex<InteractionRuntime>>` | Delete old authority; retain canonical owner state |
| `Shared.signal` | `Arc<(Mutex<Control>, Condvar)>` | Remove old holder; retain shared configuration/synchronization support |
| `Shared.routes` | `Arc<RwLock<ReliquaryRuntimeRoutes>>` | Remove old holder; retain shared configuration/synchronization support |
| `Shared.phylactery` | `Arc<Mutex<Option<Phylactery>>>` | Delete old authority; retain canonical owner state |
| `Shared.phylactery_enabled` | `Arc<AtomicBool>` | Delete old authority; retain canonical owner state |
| `Shared.memory_profiles` | `Arc<Mutex<RuntimeMemoryProfiles>>` | Delete old authority; retain canonical owner state |
| `Shared.perception_queue` | `Arc<Mutex<perception_queue::PerceptionQueue>>` | Delete old authority; retain canonical owner state |
| `Shared.insomnia_enabled` | `Arc<AtomicBool>` | Delete old authority; retain canonical owner state |
| `Shared.insomnia_backpressure_until_ns` | `Arc<AtomicI64>` | Delete old authority; retain canonical owner state |
| `Shared.config` | `InsomniaWorkerConfig` | Remove old holder; retain shared configuration/synchronization support |
| `Shared.episode_policy` | `EpisodePolicy` | Remove old holder; retain shared configuration/synchronization support |
| `OwnerExecution.owner_id` | `Option<String>` | Delete old authority; retain canonical owner state |
| `OwnerExecution.runtime` | `Option<Arc<Mutex<InteractionRuntime>>>` | Delete old authority; retain canonical owner state |
| `OwnerExecution.signal` | `Arc<(Mutex<Control>, Condvar)>` | Remove old holder; retain shared configuration/synchronization support |
| `OwnerExecution.routes` | `Arc<RwLock<ReliquaryRuntimeRoutes>>` | Remove old holder; retain shared configuration/synchronization support |
| `OwnerExecution.phylactery` | `Arc<Mutex<Option<Phylactery>>>` | Delete old authority; retain canonical owner state |
| `OwnerExecution.phylactery_enabled` | `Arc<AtomicBool>` | Delete old authority; retain canonical owner state |
| `OwnerExecution.memory_profiles` | `Arc<Mutex<RuntimeMemoryProfiles>>` | Delete old authority; retain canonical owner state |
| `OwnerExecution.perception_queue` | `Arc<Mutex<perception_queue::PerceptionQueue>>` | Delete old authority; retain canonical owner state |
| `OwnerExecution.insomnia_enabled` | `Arc<AtomicBool>` | Delete old authority; retain canonical owner state |
| `OwnerExecution.insomnia_backpressure_until_ns` | `Arc<AtomicI64>` | Delete old authority; retain canonical owner state |
| `OwnerExecution.episode_policy` | `EpisodePolicy` | Remove old holder; retain shared configuration/synchronization support |
| `OwnerExecution.config` | `InsomniaWorkerConfig` | Remove old holder; retain shared configuration/synchronization support |
| `OwnerExecution.workers` | `Vec<JoinHandle<Result<(), ReliquaryRuntimeHostError>>>` | Delete old authority; retain canonical owner state |

## Legacy method disposition

All 170 public/private legacy-host method definitions, including Drop, follow the defining file disposition. Exact visibility/lines and conservative receiver-call candidates in Reliquary and each Warlock checkout are in the JSON. Same-name canonical calls are not old-host dependencies. Shared standalone/DTO helpers outside those implementations follow the split/maintenance rules above.

| Defining file | Old-host methods |
| --- | --- |
| `runtime_host_archive_search.rs` | `search_archive`, `search_archive_for`, `search_visible_archive`, `search_archive_owner`, `search_visible_archive_owner`, `search_archive_owner_inner`, `search_all_branches_for` |
| `runtime_host_files.rs` | `file_bytes`, `file_bytes_for` |
| `runtime_host_interaction.rs` | `with_phylactery`, `with_runtime`, `owner_id`, `phylactery_owner_id`, `conversation_summaries`, `conversation_transcript`, `conversation_transcript_page`, `conversation_branch_start_node_ids`, `set_conversation_title`, `set_conversation_active`, `clear_active_conversations`, `conversation_compactions`, `put_echo_event`, `echo_events`, `put_conversation_compaction`, `search_conversation_branch`, `search_open_session`, `live_search_vector`, `open_session`, `finalize_explicit_session`, `close_session`, `begin_message`, `correlate_project_revision`, `correlate_project_revision_with_management`, `latest_project_revision_correlation`, `register_project_file`, `project_file_ref`, `files_for_source`, `attach_project_file`, `append_text`, `append_checkpointed_text`, `interrupt_message`, `cancel_message`, `complete_message`, `complete_live_message`, `sync`, `insomnia_stats`, `memory_stats`, `memory_vector_stats`, `phylactery_memory_stats`, `phylactery_memory_vector_stats` |
| `runtime_host_knowledge.rs` | `read_reliquary_knowledge`, `read_reliquary_knowledge_for`, `read_phylactery_knowledge`, `create_reliquary_knowledge_memory`, `create_reliquary_knowledge_memory_for`, `create_phylactery_knowledge_memory`, `rename_reliquary_community`, `rename_reliquary_community_for`, `rename_phylactery_community` |
| `runtime_host_knowledge_memory.rs` | `replace_reliquary_knowledge_memory`, `replace_reliquary_knowledge_memory_for`, `replace_phylactery_knowledge_memory` |
| `runtime_host_knowledge_relation.rs` | `mutate_reliquary_knowledge_relation`, `mutate_reliquary_knowledge_relation_for`, `mutate_phylactery_knowledge_relation` |
| `runtime_host_lifecycle.rs` | `start`, `start_inactive`, `start_with_phylactery`, `new`, `start_inner`, `set_routes`, `wake`, `set_insomnia_enabled`, `into_cva`, `into_cva_and_phylactery`, `stop_workers`, `drop` |
| `runtime_host_memory_provenance.rs` | `read_reliquary_memory_provenance`, `read_reliquary_memory_provenance_for`, `resolve_external_memory_provenance`, `resolve_external_memory_provenance_for` |
| `runtime_host_memory_search.rs` | `search_visible_memories`, `retrieve_reliquary_memories`, `retrieve_reliquary_memories_for`, `retrieve_phylactery_memories`, `ensure_reliquary_embedding_profile`, `ensure_reliquary_embedding_profile_for`, `ensure_phylactery_memory_profile`, `runtime_profile_candidate`, `retrieve_memory_pair` |
| `runtime_host_mount.rs` | `mount_rel`, `unmount_rel`, `set_active_rel`, `activation_phy_state`, `deactivate_current` |
| `runtime_host_owner_access.rs` | `runtime_for_owner`, `with_runtime_for`, `wake_owner`, `embedding_route`, `clear_active_conversations_for`, `close_session_for`, `sync_for`, `latest_project_revision_correlation_for`, `correlate_project_revision_with_management_for` |
| `runtime_host_owner_archive.rs` | `conversation_summaries_for`, `conversation_transcript_for`, `echo_events_for`, `search_conversation_branch_for`, `files_for_source_for`, `project_file_ref_for`, `live_search_vector_for` |
| `runtime_host_perception_api.rs` | `entity_stats`, `phylactery_entity_stats` |
| `runtime_host_phylactery.rs` | `phylactery_profile`, `set_phylactery_profile`, `has_phylactery`, `attach_phylactery`, `detach_phylactery` |
| `runtime_host_project_environment.rs` | `create_project_environment_rel`, `inspect_project_adoption_source`, `adopt_project_environment_rel`, `validate_project_environment_cva`, `validate_project_environment_for`, `checkpoint_manual_project_for`, `project_repository_root_for`, `ingest_project_attachment`, `ingest_project_attachment_for`, `resolved_file_bytes_for` |
| `runtime_host_reconciliation.rs` | `reconcile_rel_file` |
| `runtime_host_semantic_access.rs` | `visible_semantic_owners`, `semantic_owner`, `visible_semantic_owner` |
| `runtime_host_semantic_echo.rs` | `read_echo_source_for` |
| `runtime_host_semantic_knowledge.rs` | `read_knowledge_for_owner`, `create_knowledge_memory_for_owner`, `replace_knowledge_memory_for_owner`, `mutate_knowledge_relation_for_owner`, `rename_knowledge_community_for_owner`, `knowledge_memory_for_owner` |
| `runtime_host_semantic_provenance.rs` | `read_memory_provenance_for_owner`, `resolve_knowledge_provenance_for_owner` |
| `runtime_host_session.rs` | `managed_session_state`, `update_managed_session_state`, `active_session_id`, `active_session_id_for`, `require_active_session`, `start_conversation_session`, `reopen_conversation_session`, `reopened_session_pending`, `mark_reopened_session_resumed`, `close_active_session`, `close_managed_session_for`, `persist_active_message`, `begin_active_assistant_stream`, `checkpoint_active_assistant_stream`, `interrupt_active_assistant_stream`, `complete_active_assistant_stream` |
| `runtime_host_status.rs` | `background_status` |
| `runtime_host_topology.rs` | `active_execution`, `execution_for`, `mounted_rel_ids`, `active_rel_id`, `rel_metadata_for`, `set_rel_metadata_for`, `dependency_closure`, `active_dependency_closure`, `mounted_dependency_graph`, `validate_candidate_topology`, `rel_metadata`, `set_rel_metadata` |

## Legacy test replacement map

Required D2/D3 routing follows. This is not a claim that every new scenario already proves equivalence. Exact test names and fixture callers are in the JSON; preserve common endpoint/path fixtures and decoder tests.

| Legacy test family | Canonical protecting surface | Required action |
| --- | --- | --- |
| `tests/backpressure/vector_tests` | `gateway_background_tests, gateway_pending_publication_tests, phy_execution_tests` | Port remaining inference failure/backpressure/idle/route cases |
| `perception_tests/perception_test_support` | `REL/PHY Perception and gateway semantic/publication tests` | Preserve Entity resolution/reconciliation and endpoint fixtures |
| `session_tests` | `gateway_conversation_* and accepted E tests` | Port resume/message/stream behavior; remove ambient cursor expectations |
| `topology_tests` | `gateway_ambient_extended_tests and gateway_project_tests` | Port closure/cycle/metadata cases with exact grants |
| `knowledge_tests/semantic_access_tests` | `gateway_semantic_tests and gateway_ambient*_tests` | Port mutation/provenance/Echo/privacy to A operations |
| `reconciliation_tests` | `gateway_project_lifecycle and Cva preservation tests` | Preserve sibling/conflict/vector recovery; remove host entry point |
| `legacy_isolation_tests` | `gateway_admission_tests and MountClaim tests` | Replace old-host collision with canonical alias/UUID exclusion |
| `semantic_graph_runtime_host_tests` | `gateway semantic mutation plus Graph owner tests` | Port Graph relation mutation to exact owner grants |
| `route_tests/instance_tests/owner journal and codec tests` | `same concrete responsibilities` | Retain and rehome imports |

## Completion evidence

D0 final source acceptance passed in `job_R_obYJy_NQzVeQ0AKKWrL_9e` (repeated after deduplicating shared alias evidence and compacting leaf rows): 873 Rust files, all 134 runtime-host modules, 170 old-host methods, ten host fields and the Shared/OwnerExecution/managed-session fields. The scanner passed 17 lexical/extraction/classification/multiline-call fixtures; exact local and three-checkout consumer baseline checks passed. An isolated new aliased caller correctly failed the drift check. The later hard-cut command correctly failed with 141 current legacy-authority occurrences, including the root, while the migrated C-close consumer had zero concrete legacy references. This expected rejection is not a completed deletion gate.

P2/P3 enforced ownership scans passed with no canonical violations. P4 passed 42 fixtures and classified 148 public APIs. Existing Pitlord policy validation passed. Documentation structural/change-impact and whitespace checks passed after the final evidence update. No production Rust/dependency/format changes occurred, so no new Cargo or graph-refresh acceptance is claimed. D0 uses source evidence and carries the existing fresh graph gap explicitly.

The first scanner runs exposed unclassified `test_support` and root `tests` suffixes. They were reviewed and classified explicitly; common test fixtures were split for retention rather than deleted. The final source and negative matrix above passed after those repairs. These source-check failures were not waived.

## Documentation impact

- Inspected: architecture, invariants, storage format, maintainer map, limitations, roadmap, API/coverage/contracts, D/next-wave plans, E/F/G/C records and shared standards.
- Updated: manifest, machine-readable inventory, D/next-wave status, index, maintainer/coverage routes and development guard instructions.
- Not affected: production Rust, persistent formats/readers, CLI behavior, dependency pins and current legacy API descriptions.
- Compliance check: structural and changed-from-origin/main documentation checks passed; whitespace checks passed.
- Known documentation gaps: D1–D4 require accepted N3/C/E/F; re-audit after integration.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: inventory only; existing canonical owners unchanged.
- Pitlord enforcement impact: policy unchanged; source drift/hard-cut guard supplements it.
- Other verification impact: lexical/extraction fixtures, inventory drift and expected hard-cut rejection; existing direct guards.
- Known architectural gaps: no fresh graph used; convergence/deletion behavior unaccepted.

## Related docs

- [Next execution plan](multiplexing-phase5-next-parallel-execution-plan-2026-10-07.md)
- [Architecture](architecture.md)
- [Storage format](storage-format.md)
- [Maintainer map](maintainer-map.md)

## Notes

D0 closes accountability for this source snapshot. It does not waive missing contracts or certify unknown out-of-tree consumers.
