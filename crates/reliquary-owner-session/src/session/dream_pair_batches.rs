use super::*;
impl DreamPairs {
    pub(crate) fn batch_fingerprint(&self, id: [u8; 32]) -> Option<[u8; 32]> {
        self.batch_fingerprints.get(&id).copied()
    }
    pub(crate) fn archival_origin(&self, source: MemoryId, before: u64) -> bool {
        self.batch_archivals
            .get(&source)
            .is_some_and(|version| *version >= before)
            || self.supersessions.values().any(|s| {
                s.relation().graph_version > before
                    && s.cut.change.target == NodeRef::Memory(source.0)
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
        if self.batch_fingerprints.contains_key(&batch.publication_id)
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
        for lifecycle in batch.lifecycles.into_iter().filter(|l| l.archived) {
            let latest = self
                .batch_archivals
                .entry(MemoryId(lifecycle.id))
                .or_insert(version);
            *latest = (*latest).max(version);
        }
        self.batch_fingerprints
            .insert(batch.publication_id, batch.fingerprint);
        Ok(())
    }
}
