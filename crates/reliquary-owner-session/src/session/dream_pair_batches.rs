use super::*;
impl DreamPairs {
    pub(crate) fn batch(
        &self,
        id: [u8; 32],
    ) -> Option<&crate::records::dream_batch::DreamBatchRecord> {
        self.batches.get(&id)
    }
    pub(crate) fn archival_origin(&self, source: MemoryId, before: u64) -> bool {
        self.batches.values().any(|b| {
            b.graph_version + b.graph_records.len() as u64 >= before
                && b.lifecycles.iter().any(|l| l.id == source.0 && l.archived)
        }) || self.supersessions.values().any(|s| {
            s.relation().graph_version > before && s.cut.change.target == NodeRef::Memory(source.0)
        })
    }
    pub(crate) fn duplicate_origin(&self, source: MemoryId, before: u64) -> bool {
        self.initial_links(source, before)
            .iter()
            .any(|(_, kind)| *kind == reliquary_graph::RelationKind::DuplicateOf)
    }
    pub(crate) fn initial_links(
        &self,
        source: MemoryId,
        before: u64,
    ) -> Vec<(MemoryId, reliquary_graph::RelationKind)> {
        self.initial
            .get(&source)
            .into_iter()
            .flat_map(|m| m.iter())
            .filter(|(_, (_, v))| *v > before)
            .map(|(id, (kind, _))| (*id, *kind))
            .collect()
    }
    pub(crate) fn apply_batch(
        &mut self,
        batch: crate::records::dream_batch::DreamBatchRecord,
    ) -> Result<(), OwnerSessionError> {
        if self.batches.contains_key(&batch.publication_id)
            || batch
                .pairs
                .iter()
                .any(|p| self.contains(MemoryId(p.source), MemoryId(p.candidate)))
        {
            return Err(invalid());
        }
        let version = batch.graph_version + batch.graph_records.len() as u64;
        for p in &batch.pairs {
            self.pairs
                .insert(ordered(MemoryId(p.source), MemoryId(p.candidate)));
            if p.direct {
                self.initial.entry(MemoryId(p.source)).or_default().insert(
                    MemoryId(p.candidate),
                    (
                        crate::records::dream_batch::decode_kind(p.kind)
                            .map_err(OwnerSessionError::Record)?
                            .ok_or_else(invalid)?,
                        version,
                    ),
                );
            }
        }
        self.batches.insert(batch.publication_id, batch);
        Ok(())
    }
}
