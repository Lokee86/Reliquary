#!/usr/bin/env python3
"""Fast permanent guards for the exact Phase 7 capability cuts."""
import argparse
import sys
from pathlib import Path
import re

from check_owner_boundaries import strip_inactive_cfg
from check_multiplexing_phase4_coverage import masked

ROOT = Path(__file__).resolve().parents[1]
READ_POLICY = {
    "src/runtime_inference/dream_canonical.rs",
    "src/runtime_inference/dream_owner_candidates.rs",
    "src/runtime_inference/dream_owner_duplicate.rs",
    "src/runtime_inference/dream_owner_publisher.rs",
    "src/runtime_inference/dream_owner_lifecycle.rs",
    "src/runtime_inference/insomnia/evidence.rs",
    "src/retrieval/memory_retrieval_build.rs",
}
PROVIDER_POLICY = {"src/external_source/index.rs", "src/external_source/processing.rs"}
CONFIGURED_PROVIDER_BINDING_POLICY = {
    "src/facade/configured_runtime_insomnia.rs",
    "src/facade/configured_runtime_vectors.rs",
}
OFFLINE_FACADE_POLICY = {
    "src/facade/cva_lifecycle.rs", "src/facade/cva_repack.rs",
    "src/facade/cva_principal_repack.rs", "src/facade/historical_import_transform.rs",
    "src/facade/cva_reconcile_promote.rs",
}
OWNER_SERVICE_FACADE_POLICY = {
    "src/facade/insomnia_cva.rs",
    "src/facade/cva_communities.rs", "src/facade/phylactery_communities.rs",
    "src/facade/cva_memory_retrieval.rs", "src/facade/phylactery_memory_retrieval.rs",
    "src/facade/phylactery_memory_source.rs", "src/facade/external_source_extensions.rs",
    "src/facade/dream_product_extensions.rs", "src/facade/dream_pair_admission.rs",
    "src/facade/phylactery_dream_candidates.rs", "src/facade/cva_dream_population.rs",
}
CONTAINER_CONSUMER_POLICY = {
    "src/facade/cva_semantic_source.rs",
    "src/facade/cva_semantic_live.rs",
    "src/facade/cva_semantic_fragment.rs",
    "src/facade/historical_embedding.rs",
    "src/facade/historical_embedding_plan.rs",
    "src/facade/insomnia_cva.rs",
    "src/facade/insomnia_processor.rs",
    "src/runtime_inference/runtime_host_gateway_mutation_events.rs",
}
MEMORY_STORE_CONSUMER_POLICY = {
    "src/facade/cva_semantic_source.rs",
    "src/facade/cva_file_memory.rs",
    "src/facade/insomnia_cva.rs",
    "src/facade/interaction_background.rs",
}
PERCEPTION_CONSUMER_POLICY = {
    "src/facade/cva_import_readiness.rs",
    "src/facade/cva_import_completion.rs",
}
VECTOR_CONSUMER_POLICY = {"src/facade/cva_vector_recovery.rs"}
HISTORY_OBJECT_POLICY = {
    "src/history/import_epoch_scope.rs",
    "src/history/import_admission_epoch.rs",
}
MEMORY_KEY_POLICY = {
    "src/memory/memory_store.rs",
    "src/perception/entity_resolution_model.rs",
}
MEMORY_FIELD_POLICY = {
    "src/memory/memory_routing_model.rs",
    "src/perception/entity_resolution_codec.rs",
    "src/perception/entity_resolution_delta_codec.rs",
    "src/perception/entity_resolution_store.rs",
}
SEARCH_CONSUMER_POLICY = {"src/facade/cva_retrieval_memory_search.rs"}
RETRIEVAL_QUERY_BINDING_POLICY = {
    "src/facade/cva_retrieval_conversation_search.rs",
    "src/facade/cva_retrieval_lexical_search.rs",
    "src/facade/cva_retrieval_search.rs",
    "src/facade/cva_retrieval_semantic_search.rs",
}
SEARCH_RUNTIME_POLICY = {"src/runtime_inference/runtime_host_memory_search.rs"}
SEMANTIC_FRESHNESS_CALLER_POLICY = {
    "src/runtime_inference/public_vector_operations.rs",
    "src/runtime_inference/runtime_host_rel_vectors.rs",
    "src/runtime_inference/runtime_host_gateway_background.rs",
    "src/runtime_inference/runtime_host_gateway_ambient.rs",
}
EXTERNAL_VALIDATION_POLICY = {
    "src/runtime_inference/runtime_host_gateway_external.rs",
    "src/runtime_inference/runtime_host_gateway_external_processing.rs",
}
EXTERNAL_SOURCE_CONTRACT_POLICY = {"src/external_source/mod.rs"}
TRACE_CONSUMER_POLICY = {
    "src/runtime_inference/dream_settlement.rs",
    "src/runtime_inference/freshness_operations.rs",
    "src/facade/dream_freshness.rs",
}
TRACE_OWNER_POLICY = {"src/freshness/mod.rs"}
PURE_VALUE_CONTRACT_POLICY = {"src/pure_codec/memory_text_field.rs"}
PUBLIC_VALUE_API_POLICY = {"src/lib.rs"}
ENTITY_RESOLUTION_BINDING_POLICY = {"src/facade/cva_entity_resolution_engine.rs", "src/facade/cva_entity_resolver.rs"}
HISTORY_FRAME_POLICY = {"src/owner_lifecycle/history_source.rs", "src/container/container_scan.rs"}
DREAM_SOURCE_TIME_POLICY = {
    "src/runtime_inference/dream_owner_candidates.rs",
    "src/runtime_inference/dream_owner_duplicate.rs",
    "src/runtime_inference/dream_owner_publisher.rs",
    "src/runtime_inference/dream_publication_execution.rs",
}
DREAM_PUBLICATION_SEQUENCE_POLICY = {
    "src/runtime_inference/dream_owner_duplicate.rs",
    "src/runtime_inference/dream_publication_execution.rs",
}
SEMANTIC_TARGET_POLICY = {
    "src/owner_lifecycle/public_consumer_operations.rs",
    "src/runtime_inference/public_semantic_operations.rs",
}
SEMANTIC_OWNER_POLICY = {"src/owner_lifecycle/semantic_freshness_operations.rs"}
SEMANTIC_RUNTIME_POLICY = {"src/runtime_inference/public_semantic_operations.rs"}
HISTORICAL_SCOPE_POLICY = {"src/runtime_inference/public_owner_operations.rs"}
HISTORICAL_IMPORT_RUNTIME_POLICY = {"src/runtime_inference/historical_import_execution.rs"}
DREAM_PROCESSING_FACADE_POLICY = {
    "src/facade/dream_processing.rs": {
        "process_memory": "runtime_process_memory",
        "process_memory_with_pair_concurrency": "runtime_process_memory_with_pair_concurrency",
        "process_phylactery_memory": "runtime_process_phylactery_memory",
        "process_phylactery_memory_with_pair_concurrency": "runtime_process_phylactery_memory_with_pair_concurrency",
    },
    "src/facade/dream_processor_frontier.rs": {
        "process_memories_with_concurrency": "runtime_process_memories_with_concurrency",
        "process_phylactery_memories_with_concurrency": "runtime_process_phylactery_memories_with_concurrency",
    },
    "src/facade/dream_community_naming.rs": {
        "name_communities": "runtime_name_communities",
        "name_phylactery_communities": "runtime_name_phylactery_communities",
    },
}
DREAM_OWNER_FACADE_POLICY = {
    "src/facade/dream_lifecycle.rs",
    "src/facade/dream_product_extensions.rs",
    "src/facade/dream_processing.rs",
    "src/facade/dream_processor_frontier.rs",
    "src/facade/phylactery_dream_candidates.rs",
    "src/facade/phylactery_dream_lifecycle.rs",
}


def violations(path, source, facts):
    view = masked(strip_inactive_cfg(source, facts))
    errors = []
    if path in DREAM_PROCESSING_FACADE_POLICY:
        for method, runtime_method in DREAM_PROCESSING_FACADE_POLICY[path].items():
            match = re.search(
                rf"\bpub\s+fn\s+{re.escape(method)}\s*\([\s\S]*?\)\s*->[^{{]*\{{(?P<body>.*?)\n\s*\}}",
                view,
                re.S,
            )
            if not match:
                errors.append(f"{path}: supported Dream processing binding {method} disappeared")
                continue
            body = match["body"].strip()
            if not re.fullmatch(rf"self\s*\.\s*{runtime_method}\s*\([^{{}};]*\)", body, re.S):
                errors.append(f"{path}: {method} must only forward to {runtime_method}")
    if path in DREAM_OWNER_FACADE_POLICY and re.search(
        r"\bDreamGraphPublication\s*::\s*new\s*\(|\bMemoryOwnerWriter\s*::\s*new\s*\(|\bdream_lifecycle_write_parts\s*\(|\.\s*(?:publish_dream_pair|publish_knowledge_memory)\s*\(",
        view,
    ):
        errors.append(f"{path}: Dream writer construction and publication belong to native Runtime/owner lifecycle operations")
    if path == "src/runtime_inference/public_dream_operations.rs":
        phy_publication = re.search(
            r"impl\s+crate\s*::\s*phy_execution_owner\s*::\s*PhyExecutionOwner\s*\{[\s\S]*?fn\s+native_publish_dream_pair\s*\([^{}]*\)\s*->[^{}]*\{(?P<body>.*?)\n\s*\}",
            view,
            re.S,
        )
        if not phy_publication or not re.search(r"DreamGraphPublication\s*::\s*new\s*\(", phy_publication["body"]):
            errors.append(f"{path}: native PHY Dream publication must retain its owner-qualified publication operation")
        phy_lifecycle = re.search(
            r"impl\s+crate\s*::\s*phy_execution_owner\s*::\s*PhyExecutionOwner\s*\{[\s\S]*?dream_lifecycle_write_parts\s*\(\)",
            view,
            re.S,
        )
        if not phy_lifecycle:
            errors.append(f"{path}: native PHY Dream lifecycle must use its owner-qualified writer operation")
    if path in HISTORICAL_IMPORT_RUNTIME_POLICY:
        if re.search(r"\b(?:crate\s*::\s*)?cva_freshness\s*::|\brecord_dream_evidence_trace\s*\(", view):
            errors.append(f"{path}: historical Import tracing must stay in the Import-owned operation")
        if re.search(r"\b(?:ImportEpochStore|FreshnessStore|HistorySource|HistoryReadAccess)\s*::\s*(?:new|open|reopen|decode|from_bytes)\s*\(", view):
            errors.append(f"{path}: historical Import store construction and raw History access belong to Import/owner lifecycle")
        if re.search(r"\bImportEpochStore\b|\bFreshnessStore\b|\bHistorySource\b|\bHistoryReadAccess\b", view):
            errors.append(f"{path}: Runtime must not expose Import store or raw History implementation types")
        if re.search(r"\bself\s*\.\s*owner\s*\.\s*(?:import_epochs|freshness|dream_freshness|container|vector_generations|compatibility_profiles)\s*\.", view):
            errors.append(f"{path}: historical Import must not access mounted stores directly from Runtime")
        if re.search(r"\bself\s*\.\s*owner\s*\.\s*(?:history_read_access|historical_freshness_read_access|historical_vector_read_access|historical_settlement_write_access|install_historical_settlement)\s*\(", view):
            errors.append(f"{path}: historical Import owner reads and publication must pass through Import/owner lifecycle operations")
    if path in EXTERNAL_VALIDATION_POLICY and re.search(r"\.\s*external_index\s*\.\s*read_state\s*\(", view):
        errors.append(f"{path}: live External index reads belong to owner lifecycle")
    if path == "src/runtime_inference/reliquary_runtime_owner.rs":
        if re.search(r"\.\s*freshness_due\s*\.\s*(?:drain_due|advance|defer)\s*\(", view):
            errors.append(f"{path}: due crossing acceptance belongs to the lifecycle operation")
    if path == "src/perception/entity_merge.rs":
        if re.search(r"\bcrate\s*::\s*(?:owner_lifecycle|graph_endpoint_admission)\s*::", view):
            errors.append(f"{path}: Perception plans must not call lifecycle publication admission")
    if path in {"src/facade/historical_embedding.rs", "src/facade/historical_embedding_plan.rs"}:
        if re.search(r"\b(?:verify_endpoint|validate_embedding_batch|cosine_similarity|fragment_text_bounded)\s*\(|\.\s*(?:dimensions|normalization|embed|embed_bounded|supports_bounded_embedding)\s*\(|\.\s*(?:vector_generations|archive_vectors|packed_vectors)\s*\.\s*(?:generations|info)\s*\(|\.\s*archive\s*\.\s*(?:nodes|fragments)\b", view):
            errors.append(f"{path}: historical embedding must use runtime provider and owner read operations")
    if path in {"src/imports/reconciliation_replay.rs", "src/imports/owner_completion.rs", "src/imports/owner_historical.rs"}:
        if re.search(r"\.\s*(?:vector_generations|entities|relationships|graph|insomnia)\s*\.\s*(?:vector_version|entity_version|relationship_version|graph_version|memory_graph_version|stats|queue|work)\s*\(", view):
            errors.append(f"{path}: Import must consume lifecycle-owned version and work facts")
    if path in CONFIGURED_PROVIDER_BINDING_POLICY:
        if re.search(r"\b(?:ConfiguredGeneralEndpoint|OpenAiReadyEmbeddingEndpoint|InsomniaExtractor)\s*::|\.\s*(?:embed|with_batching|from_switchboard|from_insomnia_switchboard|insomnia_metadata|entity_extraction)\s*\(", view):
            errors.append(f"{path}: configured provider preparation must stay in Runtime")
    if path in RETRIEVAL_QUERY_BINDING_POLICY:
        if re.search(r"\bself\s*\.\s*(?:owner|archive|container|lexical_index|vector_generations|compatibility_profiles)\b|\b(?:ArchiveReadView|MemoryReadView|VectorReadView|MemoryLexicalReadView)\s*::\s*new\s*\(|\bcrate\s*::\s*(?:lexical_search|conversation_search|semantic_search|search_rank|search)\s*::|\.\s*embed\s*\(", view):
            errors.append(f"{path}: retrieval public binding bypasses its canonical runtime operation")
    if path in SEMANTIC_FRESHNESS_CALLER_POLICY:
        if re.search(r"\.\s*(?:process_semantic_freshness|semantic_freshness_pending_fragments)\s*\(", view):
            errors.append(f"{path}: semantic Freshness must invoke its canonical runtime operation")
    if path in {"src/runtime_inference/freshness_operations.rs", "src/runtime_inference/runtime_freshness_birth.rs"}:
        if re.search(r"\.\s*freshness\s*\.\s*(?:record_at_cut|record_at|records_at|records|contains_event|event_matches|prepare_event_postimages_for|validate_event_payload|encode_event|persist_policy_if_needed|ingest_appended|replay_entries|event_receipt)\s*\(", view):
            errors.append(f"{path}: Freshness store operations must pass through owner lifecycle")
    if path in {
        "src/facade/cva_ego.rs", "src/facade/phylactery_ego.rs",
        "src/facade/cva_relationship_owner.rs", "src/facade/cva_conversation_tombstone.rs",
        "src/facade/cva_graph.rs", "src/facade/phylactery_graph.rs",
        "src/facade/insomnia_processor.rs",
    }:
        if re.search(r"\bself\s*\.\s*(?:owner|graph|ego|relationships|container|memories|entities)\b", view):
            errors.append(f"{path}: public binding must invoke the canonical runtime operation")
    if path in {
        "src/runtime_inference/public_vector_operations.rs",
        "src/runtime_inference/reliquary_runtime_owner.rs",
    }:
        if re.search(r"\bself\s*\.\s*owner\s*\.\s*(?:packed_vectors|memory_vectors|archive_vectors|vector_generations|compatibility_profiles)\s*\.", view):
            errors.append(f"{path}: vector store calls require concrete owner contracts")
    if path in DREAM_SOURCE_TIME_POLICY:
        if re.search(r"\b(?:Fn|FnMut|FnOnce)\b|\bdyn\s+Fn", view):
            errors.append(f"{path}: Dream source-time facts must not cross as callback capabilities")
        if re.search(r"\bsource_time\s*(?:=|:)\s*[\s\S]{0,100}?\|\s*memory\b", view):
            errors.append(f"{path}: Dream source-time resolution must consume a requested Memory value, not a closure")
    if path in DREAM_PUBLICATION_SEQUENCE_POLICY:
        if re.search(r"\.\s*current_ids\s*\(", view):
            errors.append(f"{path}: duplicate timestamp sequence must not eagerly scan all Memories")
        if re.search(r"(?:Vec|HashMap|BTreeMap)\s*<\s*Memory\s*>|collect\s*::\s*<\s*Vec\s*<\s*Memory\s*>", view):
            errors.append(f"{path}: publication must resolve source time from one bounded Memory request at a time")
    if path == "src/runtime_inference/dream_owner_duplicate.rs":
        if not re.search(r"enum\s+\w*(?:Phase|Progress|Step)\b", view):
            errors.append(f"{path}: duplicate source-time flow must retain an explicit resumable phase")
        if not re.search(r"(?:SourceTime|Request)\s*\(\s*Memory\s*\)", view):
            errors.append(f"{path}: duplicate source-time phase must request a concrete Memory value")
        if not re.search(r"fn\s+resume\s*\([\s\S]{0,300}?MemoryId[\s\S]{0,200}?Option\s*<\s*i64\s*>", view):
            errors.append(f"{path}: duplicate source-time flow must resume with passive ID and timestamp values")
        if not re.search(r"filter\s*\(\s*\|relation\|[^)]*DuplicateOf[^)]*relation\.active", view, re.S):
            errors.append(f"{path}: duplicate index rebuild must be bounded to active DuplicateOf endpoints")
    if path == "src/owner_lifecycle/public_consumer_operations.rs":
        target = re.search(r"struct\s+SemanticMemoryTarget\s*\{(?P<body>[^}]+)\}", view, re.S)
        if not target or re.search(r"\bMemory\b|(?:content|title|payload|body)\s*:", target["body"]):
            errors.append(f"{path}: semantic target snapshot must contain only IDs/body coordinates")
        if re.search(r"memories\s*\.\s*memory\s*\(", view):
            errors.append(f"{path}: semantic target enumeration must not hydrate Memory bodies")
    if path == "src/runtime_inference/public_semantic_operations.rs":
        if "search_eligible_semantic_targets" in view:
            hydration_count = len(re.findall(r"semantic_freshness_operations\s*::\s*memory\s*\(", view))
            if hydration_count != 2:
                errors.append(f"{path}: semantic search must retain its two bounded Memory hydration passes")
        if re.search(r"semantic_memory_targets\s*\([^;]*\)\s*;[\s\S]{0,500}?semantic_freshness_operations\s*::\s*memory", view):
            # This adjacency rule catches turning the owner target snapshot into hydrated rows.
            if not re.search(r"for\s+target\s+in\s*&memory_targets[\s\S]{0,240}?semantic_freshness_operations\s*::\s*memory", view):
                errors.append(f"{path}: hydrate Memory only in the existing per-target source passes")
    if path in HISTORICAL_SCOPE_POLICY:
        if re.search(r"runtime_with_historical_embedding_write_scope|impl\s+Fn(?:Once|Mut)?", view):
            errors.append(f"{path}: historical write permission must not be forwarded through a caller closure")
        if not re.search(r"impl\s+Drop\s+for\s+HistoricalEmbeddingWriteScope[\s\S]{0,300}?set_historical_embedding_write_scope\s*\(\s*false\s*\)", view):
            errors.append(f"{path}: historical embedding write scope must reset permission on Drop")
    if path in OWNER_SERVICE_FACADE_POLICY:
        if re.search(r"\bself\s*\.\s*(?:owner|memories|graph|communities|container|dream_pairs|processing_epochs|memory_vectors|packed_vectors)\b|\bcrate\s*::\s*(?:memory_store|memory_vector_store|dream_owner_candidates|dream_cooldown)\b", view):
            errors.append(f"{path}: public owner/service binding bypasses native runtime operation")
    if path in CONTAINER_CONSUMER_POLICY:
        if re.search(
            r"\.\s*container\s*\.\s*(?:append|sync|path|owner_id|backing_metadata|set_owner_write_scope)\s*\(",
            view,
        ):
            errors.append(f"{path}: consumer calls a physical Container API directly")
    if path in MEMORY_STORE_CONSUMER_POLICY:
        if re.search(
            r"\.\s*memories\s*\.\s*(?:current_ids|memory|current_body_id|contains_memory)\s*\(|"
            r"owner_lifecycle\s*::\s*public_consumer_operations\s*::\s*semantic_memory_targets\s*\(|"
            r"\.\s*owner\s*\.\s*link_file_to_memory\s*\(",
            view,
        ):
            errors.append(f"{path}: public consumer reads or mutates MemoryStore directly")
    if path in SEMANTIC_RUNTIME_POLICY:
        if re.search(r"\.\s*(?:freshness|container|memories|memory_vectors|packed_vectors|archive)\s*\.", view):
            errors.append(f"{path}: semantic Gateway operation bypasses owner-lifecycle read/write contracts")
    if path == "src/runtime_inference/phy_execution_owner.rs":
        if re.search(r"\.\s*entity_resolutions\s*\.\s*(?:validate_local|records)\s*\(", view):
            errors.append(f"{path}: PHY open validation must use the ordered owner-lifecycle admission contract")
    if path == "src/owner_lifecycle/entity_resolution_open_admission.rs":
        local = re.search(r"resolutions\s*\.\s*validate_local\s*\(\s*\)\s*\?", view)
        references = re.search(r"validate_entity_resolution_references\s*\(", view)
        records = re.search(r"resolutions\s*\.\s*records\s*\(\s*\)", view)
        if not (local and references and records and local.start() < references.start() < records.start()):
            errors.append(f"{path}: validate local resolution state before materializing records for cross-store admission")
    if path == "src/runtime_inference/perception_entity_merge.rs":
        if re.search(
            r"\bowner\s*\.\s*owner\s*\.\s*(?:entities\s*\.\s*(?:publish|publish_with_global_id|tombstone_direct|retired_replacement|retention_descriptor|lifecycle_state|entity_revision|entity|candidates_for_surface|candidates_for_normalized_surface|candidates_for_alias_surface|creation_conflicts|entity_version|current|stats)|entity_resolutions\s*\.\s*retarget_resolved)\s*\(",
            view,
        ):
            errors.append(f"{path}: Gateway must route EntityStore operations through owner-lifecycle contracts")
    if path == "src/owner_lifecycle/entity_publication_access.rs":
        validation = re.search(r"validate_entity_publication\s*\(", view)
        publish = re.search(r"entities\s*\.\s*publish\s*\(", view)
        if not (validation and publish and validation.start() < publish.start()):
            errors.append(f"{path}: preserve Perception admission validation before Entity publication")
    if path in SEMANTIC_OWNER_POLICY:
        if re.search(r"\b(?:cosine_similarity|plan_local_effects|SemanticCandidate)\b", view):
            errors.append(f"{path}: owner-lifecycle semantic facts must not own scoring or candidate policy")
    if path in PERCEPTION_CONSUMER_POLICY:
        if re.search(
            r"(?:perception_import_readiness\s*::\s*ImportReadiness\s*::\s*new|\.\s*owner\s*\.\s*perception_import_readiness)\s*\(",
            view,
        ):
            errors.append(f"{path}: public consumer constructs the Perception readiness view directly")
    if path in VECTOR_CONSUMER_POLICY:
        if re.search(
            r"vector_publication\s*::\s*(?:prepare_batch|prepare_memory_batch|accept_profile)\s*\(",
            view,
        ):
            errors.append(f"{path}: public consumer invokes Vectors publication internals directly")
    if path in OFFLINE_FACADE_POLICY:
        if re.search(r"\bContainer\s*::|\.\s*container\s*\.|\bcrate\s*::\s*(?:archive_codec|turn_ingest_codec)\b", view):
            errors.append(f"{path}: public offline binding performs physical or codec implementation work")
    if path in READ_POLICY:
        if re.search(r"\b(?:Container|RelExecutionOwner|PhyExecutionOwner|Cva|Phylactery|GatewayRuntime|InstanceRuntime)\b", view):
            errors.append(f"{path}: lower policy receives physical or product composition capability")
    if path == "src/runtime_inference/dream_canonical.rs" and re.search(r"\btrait\s+DreamOwnerRead\b", view):
        errors.append(f"{path}: Dream policy restores an upper owner callback port")
    if path in PROVIDER_POLICY:
        if re.search(r"\b(?:EmbeddingEndpoint|GeneralEndpoint|TemporalInferencer|InsomniaExtractor|EmbeddingMode)\b|\bcomplete_json\s*\(", view):
            errors.append(f"{path}: External Sources invokes provider orchestration")
    if path == "src/imports/reconciliation_interval.rs":
        if re.search(r"\.insomnia\s*\.\s*apply_(?:work|attempt)\s*\(", view):
            errors.append(f"{path}: Import replay mutates InsomniaStore directly")
        if re.search(r"crate\s*::\s*freshness_dream_journal\s*::|replay_coordinate_requests\s*\(|DreamReplayCoordinates\s*::\s*default", view):
            errors.append(f"{path}: Import replay reaches DreamStore instead of the Dream transport protocol")
        if re.search(r"\.communities\s*\.\s*publish\s*\(|\.dream_freshness\s*\.\s*ingest(?:_materialized_begin)?\s*\(|\.entity_resolutions\s*\.\s*persist_change\s*\(", view):
            errors.append(f"{path}: Import reconciliation applies owner store state directly")
    if path == "src/runtime_inference/dream_replay_transport.rs":
        if re.search(r"impl\s+Fn(?:Mut|Once)?\b|\|\s*\w+\s*\|", view):
            errors.append(f"{path}: Dream replay transport accepts Import-owned callback capabilities")
    if path == "src/runtime_inference/insomnia/extraction.rs":
        if re.search(r"stage_reporter|&\s*dyn\s+Fn\s*\(\s*InsomniaSemanticStage", view):
            errors.append(f"{path}: Insomnia extraction accepts a Gateway reporting callback")
    if path == "src/runtime_inference/insomnia_worker_runtime.rs":
        if re.search(r"let\s+stage_reporter\s*=\s*\||&stage_reporter", view):
            errors.append(f"{path}: Gateway passes a reporting callback into Insomnia extraction")
        if "sync_channel" not in view or "InsomniaSemanticProgress" not in view:
            errors.append(f"{path}: Gateway does not consume bounded Insomnia stage values")
    if path == "src/owner_lifecycle/insomnia_import_replay.rs":
        if re.search(r"\bfn\s+decode_record\s*\(", view):
            errors.append(f"{path}: obsolete Insomnia replay decode forwarder remains")
    if path.startswith("src/owner_lifecycle/") and path.endswith("reliquary.rs"):
        if re.search(r"\b(?:LexicalIndex|DuplicateIndex|GatewayRuntime|RelExecutionOwner)\b", view):
            errors.append(f"{path}: durable owner contains runtime/service state")
    if path == "src/owner_lifecycle/phylactery.rs":
        if re.search(r"\b(?:LexicalIndex|DuplicateIndex|GatewayRuntime|PhyExecutionOwner)\b", view):
            errors.append(f"{path}: durable owner contains runtime/service state")
    if path in HISTORY_OBJECT_POLICY and re.search(r"\.\s*legacy_bytes\s*\(", view):
        errors.append(f"{path}: History reads Container ObjectRef encoding instead of owner-lifecycle coordinates")
    if path in HISTORY_OBJECT_POLICY and not re.search(r"frame\s*\.\s*offset", view):
        errors.append(f"{path}: History must consume the owner-lifecycle physical offset value")
    if path == "src/history/import_epoch_scope.rs" and not re.search(r"frame\s*\.\s*end", view):
        errors.append(f"{path}: epoch scope must consume the accepted frame end value")
    if path in HISTORY_FRAME_POLICY:
        if not re.search(r"pub(?:\s*\(crate\))?\s+offset\s*:\s*u64", view):
            errors.append(f"{path}: physical offset must be carried as an owner-lifecycle scalar")
    if path == "src/memory/memory_store.rs" and re.search(
        r"MemoryEntityMentionKey\s*::\s*new\s*\(", view
    ):
        errors.append(f"{path}: MemoryStore calls a Perception-owned mention-key constructor")
    if path == "src/perception/entity_resolution_model.rs":
        if not re.search(
            r"pub\s+type\s+MemoryEntityMentionKey\s*=\s*crate\s*::\s*memory_text_field_contract\s*::\s*MemoryEntityMentionKey\s*<\s*MemoryId\s*>\s*;",
            view,
        ):
            errors.append(f"{path}: mention-key is no longer a pure-codec generic value contract")
        if not re.search(
            r"pub\s+fn\s+new\s*\(\s*memory_id\s*:\s*MemoryId\s*,\s*mention\s*:\s*&crate\s*::\s*MemoryEntityMention\s*\)\s*->\s*Self\s*\{\s*Self\s*::\s*from_parts\s*\(",
            view,
        ):
            errors.append(f"{path}: public mention-key constructor contract or pure-codec delegation changed")
    if path in MEMORY_FIELD_POLICY and path != "src/memory/memory_routing_model.rs":
        if re.search(r"(?:pub\s+)?enum\s+MemoryTextField\b", view):
            errors.append(f"{path}: duplicates the shared MemoryTextField value contract")
    if path == "src/memory/memory_routing_model.rs":
        if re.search(r"(?:pub\s+)?enum\s+MemoryTextField\b", view):
            errors.append(f"{path}: MemoryTextField is declared outside the pure-codec contract owner")
    if path in SEARCH_CONSUMER_POLICY:
        if re.search(r"\b(?:memory_search\s*::\s*validate|validate)\s*\(", view):
            errors.append(f"{path}: public search binding owns Retrieval validation instead of delegating to Runtime")
        if not re.search(r"runtime_host\s*::\s*memory_search\s*::\s*search_memories_(?:rel|phy)\s*\(", view):
            errors.append(f"{path}: public search binding does not delegate to the canonical Runtime operation")
        for product in ("Cva", "Phylactery"):
            if not re.search(rf"impl\s+{product}\s*\{{[^}}]*pub\s+fn\s+search_memories\s*\(\s*&mut self\s*,\s*query\s*:\s*&str\s*,\s*limit\s*:\s*usize\s*,?\s*\)\s*->\s*Result\s*<\s*Vec\s*<\s*MemoryLexicalSearchHit\s*>\s*,\s*SearchError\s*>", view, re.S):
                errors.append(f"{path}: public {product} search signature changed")
    if path in SEARCH_RUNTIME_POLICY:
        for function in ("search_memories_rel", "search_memories_phy"):
            match = re.search(rf"fn\s+{function}\s*\([^{{]+\{{(?P<body>.*?)\n\}}", view, re.S)
            if not match or not re.search(r"validate\s*\(\s*query\s*,\s*limit\s*\)\s*\?", match["body"]):
                errors.append(f"{path}: {function} must validate query and limit before retrieval")
            else:
                validate_at = match["body"].find("validate(")
                candidates_at = match["body"].find("memory_lexical_candidates(")
                if candidates_at < 0 or validate_at > candidates_at:
                    errors.append(f"{path}: {function} reads candidates before validating its public input")
                if "query.trim()" not in match["body"] or "memory(hit.memory_id)" not in match["body"]:
                    errors.append(f"{path}: {function} changed query normalization or hit hydration")
    if path in EXTERNAL_VALIDATION_POLICY:
        if re.search(r"external_index\s*::\s*model\s*::\s*identifier\s*\(", view):
            errors.append(f"{path}: Gateway validates External Index model identifiers directly")
        if re.search(r"\b(?:descriptor|message|body)\s*\.\s*validate\s*\(|reference\s*\.\s*message\s*\.\s*validate\s*\(", view):
            errors.append(f"{path}: Gateway invokes an External Index model validator directly")
    if path in EXTERNAL_SOURCE_CONTRACT_POLICY:
        required = {
            "validate_identifier": r"external_index\s*::\s*model\s*::\s*identifier\s*\(\s*value\s*\)",
            "validate_reference": r"value\s*\.\s*validate\s*\(\s*\)",
            "validate_descriptor": r"value\s*\.\s*validate\s*\(\s*namespace\s*\)",
            "validate_hydrated": r"value\s*\.\s*validate\s*\(\s*expected\s*\)",
        }
        for name, body in required.items():
            match = re.search(rf"pub\s*\(crate\)\s+fn\s+{name}\s*\([^{{]+\{{(?P<body>.*?)\n\}}", view, re.S)
            if not match:
                errors.append(f"{path}: External Sources no longer owns {name}")
            elif not re.search(body, match["body"]):
                errors.append(f"{path}: {name} no longer preserves owner validation behavior")
    if path in TRACE_CONSUMER_POLICY and re.search(
        r"freshness_trace\s*::\s*(?:scope|measure|record|flush)\s*\(", view
    ):
        errors.append(f"{path}: caller bypasses Freshness-owned operation trace policy")
    if path in TRACE_CONSUMER_POLICY and path != "src/facade/dream_freshness.rs":
        if re.search(r"freshness\s*::\s*operation_trace_scope\s*\(\s*\"", view):
            errors.append(f"{path}: trace operation names must come from the finite Freshness enum")
        if re.search(r"freshness\s*::\s*measure_trace_stage\s*\(\s*\"", view):
            errors.append(f"{path}: trace stage names must come from the finite Freshness enum")
    if path in TRACE_OWNER_POLICY:
        if not re.search(r"enum\s+TraceOperation\b", view) or not re.search(r"enum\s+TraceStage\b", view):
            errors.append(f"{path}: Freshness must own finite operation and stage diagnostic vocabularies")
        if not re.search(r"struct\s+TraceScope\b", view):
            errors.append(f"{path}: Freshness must wrap the trace-store scope capability")
        if not re.search(r"fn\s+trace_stage\s*\(\s*stage\s*:\s*TraceStage\s*\)\s*->\s*TraceStageGuard", view):
            errors.append(f"{path}: Freshness stage timing must use a callback-free RAII guard")
        if re.search(r"measure_trace_stage|trace_stage\s*\([^)]*FnOnce|impl\s+FnOnce", view):
            errors.append(f"{path}: Freshness must not forward caller closures into the trace store")
        if 'feature="freshness-performance-trace"' in facts and not re.search(r"std\s*::\s*thread\s*::\s*panicking\s*\(\)", view):
            errors.append(f"{path}: stage timing must preserve panic behavior by skipping unwind samples")
    if path in TRACE_CONSUMER_POLICY and path != "src/facade/dream_freshness.rs":
        if re.search(r"freshness\s*::\s*measure_trace_stage\s*\(", view):
            errors.append(f"{path}: callback-based trace measurement restores a lower-to-upper call port")
        if re.search(r"freshness\s*::\s*trace_stage\s*\([^;]*,\s*\|\|", view, re.S):
            errors.append(f"{path}: trace stage accepts a caller closure instead of timing the local scope")
    if path in PURE_VALUE_CONTRACT_POLICY:
        if re.search(r"\b(?:MemoryId|MemoryEntityMention)\b", view):
            errors.append(f"{path}: pure-codec value contract depends on a domain model")
        if not re.search(r"Title\s*=>\s*1", view) or not re.search(r"Content\s*=>\s*2", view):
            errors.append(f"{path}: stable MemoryTextField tags changed")
        if not re.search(r"1\s*=>\s*Some\s*\(\s*Self\s*::\s*Title\s*\)", view) or not re.search(
            r"2\s*=>\s*Some\s*\(\s*Self\s*::\s*Content\s*\)", view
        ):
            errors.append(f"{path}: stable MemoryTextField tag decoding changed")
        if not re.search(r"pub\s+struct\s+MemoryEntityMentionKey\s*<\s*T\s*>", view):
            errors.append(f"{path}: mention key is no longer a generic passive value")
        if not re.search(r"pub\s+fn\s+from_parts\s*\(", view):
            errors.append(f"{path}: pure mention-key constructor disappeared")
    if path == "src/facade/dream_freshness.rs" and 'feature="freshness-performance-trace"' in facts:
        if not re.search(r"pub\s+fn\s+freshness_trace_flush\s*\(&self\)\s*\{\s*crate\s*::\s*freshness_operations_runtime\s*::\s*flush_freshness_trace\s*\(\)\s*;\s*\}", view):
            errors.append(f"{path}: public trace flush must remain a facade binding over the runtime operation")
    if path == "src/runtime_inference/freshness_operations.rs" and re.search(r"pub\s+fn\s+freshness_trace_flush\b", view):
        errors.append(f"{path}: Runtime must not define a public product extension")
    if path in ENTITY_RESOLUTION_BINDING_POLICY:
        engine = path.endswith("cva_entity_resolution_engine.rs")
        name = "EntityResolutionEngine" if engine else "EntityResolver"
        core = "EntityResolutionEngineCore" if engine else "EntityResolverCore"
        required_functions = (
            (
                "gateway_entity_resolution_engine_new",
                "gateway_entity_resolution_engine_with_decision_endpoint",
                "gateway_entity_resolution_engine_decision_model",
                "gateway_entity_resolution_engine_fallback_model",
            )
            if engine
            else (
                "gateway_entity_resolver_new",
                "gateway_entity_resolver_with_system_prompt",
                "gateway_entity_resolver_model",
            )
        )
        if not re.search(rf"pub\s+struct\s+{name}\s*<\s*E\s*>\s*\{{[^}}]*pub\s*\(crate\)\s+core\s*:\s*{core}\s*<\s*E\s*>", view, re.S):
            errors.append(f"{path}: facade must retain the concrete public {name}<E> binding and crate-visible core field")
        for function in required_functions:
            if f"crate::public_entity_resolution_bindings::{function}" not in view:
                errors.append(f"{path}: public EntityResolution method must delegate to Gateway function {function}")
        if re.search(r"\b(?:EntityResolutionEngineCore|EntityResolverCore)\s*::\s*(?:new|with_decision_endpoint|decision_model|fallback_model|with_system_prompt|model)\s*\(", view) or re.search(r"self\s*\.\s*core\s*\.\s*(?:with_decision_endpoint|decision_model|fallback_model|model)\s*\(", view):
            errors.append(f"{path}: public EntityResolution binding must not invoke Perception core methods")
        if re.search(r"pub\s+use\s+crate\s*::\s*public_entity_resolution_bindings", view):
            errors.append(f"{path}: public EntityResolution struct must be defined in the facade")
        if re.search(r"\bpub\s+type\s+Entity(?:ResolutionEngine|Resolver)\s*<", view):
            errors.append(f"{path}: public EntityResolution binding must be a concrete struct, not an alias")
    if path == "src/runtime_inference/public_entity_resolution_bindings.rs":
        if re.search(r"\bpub\s+struct\s+Entity(?:ResolutionEngine|Resolver)\s*<", view):
            errors.append(f"{path}: Gateway must not define public EntityResolution product structs")
        signatures = {
            "gateway_entity_resolution_engine_new": r"fallback\s*:\s*E\s*,?\s*\)\s*->\s*EntityResolutionEngineCore\s*<\s*E\s*>",
            "gateway_entity_resolution_engine_with_decision_endpoint": r"core\s*:\s*EntityResolutionEngineCore\s*<\s*E\s*>\s*,\s*decision\s*:\s*Option\s*<\s*Arc\s*<\s*dyn\s+DecisionEndpoint\s*>\s*>\s*,?\s*\)\s*->\s*EntityResolutionEngineCore\s*<\s*E\s*>",
            "gateway_entity_resolution_engine_decision_model": r"core\s*:\s*&\s*EntityResolutionEngineCore\s*<\s*E\s*>\s*,?\s*\)\s*->\s*Option\s*<\s*&str\s*>",
            "gateway_entity_resolution_engine_fallback_model": r"core\s*:\s*&\s*EntityResolutionEngineCore\s*<\s*E\s*>\s*,?\s*\)\s*->\s*&str",
            "gateway_entity_resolver_new": r"endpoint\s*:\s*E\s*,?\s*\)\s*->\s*EntityResolverCore\s*<\s*E\s*>",
            "gateway_entity_resolver_with_system_prompt": r"endpoint\s*:\s*E\s*,\s*system_prompt\s*:\s*impl\s+Into\s*<\s*String\s*>\s*,?\s*\)\s*->\s*EntityResolverCore\s*<\s*E\s*>",
            "gateway_entity_resolver_model": r"core\s*:\s*&\s*EntityResolverCore\s*<\s*E\s*>\s*,?\s*\)\s*->\s*&str",
        }
        for function, signature in signatures.items():
            if not re.search(rf"pub\s*\(crate\)\s+fn\s+{function}\s*<\s*E\s*:\s*GeneralEndpoint\s*>\s*\([^)]*{signature}", view, re.S):
                errors.append(f"{path}: Gateway function {function} disappeared or changed its core contract")
        if not re.search(r"Option\s*<\s*Arc\s*<\s*dyn\s+DecisionEndpoint\s*>\s*>", view):
            errors.append(f"{path}: Gateway function must preserve the DecisionEndpoint trait-object API")
    if path in PUBLIC_VALUE_API_POLICY:
        if not re.search(r"pub\s+use\s+entity_resolution_model\s*::\s*\{[^}]*\bMemoryEntityMentionKey\b", view, re.S):
            errors.append(f"{path}: public MemoryEntityMentionKey root export disappeared")
        if not re.search(r"pub\s+use\s+memory_text_field_contract\s*::\s*MemoryTextField\s*;", view):
            errors.append(f"{path}: public MemoryTextField root export disappeared")
    return errors


def main():
    import json
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--files", nargs="*", help="changed-source paths; default is all guarded seams")
    args = parser.parse_args()
    mapping = json.loads((ROOT / "tools/pitlord/production-components.json").read_text())
    facts = set(mapping["cfg_profile"]["rustc_cfg_facts"])
    paths = sorted(READ_POLICY | PROVIDER_POLICY | CONFIGURED_PROVIDER_BINDING_POLICY | OFFLINE_FACADE_POLICY | OWNER_SERVICE_FACADE_POLICY | CONTAINER_CONSUMER_POLICY | MEMORY_STORE_CONSUMER_POLICY | PERCEPTION_CONSUMER_POLICY | VECTOR_CONSUMER_POLICY | HISTORY_OBJECT_POLICY | MEMORY_KEY_POLICY | MEMORY_FIELD_POLICY | SEARCH_CONSUMER_POLICY | RETRIEVAL_QUERY_BINDING_POLICY | SEARCH_RUNTIME_POLICY | SEMANTIC_FRESHNESS_CALLER_POLICY | EXTERNAL_VALIDATION_POLICY | EXTERNAL_SOURCE_CONTRACT_POLICY | TRACE_CONSUMER_POLICY | TRACE_OWNER_POLICY | PURE_VALUE_CONTRACT_POLICY | PUBLIC_VALUE_API_POLICY | HISTORY_FRAME_POLICY | DREAM_SOURCE_TIME_POLICY | SEMANTIC_TARGET_POLICY | SEMANTIC_OWNER_POLICY | SEMANTIC_RUNTIME_POLICY | HISTORICAL_SCOPE_POLICY | HISTORICAL_IMPORT_RUNTIME_POLICY | ENTITY_RESOLUTION_BINDING_POLICY | set(DREAM_PROCESSING_FACADE_POLICY) | DREAM_OWNER_FACADE_POLICY | {
        "src/runtime_inference/public_dream_operations.rs",
        "src/owner_lifecycle/reliquary.rs",
        "src/owner_lifecycle/phylactery.rs",
        "src/imports/reconciliation_interval.rs",
        "src/runtime_inference/dream_replay_transport.rs",
        "src/runtime_inference/insomnia/extraction.rs",
        "src/runtime_inference/insomnia_worker_runtime.rs",
        "src/owner_lifecycle/insomnia_import_replay.rs",
        "src/runtime_inference/public_semantic_operations.rs",
        "src/owner_lifecycle/semantic_freshness_operations.rs",
        "src/runtime_inference/public_entity_resolution_bindings.rs",
        "src/runtime_inference/phy_execution_owner.rs",
        "src/owner_lifecycle/entity_resolution_open_admission.rs",
        "src/runtime_inference/perception_entity_merge.rs",
        "src/owner_lifecycle/entity_publication_access.rs",
        "src/facade/cva_ego.rs", "src/facade/phylactery_ego.rs",
        "src/facade/cva_relationship_owner.rs", "src/facade/cva_conversation_tombstone.rs",
        "src/facade/cva_graph.rs", "src/facade/phylactery_graph.rs",
        "src/runtime_inference/public_vector_operations.rs",
        "src/runtime_inference/reliquary_runtime_owner.rs",
        "src/runtime_inference/runtime_freshness_birth.rs",
        "src/imports/reconciliation_replay.rs", "src/imports/owner_completion.rs", "src/imports/owner_historical.rs",
        "src/perception/entity_merge.rs",
    })
    if args.files is not None:
        paths = sorted(set(paths) & {p.replace("\\", "/") for p in args.files})
    errors = []
    mapped = {row["path"]: row["component"] for row in mapping.get("files", [])}
    if mapped.get("src/pure_codec/memory_text_field.rs") != "pure-codec":
        errors.append("src/pure_codec/memory_text_field.rs: pure value contract must remain owned by pure-codec")
    for path in paths:
        if not (ROOT / path).is_file():
            errors.append(f"{path}: guarded production seam missing")
        else:
            errors.extend(violations(path, (ROOT / path).read_text(encoding="utf-8"), facts))
    print("\n".join(errors) if errors else f"Phase 7 source cuts: {len(paths)} guarded seams passed")
    return int(bool(errors))


if __name__ == "__main__":
    sys.exit(main())
