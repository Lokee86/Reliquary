//! Explicit destructive pruning of terminal Archive conversation bodies.
use crate::archive::history_prune::PrunedArchiveContent;
use crate::archive_codec::{ArchiveRecord, decode_record};
use crate::owner_lifecycle::LifecycleTarget;
use crate::owner_maintenance::HistoryPruneManifest;
use crate::{ContentId, Cva, CvaError, ObjectRef};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HistoryPruneRequest {
    pub targets: Vec<LifecycleTarget>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryPruneReport {
    pub pruned_conversations: usize,
    pub pruned_content_objects: usize,
    pub pruned_memory_bodies: usize,
    pub pruned_packed_vectors: usize,
    pub destroyed_body_bytes: u64,
    pub destroyed_packed_bytes: u64,
    pub source_file_bytes: u64,
    pub output_file_bytes: u64,
    pub manifest_digest: [u8; 32],
}

impl Cva {
    /// Write a separately validated copy with explicitly selected terminal conversation bodies removed.
    /// Other owner payload domains remain protected until their own retention contracts are wired.
    pub fn prune_tombstoned_history(
        &mut self,
        output_path: impl AsRef<Path>,
        request: HistoryPruneRequest,
    ) -> Result<HistoryPruneReport, CvaError> {
        self.container.require_storage_format_writable()?;
        let output_path = output_path.as_ref();
        self.container.sync()?;
        if self.open_import_epoch().is_some() {
            return Err(prune_error("cannot prune while an import epoch is open"));
        }
        if self
            .freshness
            .import_settlements()
            .iter()
            .any(|s| s.semantic.is_none())
        {
            return Err(prune_error(
                "normalize legacy import settlements on a disposable copy before pruning",
            ));
        }
        self.validate_historical_import_settlements()?;
        let source = Cva::open(self.container.path())?;
        if source.latest_global_version() != self.latest_global_version() {
            return Err(prune_error("source changed before prune admission"));
        }
        drop(source);
        if !self.owner_maintenance.is_empty() {
            return Err(prune_error(
                "additional pruning of an already pruned owner is unsupported",
            ));
        }
        let owner_uuid = self
            .owner_uuid()
            .ok_or_else(|| prune_error("pruning requires a durable owner UUID"))?;
        let watermark = self.latest_global_version();
        let mut selected = BTreeSet::new();
        let mut selected_profiles = HashSet::new();
        let mut selected_memories = HashSet::new();
        let mut description_targets = Vec::new();
        for target in request.targets {
            match target {
                LifecycleTarget::Conversation(id) => {
                    if !selected.insert(id) {
                        return Err(prune_error("duplicate prune target"));
                    }
                }
                target @ (LifecycleTarget::Entity(_) | LifecycleTarget::Relationship(_)) => {
                    description_targets.push(target);
                }
                LifecycleTarget::Memory(id) => {
                    if !selected_memories.insert(id) {
                        return Err(prune_error("duplicate prune target"));
                    }
                }
                LifecycleTarget::CompatibilityProfile(id) => {
                    if !selected_profiles.insert(id) {
                        return Err(prune_error("duplicate prune target"));
                    }
                }
                _ => {
                    return Err(prune_error(
                        "selected domain has no implemented retention contract",
                    ));
                }
            }
        }
        if selected.is_empty()
            && selected_profiles.is_empty()
            && selected_memories.is_empty()
            && description_targets.is_empty()
        {
            return Err(prune_error(
                "pruning requires explicitly selected terminal tombstones",
            ));
        }
        for id in &selected {
            if self.conversation_tombstone(id).is_none() {
                return Err(prune_error(
                    "prune target is not a terminal conversation tombstone",
                ));
            }
        }

        let eligible_packed = self.prunable_packed_vector_profiles();
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
        for version in self.archive.record_versions() {
            match self
                .archive
                .read_publication(&mut self.container, *version)?
                .0
            {
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

        let objects = self.container.chunks()?;
        let mut lengths = HashMap::<ContentId, u64>::new();
        let mut occurrences = HashMap::<ContentId, usize>::new();
        for object in &objects {
            let payload = self.container.read(*object)?;
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
            let retained = self.memories.retention_descriptor(*id)?;
            let mut bodies = Vec::new();
            for body in retained.prunable_body_ids {
                if self
                    .memories
                    .records()
                    .iter()
                    .any(|record| record.body_id == body && !selected_memories.contains(&record.id))
                {
                    return Err(prune_error(
                        "Memory body is shared outside selected tombstones",
                    ));
                }
                let object = self
                    .memories
                    .prune_body_chunk(body)
                    .ok_or_else(|| prune_error("Memory body is not independently removable"))?;
                let raw = self.container.read(object)?;
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
            memory_descriptors.push(crate::memory_store::history_prune::descriptor(
                &self.memories,
                *id,
                &bodies,
            )?);
        }
        let mut descriptors = selected
            .iter()
            .map(|id| {
                self.archive
                    .retention_descriptor(&mut self.container, id, watermark, &lengths)
            })
            .collect::<Result<Vec<_>, _>>()?;
        descriptors.extend(memory_descriptors);
        for target in &description_targets {
            descriptors.push(self.description_retention(target)?);
        }
        for (packed_id, profile_id) in &selected_packed {
            descriptors.push(self.packed_vector_retention_descriptor(
                *profile_id,
                *packed_id,
                watermark,
            )?);
        }
        descriptors.sort_by(|a, b| (&a.target, &a.payloads).cmp(&(&b.target, &b.payloads)));
        let manifest = HistoryPruneManifest {
            owner_uuid,
            source_global_version: watermark,
            source_history_digest: crate::owner_maintenance::owner_history_digest(
                &mut self.container,
                watermark,
                &self.graph,
            )
            .map_err(CvaError::Repack)?,
            descriptors,
        };
        let manifest_bytes = manifest.encode().map_err(CvaError::Repack)?;
        let manifest_digest = manifest.digest().map_err(CvaError::Repack)?;
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
            .map(|(id, _)| Ok((*id, self.packed_vector_prune_marker(*id, manifest_digest)?)))
            .collect::<Result<HashMap<_, _>, CvaError>>()?;
        let mut packed_objects = HashMap::new();
        for object in &objects {
            let payload = self.container.read(*object)?;
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
            &self.entities,
            &mut self.container,
            &selected_entities,
            manifest_digest,
        )?;
        let mut description_objects = entity_plan.replacements;
        let selected_relationships = description_targets
            .iter()
            .filter_map(|target| match target {
                LifecycleTarget::Relationship(id) => Some(*id),
                _ => None,
            })
            .collect::<HashSet<_>>();
        let relationship_plan = crate::relationship_description_prune::prepare_description_prune(
            &self.relationships,
            &mut self.container,
            &selected_relationships,
            manifest_digest,
        )?;
        description_objects.extend(relationship_plan.replacements);
        let source_file_bytes = fs::metadata(self.container.path())
            .map_err(|e| prune_error(&e.to_string()))?
            .len();
        let mut output = self.container.create_empty_like(output_path)?;
        let copy = (|| {
            let mut relocated = BTreeMap::<ObjectRef, ObjectRef>::new();
            let mut marked = HashSet::new();
            for object in &objects {
                if entity_plan.discard.contains(object)
                    || relationship_plan.discard.contains(object)
                {
                    continue;
                }
                let raw = self.container.read(*object)?;
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
                    crate::storage_reclamation::relocate_payload(&raw, &relocated)
                        .map_err(CvaError::Repack)?
                };
                let transformed =
                    crate::imports::legacy_epoch_transport::rebind_copy(&transformed, &output)
                        .map_err(CvaError::Import)?;
                let destination = output.append(&transformed)?;
                relocated.insert(*object, destination);
            }
            output.append(&manifest_bytes)?;
            output.sync()?;
            Ok::<_, CvaError>(relocated)
        })();
        let relocated = match copy {
            Ok(value) => value,
            Err(error) => {
                drop(output);
                let _ = fs::remove_file(output_path);
                return Err(error);
            }
        };
        drop(output);

        let verified = (|| {
            let mut reopened = Cva::open(output_path)?;
            self.validate_historical_import_copy(&reopened)?;
            if prune_clocks(self) != prune_clocks(&reopened)
                || self.owner_uuid() != reopened.owner_uuid()
                || !self.archive.activity_history_matches(&reopened.archive)
            {
                return Err(prune_error(
                    "pruned copy changed owner clocks or activity history",
                ));
            }
            for global in 1..=watermark {
                if self.transaction_time_ns(global) != reopened.transaction_time_ns(global) {
                    return Err(prune_error("pruned copy changed transaction times"));
                }
            }
            for object in &objects {
                if entity_plan.discard.contains(object)
                    || relationship_plan.discard.contains(object)
                {
                    continue;
                }
                let raw = self.container.read(*object)?;
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
                let expected = crate::storage_reclamation::relocate_payload(&raw, &relocated)
                    .map_err(CvaError::Repack)?;
                let destination = relocated
                    .get(object)
                    .ok_or_else(|| prune_error("nonselected raw chunk was lost"))?;
                let actual = reopened.container.read(*destination)?;
                let expected_proof =
                    crate::imports::legacy_epoch_transport::original_payload(&expected)
                        .map_err(CvaError::Import)?;
                let actual_proof =
                    crate::imports::legacy_epoch_transport::original_payload(&actual)
                        .map_err(CvaError::Import)?;
                if expected_proof.unwrap_or(&expected) != actual_proof.unwrap_or(&actual) {
                    return Err(prune_error("nonselected raw chunk changed during prune"));
                }
            }
            for node in self
                .archive
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
                let _ = fs::remove_file(output_path);
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
            let bytes = self
                .packed_vectors
                .info(*id)
                .ok_or_else(|| prune_error("packed vector info is missing"))?
                .byte_len;
            sum.checked_add(bytes)
                .ok_or_else(|| prune_error("pruned packed byte length overflow"))
        })?;
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
}

fn prune_clocks(cva: &Cva) -> [u64; 8] {
    [
        cva.latest_global_version(),
        cva.archive_version(),
        cva.memory_version(),
        cva.entity_version(),
        cva.relationship_version(),
        cva.graph_version(),
        cva.memory_graph_version(),
        cva.vector_version(),
    ]
}

fn prune_error(message: &str) -> CvaError {
    CvaError::Repack(message.to_owned())
}

#[cfg(test)]
#[path = "history_prune_tests.rs"]
mod tests;
