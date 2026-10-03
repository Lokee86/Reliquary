use super::{MigrationError, op, processing_epochs_for_migration, require_same};
use crate::cva_reconcile_archive::{
    read_archive_tail, replay_archive_tail, replay_file_memory_links,
};
use crate::cva_reconcile_entity::{read_entity_tail, replay_entity_tail};
use crate::cva_reconcile_graph::{read_graph_tail, replay_graph_tail};
use crate::cva_reconcile_interaction::replay_interaction_streams;
use crate::cva_reconcile_memory::{read_memory_tail, replay_memory_tail};
use crate::cva_reconcile_relationship::{read_relationship_tail, replay_relationship_tail};
use crate::{Cva, ReliquaryScopeKind};
use std::path::Path;

pub(super) fn migrate(
    source_path: &Path,
    output_path: &Path,
    scope: ReliquaryScopeKind,
    owner_uuid: [u8; 16],
) -> Result<String, MigrationError> {
    let mut source = op(Cva::open(source_path))?;
    let archive = op(read_archive_tail(&mut source, 0))?;
    let memories = op(read_memory_tail(&mut source, 0))?;
    let entities = op(read_entity_tail(&mut source, 0))?;
    let relationships = op(read_relationship_tail(&mut source, 0))?;
    let graph = op(read_graph_tail(&mut source, 0))?;
    let streams = source.interaction_stream_records();
    let profiles = source.compatibility_profiles();
    let packed_infos = source.packed_vector_infos();
    let memory_vector_infos = source.memory_vector_infos();
    let archive_vector_infos = source.archive_vector_infos();
    let generations = source.vector_generations.generations().to_vec();
    let compactions = source.conversation_compactions.all_records();
    let echo = source.echo_records();
    let processing_epochs = source.processing_epochs.records();
    let processing_epochs = processing_epochs_for_migration(
        &mut source.container,
        &source.memories,
        processing_epochs,
    )?;
    let dream_pairs = source.dream_pair_records();
    let entity_resolutions = source.entity_resolution_records();
    let metadata = source.rel_metadata();

    let mut output = op(Cva::create_rel_with_uuid(
        output_path,
        owner_uuid,
        metadata.type_label.clone().or_else(|| {
            Some(
                match scope {
                    ReliquaryScopeKind::Organization => "Organization",
                    ReliquaryScopeKind::Project => "Project",
                    ReliquaryScopeKind::Connection => "Connection",
                }
                .to_owned(),
            )
        }),
    ))?;
    op(output.set_rel_metadata(
        metadata.type_label.or_else(|| {
            Some(
                match scope {
                    ReliquaryScopeKind::Organization => "Organization",
                    ReliquaryScopeKind::Project => "Project",
                    ReliquaryScopeKind::Connection => "Connection",
                }
                .to_owned(),
            )
        }),
        metadata.dependencies,
    ))?;
    for profile in profiles {
        op(output
            .compatibility_profiles
            .put(&mut output.container, profile))?;
    }
    op(replay_interaction_streams(&mut output, streams))?;
    for compaction in compactions {
        op(output.put_conversation_compaction(
            compaction.conversation_id,
            compaction.through_message_id,
            compaction.summary,
            None,
        ))?;
    }
    op(replay_archive_tail(&mut output, &archive))?;
    op(replay_memory_tail(&mut output, memories))?;
    op(replay_entity_tail(&mut output, entities))?;
    op(replay_relationship_tail(&mut output, relationships))?;
    for resolution in entity_resolutions {
        op(output.import_entity_resolution(resolution))?;
    }
    op(replay_graph_tail(&mut output, &graph))?;
    for (memory_id, lane_id, state) in processing_epochs {
        op(output
            .processing_epochs
            .put(&mut output.container, memory_id, lane_id, state))?;
    }
    for (left, right) in dream_pairs {
        op(output.mark_dream_pair_evaluated(left, right))?;
    }
    op(replay_file_memory_links(
        &mut output,
        &archive.file_memory_links,
    ))?;
    for event in echo {
        op(output.put_echo_event(event))?;
    }

    copy_vectors(
        &mut source,
        &mut output,
        packed_infos,
        memory_vector_infos,
        archive_vector_infos,
        generations,
    )?;
    op(output.sync())?;
    let owner_id = output
        .owner_id()
        .ok_or_else(|| MigrationError::Operation("migrated REL has no owner ID".into()))?;
    drop(output);
    let reopened = op(Cva::open(output_path))?;
    if !source.archive.activity_order_matches(&reopened.archive) {
        return Err(MigrationError::Operation(
            "migrated REL changed eligible activity identities or acceptance order".into(),
        ));
    }
    Ok(owner_id)
}

fn copy_vectors(
    source: &mut Cva,
    output: &mut Cva,
    packed_infos: Vec<crate::PackedVectorInfo>,
    memory_vector_infos: Vec<crate::MemoryVectorInfo>,
    archive_vector_infos: Vec<crate::ArchiveVectorInfo>,
    generations: Vec<crate::VectorGeneration>,
) -> Result<(), MigrationError> {
    for info in packed_infos {
        let packed = op(source.packed_vectors(info.id))?;
        require_same(
            info.id,
            op(output.put_packed_vectors(packed))?,
            "packed vector",
        )?;
    }
    for info in memory_vector_infos {
        let set = op(source.memory_vectors(info.id))?;
        require_same(
            info.id,
            op(output.put_memory_vectors(
                set.compatibility_profile_id,
                set.packed_vector_id,
                set.memory_body_ids,
            ))?,
            "Memory Vector",
        )?;
    }
    for info in archive_vector_infos {
        let set = op(source.archive_vectors(info.id))?;
        require_same(
            info.id,
            op(output.put_archive_vectors(set.packed_vector_id, set.fragment_ids))?,
            "Archive Vector",
        )?;
    }
    for generation in generations {
        let transaction_time_ns = source.transaction_time_ns(generation.global_version);
        output
            .container
            .set_next_transaction_time_override(transaction_time_ns);
        let result = output.publish_vector_generation(
            generation.compatibility_profile_id,
            generation.archive_vector_id,
            generation.source_archive_version,
        );
        output.container.clear_next_transaction_time_override();
        let migrated = op(result)?;
        require_same(generation.id, migrated.id, "Vector Generation")?;
    }
    Ok(())
}
