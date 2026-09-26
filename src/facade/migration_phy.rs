use super::{MigrationError, op, processing_epochs_for_migration, require_same};
use crate::graph_codec::{decode_batch, decode_mutation, decode_version};
use crate::{
    Container, EntityDraft, GraphRelationOrigin, Memory, MemoryDraft, Phylactery,
    RelationshipDraft, SemanticGraphRelationChange,
};
use std::path::Path;

pub(super) fn migrate(
    source_path: &Path,
    output_path: &Path,
    owner_uuid: [u8; 16],
) -> Result<String, MigrationError> {
    let mut source = op(Phylactery::open(source_path))?;
    let revisions: Vec<_> = source
        .memories
        .records()
        .iter()
        .map(|record| (record.id, record.revision))
        .collect();
    let graph = graph_transactions(&mut source.container)?;
    let entities = source.entities.records().to_vec();
    let relationships = source.relationships.records().to_vec();
    let profiles = source.compatibility_profiles.profiles();
    let packed_infos = source.packed_vectors.infos();
    let memory_vector_infos = source.memory_vectors.infos();
    let processing_epochs = source.processing_epochs.records();
    let processing_epochs = processing_epochs_for_migration(
        &mut source.container,
        &source.memories,
        processing_epochs,
    )?;
    let dream_pairs = source.dream_pair_records();
    let entity_resolutions = source.entity_resolution_records();

    let mut output = op(Phylactery::create_with_uuid(output_path, owner_uuid))?;
    for profile in profiles {
        op(output
            .compatibility_profiles
            .put(&mut output.container, profile))?;
    }
    for (id, revision) in revisions {
        let memory = op(source.memory_revision(id, revision))?;
        let transaction_time_ns = source.transaction_time_ns(memory.global_version);
        let source_ref = memory.source_ref.clone();
        let temporal_inference = memory.temporal_inference.clone();
        let routing_metadata = memory.routing_metadata.clone();
        let draft = memory_draft(memory);
        output
            .container
            .set_next_transaction_time_override(transaction_time_ns);
        let result = output
            .memories
            .publish_with_source_ref_and_temporal_inference(
                &mut output.container,
                Some(id),
                revision.saturating_sub(1),
                draft,
                source_ref,
                temporal_inference,
            );
        output.container.clear_next_transaction_time_override();
        op(result)?;
        if let Some(metadata) = routing_metadata {
            op(output
                .memories
                .put_routing_metadata(&mut output.container, metadata))?;
        }
    }
    for entity in entities {
        let transaction_time_ns = source.transaction_time_ns(entity.global_version);
        let draft = EntityDraft {
            canonical_name: entity.canonical_name,
            aliases: entity.aliases,
            kind: entity.kind,
            summary: entity.summary,
            mutation_id: entity.mutation_id,
            created_at_ns: entity.created_at_ns,
            updated_at_ns: entity.updated_at_ns,
        };
        output
            .container
            .set_next_transaction_time_override(transaction_time_ns);
        let result = output.replay_entity_revision(
            entity.id,
            entity.revision.saturating_sub(1),
            draft,
            entity.global_id,
        );
        output.container.clear_next_transaction_time_override();
        op(result)?;
    }
    for relationship in relationships {
        let transaction_time_ns = source.transaction_time_ns(relationship.global_version);
        let draft = RelationshipDraft {
            kind: relationship.kind,
            participants: relationship.participants,
            evidence: relationship.evidence,
            summary: relationship.summary,
            mutation_id: relationship.mutation_id,
            created_at_ns: relationship.created_at_ns,
            updated_at_ns: relationship.updated_at_ns,
        };
        output
            .container
            .set_next_transaction_time_override(transaction_time_ns);
        let result = output.replay_relationship_revision(
            Some(relationship.id),
            relationship.revision.saturating_sub(1),
            draft,
        );
        output.container.clear_next_transaction_time_override();
        op(result)?;
    }
    for resolution in entity_resolutions {
        op(output.import_entity_resolution(resolution))?;
    }
    for (transaction, origin, transaction_time_ns) in graph {
        let graph_version = output.graph_version();
        output
            .container
            .set_next_transaction_time_override(transaction_time_ns);
        let result = output.set_semantic_relations_with_origin(&transaction, origin, graph_version);
        output.container.clear_next_transaction_time_override();
        op(result)?;
    }
    for (memory_id, lane_id, state) in processing_epochs {
        op(output
            .processing_epochs
            .put(&mut output.container, memory_id, lane_id, state))?;
    }
    for (left, right) in dream_pairs {
        op(output.mark_dream_pair_evaluated(left, right))?;
    }
    copy_vectors(&mut source, &mut output, packed_infos, memory_vector_infos)?;
    op(output.sync())?;
    let owner_id = output
        .owner_id()
        .ok_or_else(|| MigrationError::Operation("migrated PHY has no owner ID".into()))?;
    drop(output);
    op(Phylactery::open(output_path))?;
    Ok(owner_id)
}

fn copy_vectors(
    source: &mut Phylactery,
    output: &mut Phylactery,
    packed_infos: Vec<crate::PackedVectorInfo>,
    memory_vector_infos: Vec<crate::MemoryVectorInfo>,
) -> Result<(), MigrationError> {
    for info in packed_infos {
        let packed = op(source.packed_vectors.get(&mut source.container, info.id))?;
        require_same(
            info.id,
            op(output.packed_vectors.put(&mut output.container, packed))?,
            "packed vector",
        )?;
    }
    for info in memory_vector_infos {
        let set = op(source.memory_vectors.get(&mut source.container, info.id))?;
        let id = op(output.memory_vectors.put(
            &mut output.container,
            &output.memories,
            &output.compatibility_profiles,
            &output.packed_vectors,
            set.compatibility_profile_id,
            set.packed_vector_id,
            set.memory_body_ids,
        ))?;
        require_same(info.id, id, "Memory Vector")?;
    }
    Ok(())
}

fn graph_transactions(
    container: &mut Container,
) -> Result<
    Vec<(
        Vec<SemanticGraphRelationChange>,
        GraphRelationOrigin,
        Option<i64>,
    )>,
    MigrationError,
> {
    let chunks = op(container.chunks())?;
    let mut transactions = Vec::new();
    for chunk in chunks {
        let payload = op(container.read(chunk))?;
        let Some(version) = op(decode_version(&payload))? else {
            continue;
        };
        let mutation_payload = op(container.read(version.mutation))?;
        let mutations = if let Some(batch) = op(decode_batch(&mutation_payload))? {
            batch
        } else if let Some(mutation) = op(decode_mutation(&mutation_payload))? {
            vec![mutation]
        } else {
            return Err(MigrationError::Operation(
                "graph version points at a non-graph mutation".into(),
            ));
        };
        let origin = mutations
            .first()
            .map(|mutation| mutation.origin)
            .ok_or_else(|| MigrationError::Operation("empty graph transaction".into()))?;
        if mutations.iter().any(|mutation| mutation.origin != origin) {
            return Err(MigrationError::Operation(
                "graph transaction mixes relation origins".into(),
            ));
        }
        let changes = mutations
            .into_iter()
            .map(|mutation| SemanticGraphRelationChange {
                source: mutation.source,
                target: mutation.target,
                kind: mutation.kind,
                active: mutation.active,
            })
            .collect();
        transactions.push((
            changes,
            origin,
            container.transaction_time_ns(version.global_version),
        ));
    }
    Ok(transactions)
}

fn memory_draft(memory: Memory) -> MemoryDraft {
    MemoryDraft {
        category: memory.category,
        memory_type: memory.memory_type,
        authority_kind: memory.authority_kind,
        temporal_status: memory.temporal_status,
        title: memory.title,
        content: memory.content,
        scope: memory.scope,
        lifecycle_state: memory.lifecycle_state,
        archived: memory.archived,
        superseded_by: memory.superseded_by,
        parent_id: memory.parent_id,
        source_node_id: memory.source_node_id,
        content_source_conversation_id: memory.content_source_conversation_id,
        content_source_node_id: memory.content_source_node_id,
        grounding_source_conversation_id: memory.grounding_source_conversation_id,
        grounding_source_node_id: memory.grounding_source_node_id,
        source_episode_id: memory.source_episode_id,
        source_time_ns: memory.source_time_ns,
        mutation_id: memory.mutation_id,
        created_at_ns: memory.created_at_ns,
        updated_at_ns: memory.updated_at_ns,
    }
}
