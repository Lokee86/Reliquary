//! PHY history pruning coordinator. Reads lower proofs and owns physical publication.
use crate::MemoryBodyId;
use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::container::Container;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::maintenance::{HistoryPruneReport, select_phylactery_targets};
use crate::memory_store::MemoryStore;
use crate::memory_vector_store::MemoryVectorStore;
use crate::owner_lifecycle::OwnerPhylacteryError;
use crate::owner_maintenance::{HistoryPruneManifest, PrunedPayload, RetentionDomain};
use crate::packed_vector_store::PackedVectorStore;
use crate::relationship_store::RelationshipStore;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;

pub(crate) struct PhylacteryPruneParts<'a> {
    pub(crate) owner_maintenance: &'a crate::owner_maintenance::OwnerMaintenanceStore,
    pub(crate) container: &'a mut Container,
    pub(crate) memories: &'a MemoryStore,
    pub(crate) graph: &'a GraphStore,
    pub(crate) packed_vectors: &'a PackedVectorStore,
    pub(crate) memory_vectors: &'a MemoryVectorStore,
    pub(crate) compatibility_profiles: &'a CompatibilityProfileStore,
    pub(crate) entities: &'a EntityStore,
    pub(crate) relationships: &'a RelationshipStore,
}

pub(crate) fn prune_phylactery(
    parts: PhylacteryPruneParts<'_>,
    output_path: &Path,
    targets: Vec<crate::owner_lifecycle::LifecycleTarget>,
) -> Result<HistoryPruneReport, OwnerPhylacteryError> {
    let PhylacteryPruneParts {
        owner_maintenance,
        container,
        memories: memory_store,
        graph,
        packed_vectors,
        memory_vectors,
        compatibility_profiles,
        entities: entity_store,
        relationships: relationship_store,
    } = parts;
    if output_path == container.path() || output_path.exists() {
        return Err(OwnerPhylacteryError::Repack(
            "prune output must be a new path separate from the source".into(),
        ));
    }
    container.sync()?;
    container.validate_path_identity()?;
    if !owner_maintenance.is_empty() {
        return Err(OwnerPhylacteryError::Repack(
            "additional pruning of an already pruned owner is unsupported".into(),
        ));
    }
    let owner_uuid = container.owner_uuid().ok_or_else(|| {
        OwnerPhylacteryError::Repack("pruning requires a durable owner UUID".into())
    })?;
    let watermark = container.latest_version();
    let selected_targets = select_phylactery_targets(targets)
        .map_err(|message| OwnerPhylacteryError::Repack(message.into()))?;
    let memory_targets = selected_targets.memories;
    let entity_targets = selected_targets.entities;
    let relationship_targets = selected_targets.relationships;
    let profiles = selected_targets.compatibility_profiles;

    let mut selected_bodies = HashMap::<crate::ObjectRef, (MemoryBodyId, u64)>::new();
    let mut descriptors = Vec::new();
    for id in &memory_targets {
        let retained = memory_store.retention_descriptor(*id)?;
        let mut bodies = Vec::new();
        for body in retained.prunable_body_ids {
            if memory_store
                .records()
                .iter()
                .any(|record| record.body_id == body && record.id != *id)
            {
                return Err(OwnerPhylacteryError::Repack(
                    "Memory body is shared outside the selected tombstone".into(),
                ));
            }
            let object = memory_store.prune_body_chunk(body).ok_or_else(|| {
                OwnerPhylacteryError::Repack("Memory body is not independently removable".into())
            })?;
            let raw = container.read(object)?;
            let Some((decoded, bytes)) = crate::memory_codec::decode_body(&raw)? else {
                return Err(OwnerPhylacteryError::Repack(
                    "Memory body is retained inside grouped publication evidence".into(),
                ));
            };
            if decoded != body
                || selected_bodies
                    .insert(object, (body, bytes.len() as u64))
                    .is_some()
            {
                return Err(OwnerPhylacteryError::Repack(
                    "invalid or duplicate Memory body".into(),
                ));
            }
            bodies.push(body);
        }
        descriptors.push(crate::history::memory_retention::descriptor(
            memory_store,
            *id,
            &bodies,
        )?);
    }
    for id in &entity_targets {
        let _ = entity_store.retention_descriptor(*id)?;
        descriptors.push(crate::history::phylactery_entity_retention(
            entity_store,
            *id,
        )?);
    }
    for id in &relationship_targets {
        let _ = relationship_store.retention_descriptor(*id)?;
        descriptors.push(crate::history::phylactery_relationship_retention(
            relationship_store,
            *id,
        )?);
    }
    for profile in &profiles {
        descriptors.push(crate::history::phylactery_profile_retention(
            &crate::owner_lifecycle::HistorySource::new(container)
                .map_err(|error| OwnerPhylacteryError::Repack(error.to_string()))?,
            compatibility_profiles,
            memory_vectors,
            packed_vectors,
            *profile,
        )?);
    }
    descriptors.sort_by(|a, b| (&a.target, &a.payloads).cmp(&(&b.target, &b.payloads)));
    let manifest = HistoryPruneManifest {
        owner_uuid,
        source_global_version: watermark,
        source_history_digest: crate::history::owner_history_digest(
            &crate::owner_lifecycle::HistorySource::new(&container)
                .map_err(|error| OwnerPhylacteryError::Repack(error.to_string()))?,
            watermark,
            &graph,
        )
        .map_err(OwnerPhylacteryError::Repack)?,
        descriptors,
    };
    let manifest_bytes = manifest.encode().map_err(OwnerPhylacteryError::Repack)?;
    let manifest_digest = manifest.digest().map_err(OwnerPhylacteryError::Repack)?;
    let mut description_objects = HashMap::new();
    let mut packed_objects = HashMap::new();
    let selected_entities = entity_targets.iter().copied().collect::<HashSet<_>>();
    let selected_relationships = relationship_targets.iter().copied().collect::<HashSet<_>>();
    let mut packed_ids = HashSet::new();
    for profile in &profiles {
        for descriptor in manifest.descriptors.iter().filter(|d| {
            d.target.domain == RetentionDomain::CompatibilityProfile
                && d.target.identity == profile.0
        }) {
            for payload in &descriptor.payloads {
                if let PrunedPayload::PackedVectors(raw) = payload {
                    packed_ids.insert(crate::PackedVectorId(*raw));
                }
            }
        }
    }
    let entity_plan = crate::entity_description_prune::prepare_description_prune(
        entity_store,
        container,
        &selected_entities,
        manifest_digest,
    )?;
    description_objects.extend(entity_plan.replacements);
    let mut description_discard = entity_plan.discard;
    let relationship_plan = crate::relationship_description_prune::prepare_description_prune(
        relationship_store,
        container,
        &selected_relationships,
        manifest_digest,
    )?;
    description_objects.extend(relationship_plan.replacements);
    description_discard.extend(relationship_plan.discard);
    for object in container.chunks()? {
        let raw = container.read(object)?;
        if let Some(decoded) = crate::packed_vector_codec::decode_object(&raw)?
            && packed_ids.contains(&decoded.id)
        {
            if packed_objects.insert(decoded.id, object).is_some() {
                return Err(OwnerPhylacteryError::Repack(
                    "duplicate packed-vector object identity".into(),
                ));
            }
        }
    }
    if packed_objects.len() != packed_ids.len() {
        return Err(OwnerPhylacteryError::Repack(
            "selected packed payload is missing from source".into(),
        ));
    }
    let packed_markers = packed_ids
        .iter()
        .map(|id| Ok((*id, packed_vectors.marker_bytes(*id, manifest_digest)?)))
        .collect::<Result<HashMap<_, _>, OwnerPhylacteryError>>()?;
    let packed_by_object = packed_objects
        .iter()
        .map(|(id, object)| (*object, *id))
        .collect::<HashMap<_, _>>();
    let source_file_bytes = fs::metadata(container.path())
        .map_err(|e| OwnerPhylacteryError::Repack(e.to_string()))?
        .len();
    let source_objects = container.chunks()?;
    let mut output = container.create_empty_like(output_path)?;
    let output_guard = output.read_source()?;
    let copy = (|| {
        let mut relocated = BTreeMap::new();
        for object in &source_objects {
            if description_discard.contains(object) {
                continue;
            }
            let raw = container.read(*object)?;
            let replacement = if let Some(bytes) = description_objects.get(object) {
                Some(bytes.clone())
            } else if let Some((body, length)) = selected_bodies.get(object) {
                Some(
                    crate::memory_store::history_prune::PrunedMemoryBody::encode(
                        *body,
                        *length,
                        manifest_digest,
                    ),
                )
            } else if let Some(id) = packed_by_object.get(object) {
                Some(packed_markers[id].clone())
            } else {
                None
            };
            let payload = if let Some(replacement) = replacement {
                replacement
            } else {
                crate::maintenance::relocation::relocate_payload(&raw, &relocated)
                    .map_err(OwnerPhylacteryError::Repack)?
            };
            let destination = output.append(&payload)?;
            relocated.insert(*object, destination);
        }
        output.append(&manifest_bytes)?;
        output.sync()?;
        Ok::<_, OwnerPhylacteryError>(())
    })();
    if let Err(error) = copy {
        drop(output);
        if output_guard.validate_path_identity().is_ok() {
            let _ = fs::remove_file(output_path);
        }
        return Err(error);
    }
    let reopened = super::phylactery_copy::reopen(output).map_err(|error| {
        if output_guard.validate_path_identity().is_ok() {
            let _ = fs::remove_file(output_path);
        }
        error
    })?;
    if phy_clocks(
        container,
        memory_store,
        entity_store,
        relationship_store,
        graph,
    ) != phy_clocks(
        &reopened.container,
        &reopened.memories,
        &reopened.entities,
        &reopened.relationships,
        &reopened.graph,
    ) || container.owner_uuid() != reopened.container.owner_uuid()
    {
        drop(reopened);
        if output_guard.validate_path_identity().is_ok() {
            let _ = fs::remove_file(output_path);
        }
        return Err(OwnerPhylacteryError::Repack(
            "pruned PHY changed owner or semantic clocks".into(),
        ));
    }
    drop(reopened);
    let output_file_bytes = fs::metadata(output_path)
        .map_err(|e| OwnerPhylacteryError::Repack(e.to_string()))?
        .len();
    Ok(HistoryPruneReport {
        pruned_conversations: 0,
        pruned_content_objects: 0,
        pruned_memory_bodies: selected_bodies.len(),
        pruned_packed_vectors: packed_ids.len(),
        destroyed_body_bytes: selected_bodies.values().map(|(_, len)| *len).sum(),
        destroyed_packed_bytes: packed_ids
            .iter()
            .filter_map(|id| packed_vectors.info(*id))
            .map(|info| info.byte_len)
            .sum(),
        source_file_bytes,
        output_file_bytes,
        manifest_digest,
    })
}

fn phy_clocks(
    container: &Container,
    memories: &MemoryStore,
    entities: &EntityStore,
    relationships: &RelationshipStore,
    graph: &GraphStore,
) -> (u64, u64, u64, u64, u64) {
    (
        container.latest_version(),
        memories.memory_version(),
        entities.entity_version(),
        relationships.relationship_version(),
        graph.graph_version(),
    )
}
