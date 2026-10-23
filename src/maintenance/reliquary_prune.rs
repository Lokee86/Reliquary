//! REL history-prune coordinator. Policy is explicit; History owns accepted proofs.
use crate::archive::history_prune::PrunedArchiveContent;
use crate::archive_codec::{ArchiveRecord, decode_record};
use crate::maintenance::select_reliquary_targets;
use crate::owner_lifecycle::{
    HistoryReadAccess, HistorySource, LifecycleTarget, OwnerError, ReliquaryOwner, ReliquaryReplay,
};
use crate::owner_maintenance::HistoryPruneManifest;
use crate::{ContentId, HistoryPruneReport, HistoryPruneRequest, ObjectRef};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;

pub(crate) struct ReliquaryPruneParts<'a> {
    pub(crate) container: &'a mut crate::Container,
    pub(crate) archive: &'a crate::Archive,
    pub(crate) memories: &'a crate::memory_store::MemoryStore,
    pub(crate) graph: &'a crate::graph_store::GraphStore,
    pub(crate) entities: &'a crate::entity_store::EntityStore,
    pub(crate) relationships: &'a crate::relationship_store::RelationshipStore,
    pub(crate) packed_vectors: &'a crate::packed_vector_store::PackedVectorStore,
    pub(crate) memory_vectors: &'a crate::memory_vector_store::MemoryVectorStore,
    pub(crate) archive_vectors: &'a crate::archive_vector_store::ArchiveVectorStore,
    pub(crate) compatibility_profiles:
        &'a crate::compatibility_profile_store::CompatibilityProfileStore,
    pub(crate) vector_generations: &'a crate::vector_generation_store::VectorGenerationStore,
    pub(crate) owner_maintenance: &'a crate::owner_maintenance::OwnerMaintenanceStore,
    pub(crate) freshness: &'a crate::freshness_storage::FreshnessStore,
    pub(crate) dream_freshness: &'a crate::freshness_dream_journal::DreamFreshnessJournal,
    pub(crate) import_epochs: &'a crate::imports::epoch_store::ImportEpochStore,
}

pub(crate) fn prune_reliquary(
    parts: ReliquaryPruneParts<'_>,
    output_path: &Path,
    request: HistoryPruneRequest,
) -> Result<HistoryPruneReport, OwnerError> {
    let ReliquaryPruneParts {
        container,
        archive,
        memories,
        graph,
        entities,
        relationships,
        packed_vectors,
        memory_vectors,
        archive_vectors,
        compatibility_profiles,
        vector_generations,
        owner_maintenance,
        freshness,
        dream_freshness,
        import_epochs,
    } = parts;

    container.sync()?;
    if import_epochs.open().is_some() {
        return Err(prune_error("cannot prune while an import epoch is open"));
    }
    if freshness
        .import_settlements()
        .iter()
        .any(|settlement| settlement.semantic.is_none())
    {
        return Err(prune_error(
            "current import settlement semantic evidence is missing",
        ));
    }
    let access = HistoryReadAccess::new(container);
    for settlement in freshness.import_settlements().to_vec() {
        crate::history::validate_historical_settlement(
            &access,
            archive,
            memories,
            graph,
            freshness,
            dream_freshness,
            import_epochs,
            &settlement,
        )?;
    }
    container.validate_path_identity()?;
    if !owner_maintenance.is_empty() {
        return Err(prune_error(
            "additional pruning of an already pruned owner is unsupported",
        ));
    }
    let owner_uuid = container
        .owner_uuid()
        .ok_or_else(|| prune_error("pruning requires a durable owner UUID"))?;
    let watermark = container.latest_version();
    let source_physical_end = container.physical_length()?;
    let selected_targets = select_reliquary_targets(request.targets).map_err(prune_error)?;
    let selected = selected_targets.conversations;
    let selected_profiles = selected_targets.compatibility_profiles;
    let selected_memories = selected_targets.memories;
    let description_targets = selected_targets.descriptions;
    for id in &selected {
        if archive.conversation_tombstone(id).is_none() {
            return Err(prune_error(
                "prune target is not a terminal conversation tombstone",
            ));
        }
    }

    let eligible_packed =
        crate::history::reliquary_vector_retention::eligible_packed_vector_profiles(
            packed_vectors,
            memory_vectors,
            archive_vectors,
            vector_generations,
            compatibility_profiles,
        );
    let selected_packed = eligible_packed
        .into_iter()
        .filter(|(_, profile)| selected_profiles.contains(profile))
        .collect::<Vec<_>>();
    if selected_profiles
        .iter()
        .any(|profile| !selected_packed.iter().any(|(_, anchor)| anchor == profile))
    {
        return Err(prune_error(
            "selected compatibility profile has no eligible packed payload",
        ));
    }

    let mut selected_content = HashSet::new();
    let mut protected_content = HashSet::new();
    for version in archive.record_versions() {
        match archive.read_publication(container, *version)?.0 {
            ArchiveRecord::Node(node) => {
                if selected.contains(&node.conversation_id) {
                    selected_content.insert(node.content_id);
                } else {
                    protected_content.insert(node.content_id);
                }
            }
            ArchiveRecord::IngestedTurn(turn) => {
                if selected.contains(&turn.node.conversation_id) {
                    selected_content.insert(turn.node.content_id);
                } else {
                    protected_content.insert(turn.node.content_id);
                }
                protected_content.extend(turn.attachments.iter().map(|file| file.content_id));
            }
            ArchiveRecord::File(file) => {
                protected_content.insert(file.content_id);
            }
            _ => {}
        }
    }
    if selected_content
        .iter()
        .any(|id| protected_content.contains(id))
    {
        return Err(prune_error(
            "selected payload is required by unselected history or a file",
        ));
    }

    let objects = container.chunks()?;
    let mut lengths = HashMap::<ContentId, u64>::new();
    let mut occurrences = HashMap::<ContentId, usize>::new();
    for object in &objects {
        let payload = container.read(*object)?;
        if let ArchiveRecord::Content(id, bytes) = decode_record(&payload)?
            && selected_content.contains(&id)
        {
            let length = bytes.len() as u64;
            if lengths.insert(id, length).is_some_and(|old| old != length) {
                return Err(prune_error("conflicting selected content lengths"));
            }
            *occurrences.entry(id).or_default() += 1;
        }
    }
    if lengths.len() != selected_content.len() {
        return Err(prune_error("selected payload is missing from source"));
    }

    let mut selected_bodies = HashMap::new();
    let mut memory_descriptors = Vec::new();
    for id in &selected_memories {
        let retained = memories.retention_descriptor(*id)?;
        let mut bodies = Vec::new();
        for body in retained.prunable_body_ids {
            if memories
                .records()
                .iter()
                .any(|record| record.body_id == body && !selected_memories.contains(&record.id))
            {
                return Err(prune_error(
                    "Memory body is shared outside selected tombstones",
                ));
            }
            let object = memories
                .prune_body_chunk(body)
                .ok_or_else(|| prune_error("Memory body is not independently removable"))?;
            let raw = container.read(object)?;
            let Some((body_id, bytes)) = crate::memory_codec::decode_body(&raw)? else {
                return Err(prune_error(
                    "Memory body is retained inside grouped publication evidence",
                ));
            };
            if body_id != body {
                return Err(prune_error("Memory body identity mismatch"));
            }
            selected_bodies.insert(object, (body, bytes.len() as u64));
            bodies.push(body);
        }
        memory_descriptors.push(crate::history::memory_retention::descriptor(
            memories, *id, &bodies,
        )?);
    }
    let mut descriptors = selected
        .iter()
        .map(|id| {
            crate::history::archive_retention::descriptor(
                archive,
                &HistorySource::new(container).map_err(|error| error.0)?,
                id,
                watermark,
                &lengths,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    descriptors.extend(memory_descriptors);
    for target in &description_targets {
        descriptors.push(
            crate::history::reliquary_description_retention::description_retention(
                entities,
                relationships,
                target,
            )?,
        );
    }
    for (packed_id, profile_id) in &selected_packed {
        let profile_source = HistorySource::new(container);
        descriptors.push(
            crate::history::reliquary_vector_retention::packed_vector_retention_descriptor(
                &profile_source,
                packed_vectors,
                memory_vectors,
                archive_vectors,
                compatibility_profiles,
                vector_generations,
                *profile_id,
                *packed_id,
                watermark,
            )?,
        );
    }
    descriptors.sort_by(|a, b| (&a.target, &a.payloads).cmp(&(&b.target, &b.payloads)));
    let manifest = HistoryPruneManifest {
        owner_uuid,
        source_global_version: watermark,
        source_history_digest: crate::history::owner_history_digest(
            &crate::owner_lifecycle::HistorySource::new(&container)
                .map_err(|error| OwnerError::Repack(error.to_string()))?,
            watermark,
            graph,
        )
        .map_err(OwnerError::Repack)?,
        descriptors,
    };
    let manifest_bytes = manifest.encode().map_err(OwnerError::Repack)?;
    let manifest_digest = manifest.digest().map_err(OwnerError::Repack)?;
    let markers = lengths
        .iter()
        .map(|(id, length)| {
            (
                *id,
                PrunedArchiveContent {
                    id: *id,
                    byte_length: *length,
                    manifest_digest,
                },
            )
        })
        .collect::<HashMap<_, _>>();
    let packed_ids = selected_packed
        .iter()
        .map(|(id, _)| *id)
        .collect::<HashSet<_>>();
    let packed_markers = selected_packed
        .iter()
        .map(|(id, _)| {
            Ok((
                *id,
                crate::history::reliquary_vector_retention::packed_vector_retention_marker(
                    packed_vectors,
                    *id,
                    manifest_digest,
                )?,
            ))
        })
        .collect::<Result<HashMap<_, _>, OwnerError>>()?;
    let mut packed_objects = HashMap::new();
    for object in &objects {
        let payload = container.read(*object)?;
        if let Some(decoded) = crate::packed_vector_codec::decode_object(&payload)?
            && packed_ids.contains(&decoded.id)
            && packed_objects.insert(decoded.id, *object).is_some()
        {
            return Err(prune_error("duplicate packed vector object identity"));
        }
    }
    if packed_objects.len() != packed_ids.len() {
        return Err(prune_error(
            "selected packed payload is missing from source",
        ));
    }
    let packed_objects_by_ref = packed_objects
        .iter()
        .map(|(id, object)| (*object, *id))
        .collect::<HashMap<_, _>>();
    let selected_entities = description_targets
        .iter()
        .filter_map(|target| match target {
            LifecycleTarget::Entity(id) => Some(*id),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let entity_plan = crate::entity_description_prune::prepare_description_prune(
        entities,
        container,
        &selected_entities,
        manifest_digest,
    )?;
    let mut description_objects = entity_plan.replacements;
    let mut description_discard = entity_plan.discard;
    let selected_relationships = description_targets
        .iter()
        .filter_map(|target| match target {
            LifecycleTarget::Relationship(id) => Some(*id),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let relationship_plan = crate::relationship_description_prune::prepare_description_prune(
        relationships,
        container,
        &selected_relationships,
        manifest_digest,
    )?;
    description_objects.extend(relationship_plan.replacements);
    description_discard.extend(relationship_plan.discard);
    let source_file_bytes = fs::metadata(container.path())
        .map_err(|e| prune_error(&e.to_string()))?
        .len();
    validate_source_freeze(container, source_physical_end, owner_uuid, watermark)?;
    let mut output = container.create_empty_like(output_path)?;
    let output_guard = output.read_source()?;
    let copy = (|| {
        let mut relocated = BTreeMap::<ObjectRef, ObjectRef>::new();
        let mut marked = HashSet::new();
        for object in &objects {
            if description_discard.contains(object) {
                continue;
            }
            let raw = container.read(*object)?;
            if let Some(bytes) = description_objects.get(object) {
                let destination = output.append(bytes)?;
                relocated.insert(*object, destination);
                continue;
            }
            if let Some((body, length)) = selected_bodies.get(object) {
                let marker = crate::memory_store::history_prune::PrunedMemoryBody::encode(
                    *body,
                    *length,
                    manifest_digest,
                );
                let destination = output.append(&marker)?;
                relocated.insert(*object, destination);
                continue;
            }
            if let Some(id) = packed_objects_by_ref.get(object) {
                let destination = output.append(&packed_markers[id])?;
                relocated.insert(*object, destination);
                continue;
            }
            let transformed = if let ArchiveRecord::Content(id, _) = decode_record(&raw)?
                && let Some(marker) = markers.get(&id)
            {
                if !marked.insert(id) {
                    continue;
                }
                marker.encode()
            } else {
                crate::owner_lifecycle::relocation::relocate_payload(&raw, &relocated)
                    .map_err(OwnerError::Repack)?
            };
            let destination = output.append(&transformed)?;
            relocated.insert(*object, destination);
        }
        output.append(&manifest_bytes)?;
        output.sync()?;
        Ok::<_, OwnerError>(relocated)
    })();
    let relocated = match copy {
        Ok(value) => value,
        Err(error) => {
            drop(output);
            if output_guard.validate_path_identity().is_ok() {
                let _ = fs::remove_file(output_path);
            }
            return Err(error);
        }
    };
    let verified = (|| {
        let (mut reopened, _) = reopen_reliquary(output)?;
        validate_copy(
            container,
            archive,
            graph,
            freshness,
            import_epochs,
            &reopened,
        )?;
        if prune_clocks(
            container,
            archive,
            memories,
            entities,
            relationships,
            graph,
            vector_generations,
        ) != prune_owner_clocks(&reopened)
            || container.owner_uuid() != reopened.container.owner_uuid()
            || !archive.activity_history_matches(&reopened.archive)
        {
            return Err(prune_error(
                "pruned copy changed owner clocks or activity history",
            ));
        }
        for global in 1..=watermark {
            if container.transaction_time_ns(global)
                != reopened.container.transaction_time_ns(global)
            {
                return Err(prune_error("pruned copy changed transaction times"));
            }
        }
        for object in &objects {
            if description_discard.contains(object) {
                continue;
            }
            let raw = container.read(*object)?;
            if let Some(bytes) = description_objects.get(object) {
                if reopened.container.read(relocated[object])? != *bytes {
                    return Err(prune_error("description marker changed"));
                }
                continue;
            }
            if let Some((body, length)) = selected_bodies.get(object) {
                let expected = crate::memory_store::history_prune::PrunedMemoryBody::encode(
                    *body,
                    *length,
                    manifest_digest,
                );
                if reopened.container.read(relocated[object])? != expected {
                    return Err(prune_error("Memory marker changed"));
                }
                continue;
            }
            if let Some(id) = packed_objects_by_ref.get(object) {
                let destination = relocated
                    .get(object)
                    .ok_or_else(|| prune_error("packed marker was not written"))?;
                if reopened.container.read(*destination)? != packed_markers[id]
                    || !reopened.packed_vectors.marker_matches(*id, manifest_digest)
                {
                    return Err(prune_error("packed marker failed owner validation"));
                }
                continue;
            }
            if let ArchiveRecord::Content(id, _) = decode_record(&raw)?
                && let Some(marker) = markers.get(&id)
            {
                let Some(destination) = relocated.get(object) else {
                    continue;
                };
                if reopened.container.read(*destination)? != marker.encode() {
                    return Err(prune_error("pruned marker changed during reopen"));
                }
                continue;
            }
            let expected = crate::owner_lifecycle::relocation::relocate_payload(&raw, &relocated)
                .map_err(OwnerError::Repack)?;
            let destination = relocated
                .get(object)
                .ok_or_else(|| prune_error("nonselected raw chunk was lost"))?;
            let actual = reopened.container.read(*destination)?;
            if expected != actual {
                return Err(prune_error("nonselected raw chunk changed during prune"));
            }
        }
        for node in archive
            .nodes
            .iter()
            .filter(|node| selected.contains(&node.conversation_id))
        {
            if !matches!(
                reopened.archive.node_content(&mut reopened.container, node),
                Err(crate::ArchiveError::PayloadUnavailable(
                    crate::owner_lifecycle::PayloadUnavailable::Pruned
                ))
            ) {
                return Err(prune_error(
                    "selected historical payload remained available",
                ));
            }
        }
        let output_bytes = fs::metadata(output_path)
            .map_err(|e| prune_error(&e.to_string()))?
            .len();
        if output_bytes >= source_file_bytes {
            return Err(prune_error("prune did not shrink the output"));
        }
        Ok(output_bytes)
    })();
    let output_file_bytes = match verified {
        Ok(bytes) => bytes,
        Err(error) => {
            if output_guard.validate_path_identity().is_ok() {
                let _ = fs::remove_file(output_path);
            }
            return Err(error);
        }
    };
    let destroyed_body_bytes = lengths.iter().try_fold(0_u64, |sum, (id, length)| {
        sum.checked_add(
            length
                .checked_mul(occurrences[id] as u64)
                .ok_or_else(|| prune_error("pruned length overflow"))?,
        )
        .ok_or_else(|| prune_error("pruned length overflow"))
    })?;
    let destroyed_packed_bytes = selected_packed.iter().try_fold(0_u64, |sum, (id, _)| {
        let bytes = packed_vectors
            .info(*id)
            .ok_or_else(|| prune_error("packed vector info is missing"))?
            .byte_len;
        sum.checked_add(bytes)
            .ok_or_else(|| prune_error("pruned packed byte length overflow"))
    })?;
    if let Err(error) =
        validate_source_freeze(container, source_physical_end, owner_uuid, watermark)
    {
        if output_guard.validate_path_identity().is_ok() {
            let _ = fs::remove_file(output_path);
        }
        return Err(error);
    }
    output_guard.validate_path_identity()?;
    Ok(HistoryPruneReport {
        pruned_conversations: selected.len(),
        pruned_content_objects: markers.len(),
        pruned_memory_bodies: selected_bodies.len(),
        pruned_packed_vectors: packed_markers.len(),
        destroyed_body_bytes,
        destroyed_packed_bytes,
        source_file_bytes,
        output_file_bytes,
        manifest_digest,
    })
}

fn prune_error(message: &str) -> OwnerError {
    OwnerError::Repack(message.to_owned())
}
fn validate_source_freeze(
    container: &crate::Container,
    physical_end: u64,
    owner_uuid: [u8; 16],
    watermark: u64,
) -> Result<(), OwnerError> {
    container.validate_path_identity()?;
    if container.physical_length()? != physical_end
        || container.owner_uuid() != Some(owner_uuid)
        || container.latest_version() != watermark
    {
        return Err(crate::ContainerError::UnstableSnapshot.into());
    }
    Ok(())
}
fn validate_copy(
    container: &crate::Container,
    archive: &crate::Archive,
    graph: &crate::graph_store::GraphStore,
    freshness: &crate::freshness_storage::FreshnessStore,
    import_epochs: &crate::imports::epoch_store::ImportEpochStore,
    copy: &ReliquaryOwner,
) -> Result<(), OwnerError> {
    let source_access = HistoryReadAccess::new(container);
    let copy_access = HistoryReadAccess::new(&copy.container);
    if !crate::history::historical_copy_preserves_import_authority(
        crate::history::HistoricalCopyView {
            archive,
            graph,
            freshness,
            import_epochs,
            read_access: &source_access,
            accepted_global_head: container.latest_version(),
        },
        crate::history::HistoricalCopyView {
            archive: &copy.archive,
            graph: &copy.graph,
            freshness: &copy.freshness,
            import_epochs: &copy.import_epochs,
            read_access: &copy_access,
            accepted_global_head: copy.container.latest_version(),
        },
    )? {
        return Err(prune_error(
            "physical transformation changed historical checkpoint scoring or receipt/proof authority",
        ));
    }
    Ok(())
}
pub(super) fn reopen_reliquary(
    container: crate::Container,
) -> Result<(ReliquaryOwner, crate::FreshnessDueDispatcher), OwnerError> {
    let mut replay = ReliquaryReplay::read(container)?;
    let reconstruction = crate::history::reconstruct_reliquary(&mut replay)?;
    Ok(replay.into_owner(reconstruction))
}
fn prune_clocks(
    container: &crate::Container,
    archive: &crate::Archive,
    memories: &crate::memory_store::MemoryStore,
    entities: &crate::entity_store::EntityStore,
    relationships: &crate::relationship_store::RelationshipStore,
    graph: &crate::graph_store::GraphStore,
    vector_generations: &crate::vector_generation_store::VectorGenerationStore,
) -> [u64; 8] {
    [
        container.latest_version(),
        archive.archive_version(),
        memories.memory_version(),
        entities.entity_version(),
        relationships.relationship_version(),
        graph.graph_version(),
        graph.memory_graph_version(),
        vector_generations.vector_version(),
    ]
}
fn prune_owner_clocks(owner: &ReliquaryOwner) -> [u64; 8] {
    [
        owner.container.latest_version(),
        owner.archive.archive_version(),
        owner.memories.memory_version(),
        owner.entities.entity_version(),
        owner.relationships.relationship_version(),
        owner.graph.graph_version(),
        owner.graph.memory_graph_version(),
        owner.vector_generations.vector_version(),
    ]
}

#[cfg(test)]
mod source_freeze_tests {
    use super::validate_source_freeze;
    use crate::owner_lifecycle::OwnerError;
    use crate::{Container, ContainerError, ContainerIdentity, FileKind};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn rel_prune_source_freeze_rejects_appended_frame_and_keeps_prefix() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("rel-prune-freeze-{}.rel", uuid::Uuid::new_v4()));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut container = Container::create_with_identity(
            &path,
            ContainerIdentity {
                file_kind: FileKind::Reliquary,
                scope: None,
            },
        )
        .unwrap();
        container.append(b"initial REL payload").unwrap();
        let accepted_prefix = fs::read(&path).unwrap();
        let physical_end = container.physical_length().unwrap();
        let owner_uuid = container.owner_uuid().unwrap();
        let watermark = container.latest_version();

        validate_source_freeze(&container, physical_end, owner_uuid, watermark).unwrap();
        container.append(b"unaccepted nonsemantic frame").unwrap();

        let grown = fs::read(&path).unwrap();
        assert!(grown.starts_with(&accepted_prefix));
        assert!(matches!(
            validate_source_freeze(&container, physical_end, owner_uuid, watermark),
            Err(OwnerError::Container(ContainerError::UnstableSnapshot))
        ));
        drop(container);
        fs::remove_file(path).unwrap();
    }
}
