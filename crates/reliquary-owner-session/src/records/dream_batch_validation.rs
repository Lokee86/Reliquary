use super::*;
use std::collections::BTreeSet;
impl DreamBatchRecord {
    pub(super) fn validate(&self) -> Result<(), String> {
        if self.publication_id == [0; 32]
            || self.observed.is_empty()
            || self.observed.len() > 128
            || self.pairs.len() > 16
            || self.graph_records.len() > 256
            || self.lifecycles.len() > 128
            || (self.pairs.is_empty()
                && self.graph_records.is_empty()
                && self.lifecycles.is_empty())
        {
            return Err("invalid Dream batch bounds".into());
        }
        let ids: BTreeSet<_> = self.observed.iter().map(|o| o.id).collect();
        if ids.len() != self.observed.len() || self.observed.iter().any(|o| o.revision == 0) {
            return Err("duplicate Dream batch observations".into());
        }
        let mut pairs = BTreeSet::new();
        for pair in &self.pairs {
            decode_kind(pair.kind)?;
            if pair.source != self.observed[0].id
                || !ids.contains(&pair.candidate)
                || pair.source == pair.candidate
                || !pairs.insert(pair.candidate)
                || (pair.rejected && pair.kind != 0)
                || (pair.direct && (pair.kind == 0 || pair.rejected))
            {
                return Err("invalid Dream batch pair evidence".into());
            }
        }
        let mut touched = BTreeSet::new();
        for (index, bytes) in self.graph_records.iter().enumerate() {
            let r = reliquary_graph::dream_relation_in_frame(bytes, self.owner)
                .map_err(|e| e.to_string())?;
            if r.graph_version
                != self
                    .graph_version
                    .checked_add(index as u64 + 1)
                    .ok_or("Graph version exhausted")?
                || r.origin != reliquary_graph::RelationOrigin::Dream
            {
                return Err("invalid Dream batch Graph cut".into());
            }
            for endpoint in [r.source, r.target] {
                if !matches!(endpoint,reliquary_graph::NodeRef::Memory(id) if ids.contains(&id)) {
                    return Err("Dream batch endpoint outside observation cut".into());
                }
            }
        }
        for l in &self.lifecycles {
            if !touched.insert(l.id)
                || !self
                    .observed
                    .iter()
                    .any(|o| o.id == l.id && o.revision == l.revision)
                || l.delta.len() > 65535
                || l.updated_at_ns != self.updated_at_ns
                || l.mutation != format!("dream-batch:{}:{}", hex(&self.publication_id), hex(&l.id))
            {
                return Err("invalid Dream lifecycle evidence".into());
            }
        }
        if fingerprint(&self.request()?)? != self.fingerprint {
            return Err("Dream batch request digest mismatch".into());
        }
        Ok(())
    }
}
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
