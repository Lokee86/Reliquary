use crate::archive_codec::ArchiveRecord;
use crate::{ArchiveVectorId, ContentId, MemoryBodyId, ObjectRef, PackedVectorId};
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
}

pub(crate) struct ReclamationPlan {
    pub(crate) drop: HashSet<ObjectRef>,
    pub(crate) unpublished_backing_chunks: usize,
    pub(crate) orphan_content_chunks: usize,
    pub(crate) orphan_memory_body_chunks: usize,
    pub(crate) unactivated_archive_vector_chunks: usize,
    pub(crate) unreferenced_packed_vector_chunks: usize,
    pub(crate) free_compaction_chunks: usize,
}

impl ReclamationScan {
    pub(crate) fn ingest(&mut self, object: ObjectRef, payload: &[u8]) -> Result<(), String> {
        self.external_index.ingest(payload);
        if let Some(publication) = crate::external_source::publication::decode(payload)
            .map_err(|error| error.to_string())?
        {
            self.completion_body_refs
                .extend(publication.records.into_iter().map(|r| r.body_id));
            return Ok(());
        }
        if let Some(version) = crate::archive_history_codec::decode_record_version(payload)
            .map_err(|error| error.to_string())?
        {
            self.published_backing.insert(version.record);
            return Ok(());
        }
        if let Some(version) =
            crate::memory_codec::decode_version(payload).map_err(|error| error.to_string())?
        {
            self.published_backing.insert(version.record);
            return Ok(());
        }
        if let Some(version) =
            crate::entity_codec::decode_version(payload).map_err(|error| error.to_string())?
        {
            self.published_backing.insert(version.record);
            return Ok(());
        }
        if let Some(version) =
            crate::graph_codec::decode_version(payload).map_err(|error| error.to_string())?
        {
            self.published_backing.insert(version.mutation);
            return Ok(());
        }
        if let Some(version) =
            crate::relationship_codec::decode_version(payload).map_err(|error| error.to_string())?
        {
            self.published_backing.insert(version.record);
            return Ok(());
        }
        if let Some(version) = crate::vector_generation_codec::decode_version(payload)
            .map_err(|error| error.to_string())?
        {
            self.published_backing.insert(version.record);
            return Ok(());
        }

        match crate::archive_codec::decode_record(payload).map_err(|error| error.to_string())? {
            ArchiveRecord::Content(id, _) => {
                self.content_chunks.push((object, id));
                return Ok(());
            }
            ArchiveRecord::Node(node) => {
                self.backing_candidates.insert(object);
                self.archive_content_refs
                    .insert(object, vec![node.content_id]);
                return Ok(());
            }
            ArchiveRecord::File(file) => {
                self.backing_candidates.insert(object);
                self.archive_content_refs
                    .insert(object, vec![file.content_id]);
                return Ok(());
            }
            ArchiveRecord::IngestedTurn(turn) => {
                self.backing_candidates.insert(object);
                let mut refs = Vec::with_capacity(1 + turn.attachments.len());
                refs.push(turn.node.content_id);
                refs.extend(turn.attachments.into_iter().map(|file| file.content_id));
                self.archive_content_refs.insert(object, refs);
                return Ok(());
            }
            ArchiveRecord::ImportReceipt(_)
            | ArchiveRecord::Branch(_)
            | ArchiveRecord::ConversationMetadata(_)
            | ArchiveRecord::Fragment(_)
            | ArchiveRecord::Episode(_)
            | ArchiveRecord::FileMemoryLink(_) => {
                self.backing_candidates.insert(object);
                return Ok(());
            }
            ArchiveRecord::Other => {}
        }

        if let Some((id, _)) =
            crate::memory_codec::decode_body(payload).map_err(|error| error.to_string())?
        {
            self.memory_body_chunks.push((object, id));
            return Ok(());
        }
        if let Some(record) =
            crate::memory_codec::decode_record(payload).map_err(|error| error.to_string())?
        {
            self.backing_candidates.insert(object);
            self.memory_body_refs.insert(object, record.body_id);
            return Ok(());
        }
        if let Some(completion) =
            crate::insomnia::completion::decode_completion(payload).map_err(str::to_owned)?
        {
            self.completion_body_refs
                .extend(completion.records.into_iter().map(|record| record.body_id));
            return Ok(());
        }
        if crate::entity_codec::decode_record(payload)
            .map_err(|error| error.to_string())?
            .is_some()
            || crate::entity_codec::decode_tombstone(payload)
                .map_err(|error| error.to_string())?
                .is_some()
        {
            self.backing_candidates.insert(object);
            return Ok(());
        }
        if crate::graph_codec::decode_batch(payload)
            .map_err(|error| error.to_string())?
            .is_some()
            || crate::graph_codec::decode_mutation(payload)
                .map_err(|error| error.to_string())?
                .is_some()
        {
            self.backing_candidates.insert(object);
            return Ok(());
        }
        if crate::relationship_codec::decode_record(payload)
            .map_err(|error| error.to_string())?
            .is_some()
        {
            self.backing_candidates.insert(object);
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
        if let Some(vectors) =
            crate::memory_vector_codec::decode_object(payload).map_err(|error| error.to_string())?
        {
            self.memory_vector_packed_refs
                .insert(vectors.packed_vector_id);
            self.memory_vector_body_refs.extend(vectors.memory_body_ids);
            return Ok(());
        }
        if let Some(vectors) = crate::archive_vector_codec::decode_object(payload)
            .map_err(|error| error.to_string())?
        {
            self.archive_vector_chunks
                .push((object, vectors.id, vectors.packed_vector_id));
            return Ok(());
        }
        if let Some(packed) =
            crate::packed_vector_codec::decode_object(payload).map_err(|error| error.to_string())?
        {
            self.packed_vector_chunks.push((object, packed.id));
            return Ok(());
        }
        if let Some(compaction) = crate::conversation_compaction_codec::decode(payload)
            .map_err(|error| error.to_string())?
            && matches!(
                compaction,
                crate::conversation_compaction_codec::DecodedCompactionChunk::Free
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

        ReclamationPlan {
            drop,
            unpublished_backing_chunks: unpublished.len(),
            orphan_content_chunks: orphan_content.len(),
            orphan_memory_body_chunks: orphan_bodies.len(),
            unactivated_archive_vector_chunks: unactivated_archive_vectors.len(),
            unreferenced_packed_vector_chunks: unreferenced_packed.len(),
            free_compaction_chunks: self.free_compaction_chunks.len(),
        }
    }
}
