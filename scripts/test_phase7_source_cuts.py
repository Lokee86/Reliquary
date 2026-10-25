import sys
import re
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_phase7_source_cuts import ROOT, violations
from check_owner_boundaries import strip_inactive_cfg


class SourceCutTests(unittest.TestCase):
    def test_gateway_external_reads_cannot_bypass_lifecycle(self):
        for path in ("src/runtime_inference/runtime_host_gateway_external.rs", "src/runtime_inference/runtime_host_gateway_external_processing.rs"):
            self.assertTrue(violations(path, "runtime.cva.external_index.read_state(namespace);", {"windows"}))
            self.assertFalse(violations(path, "crate::external_owner_access::read_state(index, namespace);", {"windows"}))

    def test_due_acceptance_cannot_return_to_runtime_store_calls(self):
        path = "src/runtime_inference/reliquary_runtime_owner.rs"
        self.assertTrue(violations(path, "self.freshness_due.drain_due(turn, 1);", {"windows"}))
        self.assertTrue(violations(path, "self.freshness_due.advance(due);", {"windows"}))
        self.assertFalse(violations(path, "crate::owner_lifecycle::freshness_operations::accept_next_due_crossing(dispatcher, turn);", {"windows"}))

    def test_perception_plans_cannot_invoke_publication_admission(self):
        path = "src/perception/entity_merge.rs"
        self.assertTrue(violations(path, "crate::owner_lifecycle::validate_live_relationship_refs(owner, memories, entities, draft);", {"windows"}))
        self.assertTrue(violations(path, "crate::graph_endpoint_admission::validate_semantic_changes(memories, entities, changes, origin);", {"windows"}))
        self.assertFalse(violations(path, "plan.relationships.push(write);", {"windows"}))

    def test_historical_embedding_bypasses_remain_rejected(self):
        for path in ("src/facade/historical_embedding.rs", "src/facade/historical_embedding_plan.rs"):
            for code in ("endpoint.embed_bounded(mode, inputs, cap);", "cva.archive.fragments.len();", "owner.packed_vectors.info(id);", "verify_endpoint(profile, endpoint);"):
                self.assertTrue(violations(path, code, {"windows"}), (path, code))
            self.assertFalse(violations(path, "cva.runtime_historical_embedding_batch(endpoint, cancel, report, limits, mode, inputs);", {"windows"}))

    def test_import_version_facts_cannot_reach_owner_stores(self):
        for path in ("src/imports/reconciliation_replay.rs", "src/imports/owner_completion.rs", "src/imports/owner_historical.rs"):
            self.assertTrue(violations(path, "owner.vector_generations.vector_version();", {"windows"}))
            self.assertTrue(violations(path, "owner.insomnia.work(id);", {"windows"}))
            self.assertFalse(violations(path, "crate::owner_lifecycle::import_reconciliation_restore::import_watermarks(owner);", {"windows"}))

    def test_configured_provider_bindings_cannot_construct_endpoints(self):
        for path in ("src/facade/configured_runtime_insomnia.rs", "src/facade/configured_runtime_vectors.rs"):
            self.assertTrue(violations(path, "OpenAiReadyEmbeddingEndpoint::from_switchboard(switchboard);", {"windows"}))
            self.assertTrue(violations(path, "InsomniaExtractor::new(endpoint);", {"windows"}))
            self.assertFalse(violations(path, "super::provider_operations::prepare_insomnia(switchboard, 16, 1, false);", {"windows"}))

    def test_community_bindings_cannot_reach_graph_stores(self):
        for path in ("src/facade/cva_communities.rs", "src/facade/phylactery_communities.rs"):
            self.assertTrue(violations(path, "self.communities.latest_lineage();", {"windows"}))
            self.assertTrue(violations(path, "self.owner.communities.publish(container, graph, owner, snapshot);", {"windows"}))
            self.assertFalse(violations(path, "self.runtime_refresh_communities_leiden();", {"windows"}))

    def test_retrieval_query_bindings_cannot_reintroduce_live_views(self):
        for path in (
            "src/facade/cva_retrieval_conversation_search.rs",
            "src/facade/cva_retrieval_lexical_search.rs",
            "src/facade/cva_retrieval_search.rs",
            "src/facade/cva_retrieval_semantic_search.rs",
        ):
            for code in ("ArchiveReadView::new(archive, container);", "self.owner.vector_generations.current_generation(id);", "crate::lexical_search::lexical_candidates(index, archive, query, limit);", "endpoint.embed(mode, texts);"):
                self.assertTrue(violations(path, code, {"windows"}), (path, code))
            self.assertFalse(violations(path, "self.runtime_semantic_search(profile, endpoint, query, limit);", {"windows"}))

    def test_runtime_freshness_store_access_requires_owner_operation(self):
        for path in ("src/runtime_inference/freshness_operations.rs", "src/runtime_inference/runtime_freshness_birth.rs"):
            self.assertTrue(violations(path, "self.owner.freshness.record_at(id, turn);", {"windows"}))
            self.assertTrue(violations(path, "owner.freshness.encode_event(owner_id, id, turn, proof, effects, records);", {"windows"}))
            self.assertFalse(violations(path, "self.owner.freshness_record_at(id, turn);", {"windows"}))

    def test_runtime_semantic_freshness_cannot_return_to_public_bindings(self):
        for path in (
            "src/runtime_inference/public_vector_operations.rs",
            "src/runtime_inference/runtime_host_rel_vectors.rs",
            "src/runtime_inference/runtime_host_gateway_background.rs",
            "src/runtime_inference/runtime_host_gateway_ambient.rs",
        ):
            self.assertTrue(violations(path, "owner.process_semantic_freshness(id, 64);", {"windows"}))
            self.assertTrue(violations(path, "owner.semantic_freshness_pending_fragments();", {"windows"}))
            self.assertFalse(violations(path, "owner.runtime_process_semantic_freshness(id, 64); owner.runtime_semantic_freshness_pending_fragments();", {"windows"}))

    def test_new_public_bindings_cannot_reach_live_owner_fields(self):
        for path in ("src/facade/insomnia_cva.rs", "src/facade/cva_ego.rs", "src/facade/cva_relationship_owner.rs", "src/facade/cva_conversation_tombstone.rs", "src/facade/cva_graph.rs", "src/facade/phylactery_graph.rs", "src/facade/insomnia_processor.rs"):
            self.assertTrue(violations(path, "self.owner.graph.active_relations();", {"windows"}), path)
            self.assertFalse(violations(path, "self.native_graph_graph_stats();", {"windows"}), path)

    def test_gateway_vector_contract_rejects_store_bypass(self):
        for path in ("src/runtime_inference/public_vector_operations.rs", "src/runtime_inference/reliquary_runtime_owner.rs"):
            self.assertTrue(violations(path, "self.owner.packed_vectors.put(&mut self.owner.container, packed);", {"windows"}), path)
            self.assertTrue(violations(path, "self.owner.compatibility_profiles.get_active(id);", {"windows"}), path)
            self.assertFalse(violations(path, "self.owner.vector_profile_get_active(id);", {"windows"}), path)

    def test_entity_resolution_facades_define_concrete_gateway_backed_bindings(self):
        engine = "pub struct EntityResolutionEngine<E> { pub(crate) core: EntityResolutionEngineCore<E> } impl<E: GeneralEndpoint> EntityResolutionEngine<E> { pub fn new(fallback: E) -> Self { Self { core: crate::public_entity_resolution_bindings::gateway_entity_resolution_engine_new(fallback) } } pub fn with_decision_endpoint(self, decision: Option<Arc<dyn DecisionEndpoint>>) { crate::public_entity_resolution_bindings::gateway_entity_resolution_engine_with_decision_endpoint(self.core, decision); } pub fn decision_model(&self) -> Option<&str> { crate::public_entity_resolution_bindings::gateway_entity_resolution_engine_decision_model(&self.core) } pub fn fallback_model(&self) -> &str { crate::public_entity_resolution_bindings::gateway_entity_resolution_engine_fallback_model(&self.core) } }"
        resolver = "pub struct EntityResolver<E> { pub(crate) core: EntityResolverCore<E> } impl<E: GeneralEndpoint> EntityResolver<E> { pub fn new(endpoint: E) -> Self { Self { core: crate::public_entity_resolution_bindings::gateway_entity_resolver_new(endpoint) } } pub fn with_system_prompt(endpoint: E, prompt: impl Into<String>) { crate::public_entity_resolution_bindings::gateway_entity_resolver_with_system_prompt(endpoint, prompt); } pub fn model(&self) -> &str { crate::public_entity_resolution_bindings::gateway_entity_resolver_model(&self.core) } }"
        self.assertFalse(violations("src/facade/cva_entity_resolution_engine.rs", engine, {"windows"}))
        self.assertFalse(violations("src/facade/cva_entity_resolver.rs", resolver, {"windows"}))

    def test_entity_resolution_facades_cannot_call_perception_core(self):
        for path, code in (
            ("src/facade/cva_entity_resolution_engine.rs", "EntityResolutionEngineCore::new(fallback); self.core.fallback_model();"),
            ("src/facade/cva_entity_resolver.rs", "EntityResolverCore::with_system_prompt(endpoint, prompt); self.core.model();"),
        ):
            self.assertTrue(violations(path, code, {"windows"}))
        self.assertTrue(violations(
            "src/facade/cva_entity_resolution_engine.rs",
            "fn model(&self) { self.core.fallback_model(); }",
            {"windows"},
        ))

    def test_entity_resolution_facades_cannot_alias_gateway_types(self):
        self.assertTrue(violations(
            "src/facade/cva_entity_resolver.rs",
            "pub type EntityResolver<E> = EntityResolverCore<E>;",
            {"windows"},
        ))

    def test_gateway_owns_concrete_entity_resolution_bindings(self):
        code = "pub(crate) fn gateway_entity_resolution_engine_new<E: GeneralEndpoint>(fallback: E) -> EntityResolutionEngineCore<E> { EntityResolutionEngineCore::new(fallback) } pub(crate) fn gateway_entity_resolution_engine_with_decision_endpoint<E: GeneralEndpoint>(core: EntityResolutionEngineCore<E>, decision: Option<Arc<dyn DecisionEndpoint>>) -> EntityResolutionEngineCore<E> { core.with_decision_endpoint(decision) } pub(crate) fn gateway_entity_resolution_engine_decision_model<E: GeneralEndpoint>(core: &EntityResolutionEngineCore<E>) -> Option<&str> { core.decision_model() } pub(crate) fn gateway_entity_resolution_engine_fallback_model<E: GeneralEndpoint>(core: &EntityResolutionEngineCore<E>) -> &str { core.fallback_model() } pub(crate) fn gateway_entity_resolver_new<E: GeneralEndpoint>(endpoint: E) -> EntityResolverCore<E> { EntityResolverCore::new(endpoint) } pub(crate) fn gateway_entity_resolver_with_system_prompt<E: GeneralEndpoint>(endpoint: E, system_prompt: impl Into<String>) -> EntityResolverCore<E> { EntityResolverCore::with_system_prompt(endpoint,system_prompt) } pub(crate) fn gateway_entity_resolver_model<E: GeneralEndpoint>(core: &EntityResolverCore<E>) -> &str { core.model() }"
        self.assertFalse(violations("src/runtime_inference/public_entity_resolution_bindings.rs", code, {"windows"}))
        self.assertTrue(violations("src/runtime_inference/public_entity_resolution_bindings.rs", "pub struct EntityResolver<E> { core: EntityResolverCore<E> }", {"windows"}))

    def test_public_retrieval_cannot_construct_private_readers(self):
        self.assertTrue(violations("src/facade/cva_memory_retrieval.rs", "fn build() { MemoryReadView::new(&self.memories,&self.container); }", {"windows"}))

    def test_public_dream_cannot_reach_owner_store(self):
        self.assertTrue(violations("src/facade/dream_pair_admission.rs", "fn put() { self.owner.dream_pairs.put(payload); }", {"windows"}))

    def test_public_dream_delegates_to_native_operation(self):
        self.assertFalse(violations("src/facade/dream_pair_admission.rs", "fn put() { self.native_mark_dream_pair_evaluated(left,right) }", {"windows"}))

    def test_offline_public_binding_cannot_create_physical_owner(self):
        self.assertTrue(violations("src/facade/cva_lifecycle.rs", "fn create() { Container::create(path); }", {"windows"}))

    def test_offline_public_binding_cannot_publish_or_decode(self):
        for code in ("fn repack() { self.container.append(payload); }", "use crate::archive_codec::decode_record;"):
            self.assertTrue(violations("src/facade/cva_repack.rs", code, {"windows"}))

    def test_offline_public_binding_delegates_to_maintenance(self):
        self.assertFalse(violations("src/facade/cva_repack.rs", "fn repack() { self.owner.repack_project_backed_attachments(path) }", {"windows"}))

    def test_physical_capability_negative(self):
        self.assertTrue(violations("src/runtime_inference/dream_owner_candidates.rs", "fn read(owner: &mut Container) {}", {"windows"}))

    def test_runtime_callback_negative(self):
        self.assertTrue(violations("src/runtime_inference/dream_owner_lifecycle.rs", "fn apply(gateway: &GatewayRuntime) {}", {"windows"}))

    def test_dream_owner_callback_negative(self):
        self.assertTrue(violations("src/runtime_inference/dream_canonical.rs", "trait DreamOwnerRead { fn same_publication_cohort(&self); }", {"windows"}))

    def test_provider_negative(self):
        self.assertTrue(violations("src/external_source/index.rs", "fn run<E: EmbeddingEndpoint>(e: E) {}", {"windows"}))

    def test_owner_cache_negative(self):
        self.assertTrue(violations("src/owner_lifecycle/reliquary.rs", "struct Owner { cache: LexicalIndex }", {"windows"}))

    def test_concrete_owner_reader_positive(self):
        self.assertFalse(violations("src/runtime_inference/dream_owner_candidates.rs", "fn read(owner: &MemoryReadView) {}", {"windows"}))

    def test_import_cannot_apply_insomnia_records_directly(self):
        self.assertTrue(violations(
            "src/imports/reconciliation_interval.rs",
            "output.insomnia.apply_work(work); output.insomnia.apply_attempt(attempt);",
            {"windows"},
        ))

    def test_insomnia_replay_cannot_restore_decode_forwarder(self):
        self.assertTrue(violations(
            "src/owner_lifecycle/insomnia_import_replay.rs",
            "pub(crate) fn decode_record(payload: &[u8]) {}",
            {"windows"},
        ))

    def test_cfg_inactive_and_comments_positive(self):
        self.assertFalse(violations("src/external_source/index.rs", "// GeneralEndpoint\n#[cfg(test)] fn f(p: &GeneralEndpoint) {}", {"windows"}))

    def test_public_container_capability_calls_are_rejected(self):
        cases = [
            ("src/facade/cva_semantic_source.rs", "self.container.append(payload); self.container.sync();"),
            ("src/facade/historical_embedding.rs", "self.container.path(); self.container.sync(); self.container.set_owner_write_scope(true);"),
            ("src/facade/historical_embedding_plan.rs", "cva.container.path(); cva.container.sync();"),
            ("src/facade/insomnia_cva.rs", "self.container.append(payload); self.container.sync();"),
            ("src/facade/insomnia_processor.rs", "self.container.owner_id();"),
            ("src/runtime_inference/runtime_host_gateway_mutation_events.rs", "runtime.cva.container.backing_metadata();"),
        ]
        for path, code in cases:
            with self.subTest(path=path):
                self.assertTrue(violations(path, code, {"windows"}))

    def test_public_container_capability_delegation_positive(self):
        cases = [
            ("src/facade/cva_semantic_source.rs", "self.runtime_append_semantic_source(owner, payload);"),
            ("src/facade/historical_embedding.rs", "self.runtime_historical_backing_facts(); self.runtime_sync_public_owner(); self.runtime_with_historical_embedding_write_scope(operation);"),
            ("src/facade/historical_embedding_plan.rs", "self.runtime_historical_backing_length(); self.runtime_sync_public_owner();"),
            ("src/facade/insomnia_cva.rs", "self.runtime_complete_insomnia_episode(args);"),
            ("src/facade/insomnia_processor.rs", "self.owner_id();"),
            ("src/runtime_inference/runtime_host_gateway_mutation_events.rs", "runtime.cva.owner.backing_len();"),
        ]
        for path, code in cases:
            with self.subTest(path=path):
                self.assertFalse(violations(path, code, {"windows"}))

    def test_public_memory_store_access_is_rejected(self):
        cases = [
            ("src/facade/cva_semantic_source.rs", "self.memories.current_ids(); self.owner.memories.memory(container, id); self.owner.memories.current_body_id(id);"),
            ("src/facade/cva_semantic_source.rs", "owner_lifecycle::public_consumer_operations::semantic_memory_targets(owner);"),
            ("src/facade/cva_file_memory.rs", "self.owner.memories.current_body_id(id); self.owner.link_file_to_memory(file, id);"),
            ("src/facade/insomnia_cva.rs", "self.memories.contains_memory(id);"),
            ("src/facade/interaction_background.rs", "self.memories.current_ids();"),
        ]
        for path, code in cases:
            with self.subTest(path=path):
                self.assertTrue(violations(path, code, {"windows"}))

    def test_public_memory_owner_operation_delegation_positive(self):
        for path, code in (
            ("src/facade/cva_semantic_source.rs", "self.runtime_semantic_memory_targets(profile);"),
            ("src/facade/cva_file_memory.rs", "self.runtime_link_file_to_memory(file, id);"),
            ("src/facade/insomnia_cva.rs", "self.runtime_complete_insomnia_episode(args);"),
            ("src/facade/interaction_background.rs", "self.runtime_memory_ids();"),
        ):
            with self.subTest(path=path):
                self.assertFalse(violations(path, code, {"windows"}))

    def test_public_perception_and_vector_internals_are_rejected(self):
        self.assertTrue(violations(
            "src/facade/cva_import_readiness.rs",
            "perception_import_readiness::ImportReadiness::new(&owner);",
            {"windows"},
        ))
        self.assertTrue(violations(
            "src/facade/cva_import_completion.rs",
            "self.owner.perception_import_readiness();",
            {"windows"},
        ))
        self.assertTrue(violations(
            "src/facade/cva_vector_recovery.rs",
            "vector_publication::prepare_batch(owner); owner_lifecycle::vector_publication::prepare_memory_batch(owner);",
            {"windows"},
        ))

    def test_public_perception_and_vector_owner_routes_positive(self):
        self.assertFalse(violations(
            "src/facade/cva_import_readiness.rs",
            "self.runtime_perception_import_readiness();",
            {"windows"},
        ))
        self.assertFalse(violations(
            "src/facade/cva_vector_recovery.rs",
            "self.runtime_rebuild_derived_vectors(endpoint);",
            {"windows"},
        ))

    def test_history_cannot_reintroduce_object_ref_byte_access(self):
        for path in (
            "src/history/import_epoch_scope.rs",
            "src/history/import_admission_epoch.rs",
        ):
            with self.subTest(path=path):
                self.assertTrue(violations(path, "frame.object.legacy_bytes();", {"windows"}))
                self.assertFalse(violations(path, "let offset = frame.offset; let end = frame.end;", {"windows"}))

    def test_pure_value_contract_rejects_domain_ownership_or_tag_drift(self):
        good = """
            pub enum MemoryTextField { Title, Content }
            impl MemoryTextField {
                fn tag(self) -> u8 { match self { Self::Title => 1, Self::Content => 2 } }
                fn from_tag(tag: u8) -> Option<Self> { match tag { 1 => Some(Self::Title), 2 => Some(Self::Content), _ => None } }
            }
            pub struct MemoryEntityMentionKey<T> { pub memory_id: T, pub field: MemoryTextField, pub start_byte: u32, pub end_byte: u32 }
            impl<T> MemoryEntityMentionKey<T> { pub fn from_parts(memory_id: T, field: MemoryTextField, start_byte: u32, end_byte: u32) -> Self { Self { memory_id, field, start_byte, end_byte } } }
        """
        self.assertFalse(violations("src/pure_codec/memory_text_field.rs", good, {"windows"}))
        for bad in (
            good.replace("Self::Content => 2", "Self::Content => 3"),
            good.replace("MemoryEntityMentionKey<T>", "MemoryEntityMentionKey<MemoryId>"),
            good.replace("pub fn from_parts", "pub fn new"),
        ):
            with self.subTest(bad=bad):
                self.assertTrue(violations("src/pure_codec/memory_text_field.rs", bad, {"windows"}))

    def test_memory_key_keeps_public_constructor_but_store_uses_pure_contract(self):
        self.assertTrue(violations(
            "src/memory/memory_store.rs",
            "MemoryEntityMentionKey::new(memory_id, mention);",
            {"windows"},
        ))
        self.assertFalse(violations(
            "src/memory/memory_store.rs",
            "MemoryEntityMentionKey::from_parts(memory_id, mention.field, mention.start_byte, mention.end_byte);",
            {"windows"},
        ))
        model = (ROOT / "src/perception/entity_resolution_model.rs").read_text(encoding="utf-8")
        self.assertFalse(violations("src/perception/entity_resolution_model.rs", model, {"windows"}))

    def test_public_search_validation_stays_in_runtime_before_reads(self):
        facade = (ROOT / "src/facade/cva_retrieval_memory_search.rs").read_text(encoding="utf-8")
        runtime = (ROOT / "src/runtime_inference/runtime_host_memory_search.rs").read_text(encoding="utf-8")
        self.assertFalse(violations("src/facade/cva_retrieval_memory_search.rs", facade, {"windows"}))
        self.assertFalse(violations("src/runtime_inference/runtime_host_memory_search.rs", runtime, {"windows"}))
        self.assertTrue(violations(
            "src/facade/cva_retrieval_memory_search.rs",
            "crate::memory_search::validate(query, limit)?;",
            {"windows"},
        ))
        self.assertTrue(violations(
            "src/runtime_inference/runtime_host_memory_search.rs",
            "fn search_memories_rel(cva: &mut Cva, query: &str, limit: usize) { let _ = cva.memory_lexical_candidates(query, limit); validate(query, limit)?; }",
            {"windows"},
        ))

    def test_gateway_routes_external_validation_through_external_sources(self):
        for path, code in (
            ("src/runtime_inference/runtime_host_gateway_external.rs", "descriptor.validate(&state.namespace)?;"),
            ("src/runtime_inference/runtime_host_gateway_external_processing.rs", "crate::external_index::model::identifier(scope)?;"),
            ("src/runtime_inference/runtime_host_gateway_external_processing.rs", "message.validate(descriptor)?;"),
            ("src/runtime_inference/runtime_host_gateway_external_processing.rs", "reference.message.validate()?;"),
        ):
            with self.subTest(path=path, code=code):
                self.assertTrue(violations(path, code, {"windows"}))
        ext_source = (ROOT / "src/external_source/mod.rs").read_text(encoding="utf-8")
        self.assertFalse(violations("src/external_source/mod.rs", ext_source, {"windows"}))

    def test_trace_bypass_is_rejected_in_both_cfg_profiles(self):
        profiles = (
            {"windows"},
            {"windows", 'feature="freshness-performance-trace"'},
        )
        self.assertTrue(violations(
            "src/imports/reconciliation_interval.rs",
            "let requests = crate::freshness_dream_journal::DreamFreshnessJournal::replay_coordinate_requests(payload);",
            {"windows"},
        ))
        self.assertFalse(violations(
            "src/imports/reconciliation_interval.rs",
            "let plan = crate::dream_replay_transport::prepare_rebase(journal, payload); fill_plan(plan);",
            {"windows"},
        ))
        for facts in profiles:
            for path in (
                "src/runtime_inference/dream_settlement.rs",
                "src/runtime_inference/freshness_operations.rs",
                "src/facade/dream_freshness.rs",
            ):
                with self.subTest(path=path, facts=facts):
                    self.assertFalse(violations(path, (ROOT / path).read_text(encoding="utf-8"), facts))
                    gated = '#[cfg(feature = "freshness-performance-trace")] fn t() { crate::freshness_trace::scope("bypass"); }'
                    direct = 'fn t() { crate::freshness_trace::scope("bypass"); }'
                    if 'feature="freshness-performance-trace"' in facts:
                        self.assertTrue(violations(path, gated, facts))
                    else:
                        self.assertFalse(violations(path, gated, facts))

                    self.assertTrue(violations(path, direct, facts))
        trace = (ROOT / "src/freshness/trace.rs").read_text(encoding="utf-8")
        default = strip_inactive_cfg(trace, {"windows"})
        enabled = strip_inactive_cfg(
            trace, {"windows", 'feature="freshness-performance-trace"'}
        )
        self.assertRegex(default, r"fn\s+measure<T>[^\n]+\n\s*operation\(\)")
        self.assertRegex(default, r"fn\s+scope\([^)]*\)\s*->\s*Scope\s*\{\s*Scope\s*\}")
        self.assertIn("thread_local!", enabled)
        self.assertIn("pub(crate) use enabled::{Scope, enabled, flush, measure, record, scope};", enabled)
        freshness = (ROOT / "src/freshness/mod.rs").read_text(encoding="utf-8")
        self.assertFalse(violations("src/freshness/mod.rs", freshness, {"windows"}))
        self.assertIn("std::thread::panicking()", freshness)
        self.assertIn("freshness_trace::record", freshness)
        self.assertIn("enabled().then(std::time::Instant::now)", freshness)
        self.assertRegex(
            freshness,
            re.compile(
                r"if\s*!std::thread::panicking\(\)\s*&&\s*let Some\(started\)\s*=\s*self\.started\s*\{.*?freshness_trace::record",
                re.S,
            ),
        )
        stage_impl = freshness.split("impl TraceStage {", 1)[1].split("pub(crate) struct TraceScope", 1)[0]
        for stage in (
            "accepted_use.receipt_validation",
            "accepted_use.root_collection",
            "accepted_use.exact_retry_lookup",
            "accepted_use.direct_graph_materialization",
            "accepted_use.adjacency_construction",
            "accepted_use.community_proof_pinning",
            "accepted_use.community_membership_materialization",
            "accepted_use.propagation",
            "accepted_use.effect_postimage_preparation",
            "accepted_use.container_append_sync",
            "dream.propagation",
            "dream.durable_historical_graph_scan",
            "dream.root_collection_persistence",
            "dream.duplicate_root_resolution",
            "dream.historical_graph_snapshot",
            "dream.receipt_validation_postimage_append_sync_due_refresh",
        ):
            self.assertIn(f'"{stage}"', stage_impl)
        self.assertNotIn("impl FnOnce", freshness)
        self.assertTrue(violations(
            "src/freshness/mod.rs",
            "pub(crate) fn trace_stage(stage: TraceStage, operation: impl FnOnce()) {}",
            {"windows"},
        ))
        self.assertTrue(violations(
            "src/runtime_inference/freshness_operations.rs",
            "crate::freshness::trace_stage(TraceStage::AcceptedUse, || operation());",
            {"windows"},
        ))
        facade = (ROOT / "src/facade/dream_freshness.rs").read_text(encoding="utf-8")
        feature = {"windows", 'feature="freshness-performance-trace"'}
        self.assertFalse(violations("src/facade/dream_freshness.rs", facade, feature))

    def test_dream_replay_transport_rejects_import_callback_arguments(self):
        bad = "fn rebase(turn: impl FnMut(u64) -> Result<u64, String>) { journal.rebase_replay_payload(payload, turn); }"
        good = "fn rebase(coordinates: &DreamReplayCoordinates) { journal.rebase_replay_payload(payload, coordinates); }"
        self.assertTrue(violations("src/runtime_inference/dream_replay_transport.rs", bad, {"windows"}))
        self.assertFalse(violations("src/runtime_inference/dream_replay_transport.rs", good, {"windows"}))

    def test_insomnia_extraction_rejects_gateway_reporting_callback(self):
        bad = "fn start(stage_reporter: &dyn Fn(InsomniaSemanticStage, usize)) { stage_reporter(stage, items); }"
        good = "fn start(stage_events: Option<&SyncSender<InsomniaSemanticProgress>>) { send_value(stage_events); }"
        self.assertTrue(violations("src/runtime_inference/insomnia/extraction.rs", bad, {"windows"}))
        self.assertFalse(violations("src/runtime_inference/insomnia/extraction.rs", good, {"windows"}))

    def test_gateway_uses_bounded_stage_values_instead_of_callback(self):
        bad = "let stage_reporter = |stage, items| progress.report(stage, items); extractor.start_with_stage_reporter(&stage_reporter);"
        good = "let (sender, receiver) = mpsc::sync_channel::<InsomniaSemanticProgress>(0); for event in receiver { report(event) } extractor.start_with_stage_events(Some(&sender));"
        self.assertTrue(violations("src/runtime_inference/insomnia_worker_runtime.rs", bad, {"windows"}))
        self.assertFalse(violations("src/runtime_inference/insomnia_worker_runtime.rs", good, {"windows"}))

    def test_dream_source_time_is_value_sequenced_and_never_callback_owned(self):
        paths = (
            "src/runtime_inference/dream_owner_candidates.rs",
            "src/runtime_inference/dream_owner_duplicate.rs",
            "src/runtime_inference/dream_owner_publisher.rs",
            "src/runtime_inference/dream_publication_execution.rs",
        )
        for path in paths:
            with self.subTest(path=path):
                actual = (ROOT / path).read_text(encoding="utf-8")
                self.assertFalse(violations(path, actual, {"windows"}))
                self.assertTrue(violations(path, "fn source_time(f: impl Fn(&Memory) -> Option<i64>) {}", {"windows"}))
        for path, bad in (
            ("src/runtime_inference/dream_owner_duplicate.rs", "fn keys<R: Fn(&Memory) -> Option<i64>>(resolve: R) {}"),
            ("src/runtime_inference/dream_publication_execution.rs", "let source_time = |memory: &Memory| resolve(memory);"),
            ("src/runtime_inference/dream_owner_duplicate.rs", "let memories = memories.current_ids().into_iter().map(|id| memories.memory(id)).collect::<Vec<Memory>>();"),
            ("src/runtime_inference/dream_publication_execution.rs", "let corpus: Vec<Memory> = ids.into_iter().map(load).collect();"),
        ):
            with self.subTest(path=path, bad=bad):
                self.assertTrue(violations(path, bad, {"windows"}))

    def test_semantic_target_snapshot_stays_id_only_and_memory_hydration_stays_bounded(self):
        for path in (
            "src/owner_lifecycle/public_consumer_operations.rs",
            "src/runtime_inference/public_semantic_operations.rs",
        ):
            with self.subTest(path=path):
                actual = (ROOT / path).read_text(encoding="utf-8")
                self.assertFalse(violations(path, actual, {"windows"}))
        self.assertTrue(violations(
            "src/owner_lifecycle/public_consumer_operations.rs",
            "struct SemanticMemoryTarget { id: MemoryId, body_id: MemoryBodyId, memory: Memory }",
            {"windows"},
        ))
        self.assertTrue(violations(
            "src/owner_lifecycle/public_consumer_operations.rs",
            "fn semantic_memory_targets(memories: &MemoryStore) { memories.memory(id); }",
            {"windows"},
        ))
        one_pass = "fn search_eligible_semantic_targets() { for target in &memory_targets { runtime_memory(target.id); } }"
        self.assertTrue(violations("src/runtime_inference/public_semantic_operations.rs", one_pass, {"windows"}))

    def test_historical_embedding_permission_is_a_drop_guard_not_caller_callback(self):
        path = "src/runtime_inference/public_owner_operations.rs"
        actual = (ROOT / path).read_text(encoding="utf-8")
        self.assertFalse(violations(path, actual, {"windows"}))
        self.assertTrue(violations(
            path,
            "fn runtime_with_historical_embedding_write_scope<T>(&mut self, operation: impl FnOnce()) { operation(); }",
            {"windows"},
        ))
        self.assertTrue(violations(
            path,
            "struct HistoricalEmbeddingWriteScope; impl Deref for HistoricalEmbeddingWriteScope {}",
            {"windows"},
        ))


    def test_import_restore_writes_are_owner_lifecycle_operations(self):
        path = "src/imports/reconciliation_interval.rs"
        for direct in (
            "output.communities.publish(&mut output.container, &output.graph, owner, snapshot);",
            "output.dream_freshness.ingest(&rebased)?;",
            "output.dream_freshness.ingest_materialized_begin(owner, pass)?;",
            "output.entity_resolutions.persist_change(&mut output.container, change)?;",
        ):
            with self.subTest(direct=direct):
                self.assertTrue(violations(path, direct, {"windows"}))
        source = (ROOT / path).read_text(encoding="utf-8")
        self.assertFalse(violations(path, source, {"windows"}))

    def test_semantic_freshness_gateway_requires_owner_lifecycle_contracts(self):
        gateway = "fn run(&mut self) { self.owner.freshness.contains_event(id); self.owner.container.append(payload); }"
        owner = "fn plan() { cosine_similarity(source, target); plan_local_effects(policy, candidates); }"
        self.assertTrue(violations("src/runtime_inference/public_semantic_operations.rs", gateway, {"windows"}))
        self.assertTrue(violations("src/owner_lifecycle/semantic_freshness_operations.rs", owner, {"windows"}))
        runtime = (ROOT / "src/runtime_inference/public_semantic_operations.rs").read_text(encoding="utf-8")
        lifecycle = (ROOT / "src/owner_lifecycle/semantic_freshness_operations.rs").read_text(encoding="utf-8")
        self.assertFalse(violations("src/runtime_inference/public_semantic_operations.rs", runtime, {"windows"}))
        self.assertFalse(violations("src/owner_lifecycle/semantic_freshness_operations.rs", lifecycle, {"windows"}))

    def test_phy_open_validation_uses_owner_lifecycle_admission(self):
        runtime_path = "src/runtime_inference/phy_execution_owner.rs"
        self.assertTrue(violations(
            runtime_path,
            "fn validate_open_owner(&self) { self.entity_resolutions.validate_local()?; self.entity_resolutions.records(); }",
            {"windows"},
        ))
        contract_path = "src/owner_lifecycle/entity_resolution_open_admission.rs"
        self.assertTrue(violations(
            contract_path,
            "fn validate() { let rows = resolutions.records(); validate_entity_resolution_references(memories, entities, &rows); resolutions.validate_local()?; }",
            {"windows"},
        ))
        runtime = (ROOT / runtime_path).read_text(encoding="utf-8")
        contract = (ROOT / contract_path).read_text(encoding="utf-8")
        self.assertFalse(violations(runtime_path, runtime, {"windows"}))
        self.assertFalse(violations(contract_path, contract, {"windows"}))

    def test_gateway_entity_store_calls_use_owner_lifecycle_access(self):
        path = "src/runtime_inference/perception_entity_merge.rs"
        for direct in (
            "owner.owner.entities.publish(&mut owner.owner.container, id, revision, draft);",
            "owner.owner.entities.entity(id);",
            "owner.owner.entities.tombstone_direct(&mut owner.owner.container, id, revision);",
            "owner.owner.entity_resolutions.retarget_resolved(&mut owner.owner.container, key, from, to, now);",
        ):
            with self.subTest(direct=direct):
                self.assertTrue(violations(path, direct, {"windows"}))
        source = (ROOT / path).read_text(encoding="utf-8")
        access = (ROOT / "src/owner_lifecycle/entity_publication_access.rs").read_text(encoding="utf-8")
        self.assertFalse(violations(path, source, {"windows"}))
        self.assertFalse(violations("src/owner_lifecycle/entity_publication_access.rs", access, {"windows"}))

    def test_dream_facade_processing_is_runtime_forwarding_only(self):
        for path in (
            "src/facade/dream_processing.rs",
            "src/facade/dream_processor_frontier.rs",
        ):
            with self.subTest(path=path):
                source = (ROOT / path).read_text(encoding="utf-8")
                self.assertFalse(violations(path, source, {"windows"}))
        path = "src/facade/dream_processing.rs"
        for bad in (
            "pub fn process_memory(&self) -> Result<(), Error> { for pair in pairs { evaluate(pair); } self.runtime_process_memory() }",
            "pub fn process_memory(&self) -> Result<(), Error> { self.runtime_process_memory(); retry_failed_pairs(); }",
            "pub fn process_memory(&self) -> Result<(), Error> { std::thread::scope(|scope| scope.spawn(work)); self.runtime_process_memory() }",
        ):
            with self.subTest(bad=bad):
                self.assertTrue(violations(path, bad, {"windows"}))
        self.assertTrue(violations(
            "src/facade/dream_processor_frontier.rs",
            "pub fn process_memories_with_concurrency(&self) -> Result<(), Error> { classify_pairs(); self.runtime_process_memories_with_concurrency() }",
            {"windows"},
        ))

    def test_dream_publication_and_phy_writer_stay_below_public_facade(self):
        facade_paths = (
            "src/facade/dream_lifecycle.rs",
            "src/facade/dream_product_extensions.rs",
            "src/facade/dream_processing.rs",
            "src/facade/dream_processor_frontier.rs",
            "src/facade/phylactery_dream_candidates.rs",
            "src/facade/phylactery_dream_lifecycle.rs",
        )
        for path in facade_paths:
            source = (ROOT / path).read_text(encoding="utf-8")
            with self.subTest(path=path):
                self.assertFalse(violations(path, source, {"windows"}))
        for bad in (
            "DreamGraphPublication::new(&mut self.owner.container, &self.owner.memories, &mut self.owner.graph);",
            "MemoryOwnerWriter::new(&mut self.owner.memories, &mut self.owner.container);",
            "self.owner.dream_lifecycle_write_parts();",
            "self.owner.graph.publish_dream_pair(classification);",
        ):
            self.assertTrue(violations("src/facade/phylactery_dream_lifecycle.rs", bad, {"windows"}), bad)
        runtime_path = "src/runtime_inference/public_dream_operations.rs"
        runtime = (ROOT / runtime_path).read_text(encoding="utf-8")
        self.assertFalse(violations(runtime_path, runtime, {"windows"}))
        self.assertTrue(violations(
            runtime_path,
            "impl crate::phy_execution_owner::PhyExecutionOwner { fn native_publish_dream_pair() {} fn native_reconcile_dream_lifecycle() {} }",
            {"windows"},
        ))

    def test_dream_community_naming_facade_only_binds_runtime_operations(self):
        path = "src/facade/dream_community_naming.rs"
        source = (ROOT / path).read_text(encoding="utf-8")
        self.assertFalse(violations(path, source, {"windows"}))
        for bad in (
            "pub fn name_communities(&self) -> Result<(), Error> { let ids = representative_ids(community, index); self.runtime_name_communities() }",
            "pub fn name_communities(&self) -> Result<(), Error> { let name = self.generate_name(memories)?; self.runtime_name_communities() }",
            "pub fn name_communities(&self) -> Result<(), Error> { for community in communities { provider.generate_name(community); } self.runtime_name_communities() }",
        ):
            with self.subTest(bad=bad):
                self.assertTrue(violations(path, bad, {"windows"}))

    def test_historical_import_runtime_uses_import_and_lifecycle_operations(self):
        path = "src/runtime_inference/historical_import_execution.rs"
        bad_cases = (
            "crate::cva_freshness::record_dream_evidence_trace(id);",
            "let history = HistorySource::new(&self.owner.container)?;",
            "let history = HistoryReadAccess::new(&self.owner.container);",
            "let epochs = self.owner.import_epochs.open();",
            "let store: ImportEpochStore = self.runtime_completion_epochs_from_bytes()?;",
        )
        for bad in bad_cases:
            with self.subTest(bad=bad):
                self.assertTrue(violations(path, bad, {"windows"}))
        self.assertFalse(violations(
            path,
            "crate::imports::historical_owner_operations::reconstruct(&mut self.owner, options)?;",
            {"windows"},
        ))
        source = (ROOT / path).read_text(encoding="utf-8")
        self.assertFalse(violations(path, source, {"windows"}))


if __name__ == "__main__":
    unittest.main()
