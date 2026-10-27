use crate::{ArchiveVectorId, ContentId, MemoryBodyId, PackedVectorId};
use reliquary_archive::archive_codec::ArchiveRecord;
use reliquary_container::ObjectRef;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(crate) struct ReclamationScan {
    external_index: crate::external_source::store::ExternalIndexStore,
    backing_candidates: HashSet<ObjectRef>,
    published_backing: HashSet<ObjectRef>,
    archive_content_refs: HashMap<ObjectRef, Vec<ContentId>>,
    content_chunks: Vec<(ObjectRef, ContentId)>,
    memory_body_refs: HashMap<ObjectRef, MemoryBodyId>,
    memory_body_chunks: Vec<(ObjectRef, MemoryBodyId)>,
    completion_body_refs: HashSet<MemoryBodyId>,
    memory_vector_body_refs: HashSet<MemoryBodyId>,
    generation_archive_vectors: HashMap<ObjectRef, ArchiveVectorId>,
    archive_vector_chunks: Vec<(ObjectRef, ArchiveVectorId, PackedVectorId)>,
    memory_vector_packed_refs: HashSet<PackedVectorId>,
    packed_vector_chunks: Vec<(ObjectRef, PackedVectorId)>,
    free_compaction_chunks: HashSet<ObjectRef>,
    external_snapshot_chunks: Vec<ObjectRef>,
    external_snapshot_heads: HashMap<String, ObjectRef>,
    external_discard: Option<ObjectRef>,
}

pub(crate) struct ReclamationPlan {
    pub(crate) drop: HashSet<ObjectRef>,
    pub(crate) unpublished_backing_chunks: usize,
    pub(crate) orphan_content_chunks: usize,
    pub(crate) orphan_memory_body_chunks: usize,
    pub(crate) unactivated_archive_vector_chunks: usize,
    pub(crate) unreferenced_packed_vector_chunks: usize,
    pub(crate) free_compaction_chunks: usize,
    pub(crate) superseded_external_index_chunks: usize,
}

impl ReclamationScan {
    pub(crate) fn ingest(&mut self, object: ObjectRef, payload: &[u8]) -> Result<(), String> {
        self.external_index.ingest(payload);
        if payload.starts_with(crate::external_source::compact_index::MAGIC) {
            self.external_snapshot_chunks.push(object);
            match crate::external_source::compact_index::decode(payload) {
                Ok(crate::external_source::compact_index::Record::Snapshot(state)) => {
                    self.external_snapshot_heads.insert(state.namespace, object);
                }
                Ok(crate::external_source::compact_index::Record::Discard) | Err(_) => {
                    self.external_snapshot_heads.clear();
                    self.external_discard = Some(object);
                }
            }
        } else if payload.starts_with(b"CVAXIDX") {
            self.external_snapshot_heads.clear();
            self.external_discard = Some(object);
        }
        if let Some(publication) = crate::external_source::publication::decode(payload)
            .map_err(|error| error.to_string())?
        {
            self.completion_body_refs
                .extend(publication.records.into_iter().map(|r| r.body_id));
            return Ok(());
        }
        if let Some(envelope) = reliquary_container::publication_envelope::decode(payload)
            .map_err(|error| error.to_string())?
        {
            let memory = crate::memory_compact_codec::publication::decode(
                envelope.body,
                envelope.global_version,
            )
            .map_err(|error| error.to_string())?;
            let archive = reliquary_archive::archive_codec::publication::decode(
                envelope.body,
                envelope.global_version,
            )
            .map_err(|error| error.to_string())?;
            if memory.is_none()
                && archive.is_none()
                && reliquary_perception_store::entity_compact_codec::publication::decode(
                    envelope.body,
                    envelope.global_version,
                )
                .map_err(|error| error.to_string())?
                .is_none()
                && reliquary_perception_store::relationship_compact_codec::publication::decode(
                    envelope.body,
                    envelope.global_version,
                )
                .map_err(|error| error.to_string())?
                .is_none()
                && crate::graph_compact_codec::publication::decode(
                    envelope.body,
                    envelope.global_version,
                )
                .map_err(|error| error.to_string())?
                .is_none()
            {
                return Err("unsupported publication envelope body during reclamation".to_owned());
            }
            // Publication envelopes are semantic carriers in their own
            // right. A compact Memory creation also keeps its immutable
            // body alive; deltas and tombstones do not replace that body.
            self.published_backing.insert(object);
            if let Some(
                reliquary_archive::archive_codec::publication::ArchivePublication::Record {
                    record,
                    ..
                },
            ) = archive
            {
                let refs = match record {
                    ArchiveRecord::Node(node) => vec![node.content_id],
                    ArchiveRecord::File(file) => vec![file.content_id],
                    ArchiveRecord::IngestedTurn(turn) => {
                        let mut refs = vec![turn.node.content_id];
                        refs.extend(turn.attachments.into_iter().map(|file| file.content_id));
                        refs
                    }
                    _ => Vec::new(),
                };
                if !refs.is_empty() {
                    self.archive_content_refs.insert(object, refs);
                }
            }
            if let Some(crate::memory_compact_codec::publication::MemoryPublication::Creation(
                creation,
            )) = memory
            {
                self.memory_body_refs
                    .insert(object, creation.record.body_id);
            }
            return Ok(());
        }
        // Native bodies outside envelopes are unpublished. This structural probe
        // does not allocate or admit any owner/global clock.
        if crate::memory_compact_codec::publication::decode(payload, 1)
            .map_err(|e| e.to_string())?
            .is_some()
            || (!payload.starts_with(b"CVAETM01")
                && reliquary_perception_store::entity_compact_codec::publication::decode(
                    payload, 1,
                )
                .map_err(|e| e.to_string())?
                .is_some())
            || reliquary_perception_store::relationship_compact_codec::publication::decode(
                payload, 1,
            )
            .map_err(|e| e.to_string())?
            .is_some()
            || (!payload.starts_with(b"CVAAFM")
                && !payload.starts_with(b"CVAARE")
                && !payload.starts_with(b"CVACONT1")
                && reliquary_archive::archive_codec::publication::decode(payload, 1)
                    .map_err(|e| e.to_string())?
                    .is_some())
            || ((payload.starts_with(b"CVAGCM") || payload.starts_with(b"CVAGCB"))
                && crate::graph_compact_codec::publication::decode(payload, 1)
                    .map_err(|e| e.to_string())?
                    .is_some())
        {
            self.backing_candidates.insert(object);
            return Ok(());
        }
        if let Some(version) = crate::vector_generation_codec::decode_version(payload)
            .map_err(|error| error.to_string())?
        {
            self.published_backing.insert(version.record);
            return Ok(());
        }

        if payload.starts_with(b"CVACONT1") {
            let ArchiveRecord::Content(id, _) =
                reliquary_archive::archive_codec::decode_record(payload)
                    .map_err(|error| error.to_string())?
            else {
                unreachable!()
            };
            self.content_chunks.push((object, id));
            return Ok(());
        }

        if let Some((id, _)) =
            crate::memory_codec::decode_body(payload).map_err(|error| error.to_string())?
        {
            self.memory_body_chunks.push((object, id));
            return Ok(());
        }
        if let Some(completion) =
            crate::insomnia::completion::decode_completion(payload).map_err(str::to_owned)?
        {
            self.completion_body_refs
                .extend(completion.records.into_iter().map(|record| record.body_id));
            return Ok(());
        }
        if let Some(generation) = crate::vector_generation_codec::decode_generation(payload)
            .map_err(|error| error.to_string())?
        {
            self.backing_candidates.insert(object);
            self.generation_archive_vectors
                .insert(object, generation.archive_vector_id);
            return Ok(());
        }
        if let Some(vectors) = reliquary_vector_store::decode_memory_vector_object(payload)
            .map_err(|error| error.to_string())?
        {
            self.memory_vector_packed_refs
                .insert(vectors.packed_vector_id);
            self.memory_vector_body_refs.extend(
                vectors
                    .memory_body_ids
                    .into_iter()
                    .map(|x| MemoryBodyId(x.0)),
            );
            return Ok(());
        }
        if let Some((id, packed_vector_id)) =
            reliquary_vector_store::archive_vector_object_packed_id(payload)
                .map_err(|error| error.to_string())?
        {
            self.archive_vector_chunks
                .push((object, id, packed_vector_id));
            return Ok(());
        }
        if let Some(packed) = reliquary_vector_store::codec::decode_object(payload)
            .map_err(|error| error.to_string())?
        {
            self.packed_vector_chunks.push((object, packed.id));
            return Ok(());
        }
        if let Some(compaction) = reliquary_archive::conversation_compaction_codec::decode(payload)
            .map_err(|error| error.to_string())?
            && matches!(
                compaction,
                reliquary_archive::conversation_compaction_codec::DecodedCompactionChunk::Free
            )
        {
            self.free_compaction_chunks.insert(object);
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> ReclamationPlan {
        let unpublished: HashSet<_> = self
            .backing_candidates
            .difference(&self.published_backing)
            .copied()
            .collect();
        let mut referenced_content = HashSet::new();
        let mut referenced_bodies = self.completion_body_refs;
        referenced_bodies.extend(self.memory_vector_body_refs);
        let mut activated_archive_vectors = HashSet::new();
        for published in &self.published_backing {
            if let Some(refs) = self.archive_content_refs.get(published) {
                referenced_content.extend(refs.iter().copied());
            }
            if let Some(body_id) = self.memory_body_refs.get(published) {
                referenced_bodies.insert(*body_id);
            }
            if let Some(archive_vector_id) = self.generation_archive_vectors.get(published) {
                activated_archive_vectors.insert(*archive_vector_id);
            }
        }

        let orphan_content: HashSet<_> = self
            .content_chunks
            .into_iter()
            .filter_map(|(object, id)| (!referenced_content.contains(&id)).then_some(object))
            .collect();
        let orphan_bodies: HashSet<_> = self
            .memory_body_chunks
            .into_iter()
            .filter_map(|(object, id)| (!referenced_bodies.contains(&id)).then_some(object))
            .collect();

        let mut referenced_packed = self.memory_vector_packed_refs;
        referenced_packed.extend(
            self.external_index
                .sources
                .values()
                .flat_map(|s| &s.messages)
                .flat_map(|m| &m.chunks)
                .map(|c| PackedVectorId(c.matrix)),
        );
        let unactivated_archive_vectors: HashSet<_> = self
            .archive_vector_chunks
            .iter()
            .filter_map(|(object, id, _)| {
                (!activated_archive_vectors.contains(id)).then_some(*object)
            })
            .collect();
        for (object, id, packed_id) in &self.archive_vector_chunks {
            if !unactivated_archive_vectors.contains(object)
                && activated_archive_vectors.contains(id)
            {
                referenced_packed.insert(*packed_id);
            }
        }
        let unreferenced_packed: HashSet<_> = self
            .packed_vector_chunks
            .into_iter()
            .filter_map(|(object, id)| (!referenced_packed.contains(&id)).then_some(object))
            .collect();

        let mut drop = unpublished.clone();
        drop.extend(orphan_content.iter().copied());
        drop.extend(orphan_bodies.iter().copied());
        drop.extend(unactivated_archive_vectors.iter().copied());
        drop.extend(unreferenced_packed.iter().copied());
        drop.extend(self.free_compaction_chunks.iter().copied());
        // Current snapshots stand alone, so only the latest per namespace and last
        // damage/discard barrier remain required. No semantic prune marker.
        let retained_external: HashSet<_> = self
            .external_snapshot_heads
            .values()
            .copied()
            .chain(self.external_discard)
            .collect();
        let superseded_external: HashSet<_> = self
            .external_snapshot_chunks
            .into_iter()
            .filter(|object| !retained_external.contains(object))
            .collect();
        drop.extend(superseded_external.iter().copied());

        ReclamationPlan {
            drop,
            unpublished_backing_chunks: unpublished.len(),
            orphan_content_chunks: orphan_content.len(),
            orphan_memory_body_chunks: orphan_bodies.len(),
            unactivated_archive_vector_chunks: unactivated_archive_vectors.len(),
            unreferenced_packed_vector_chunks: unreferenced_packed.len(),
            free_compaction_chunks: self.free_compaction_chunks.len(),
            superseded_external_index_chunks: superseded_external.len(),
        }
    }
}
