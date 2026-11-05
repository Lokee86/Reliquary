//! Rebuildable owner-local discovery index. Queries hydrate a bounded pool.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug)]
pub struct DreamCandidatePool {
    pub memory_version: u64,
    pub ids: Vec<MemoryId>,
    pub index_entries_examined: usize,
}
pub(super) struct DreamReadIndex {
    pub source_terms: BTreeMap<MemoryId, BTreeSet<String>>,
    pub locations: BTreeMap<MemoryId, (u64, Option<i64>, Vec<([u8; 32], u8, u16)>)>,
    pub version: u64,
    pub active: BTreeSet<MemoryId>,
    pub recent: BTreeSet<(u64, MemoryId)>,
    pub times: BTreeSet<(i64, MemoryId)>,
    pub terms: BTreeMap<String, BTreeSet<MemoryId>>,
    pub semantic: BTreeMap<([u8; 32], u8, u16), BTreeSet<MemoryId>>,
}
impl DreamReadIndex {
    pub fn build(
        memories: &MemoryStore,
        packed: &PackedVectorStore,
        profiles: &CompatibilityProfileStore,
        vectors: &MemoryVectorStore,
    ) -> Result<Self, OwnerSessionError> {
        let mut index = Self {
            source_terms: BTreeMap::new(),
            locations: BTreeMap::new(),
            version: memories.memory_version(),
            active: BTreeSet::new(),
            recent: BTreeSet::new(),
            times: BTreeSet::new(),
            terms: BTreeMap::new(),
            semantic: BTreeMap::new(),
        };
        for id in memories.memory_ids() {
            index.refresh(memories, packed, profiles, vectors, &[id])?;
        }
        Ok(index)
    }
    pub fn refresh(
        &mut self,
        memories: &MemoryStore,
        packed: &PackedVectorStore,
        profiles: &CompatibilityProfileStore,
        vectors: &MemoryVectorStore,
        ids: &[MemoryId],
    ) -> Result<(), OwnerSessionError> {
        for id in ids {
            self.remove(*id);
            let Ok(current) = memories.current(*id) else {
                continue;
            };
            let record = &current.record;
            if record.archived {
                continue;
            }
            let reliquary_memory_core::PayloadState::Available(body) = &current.payload else {
                continue;
            };
            let birth = memories
                .first_memory_version(*id)
                .ok_or(OwnerSessionError::StaleDreamCut)?;
            self.active.insert(*id);
            self.recent.insert((birth, *id));
            if let Some(time) = record.source_time_ns {
                self.times.insert((time, *id));
            }
            let terms = tokens(&format!(
                "{} {} {} {}",
                body.title, body.content, record.category, record.memory_type
            ));
            self.source_terms.insert(*id, terms.clone());
            for term in terms {
                self.terms.entry(term).or_default().insert(*id);
            }
            let mut keys = Vec::new();
            for profile in profiles.iter().filter(|p| !profiles.is_retired(p.id)) {
                if let Some(location) = vectors.location(
                    profile.id,
                    reliquary_vector_store::MemoryBodyId(record.body_id.0),
                ) {
                    let mapping = vectors
                        .info(location.set_id)
                        .ok_or(MemoryVectorError::MissingObject)?;
                    let Ok(row) = packed.read_f32_row(mapping.packed_vector_id, location.row)
                    else {
                        continue;
                    };
                    if row.iter().any(|v| !v.is_finite()) {
                        continue;
                    }
                    for table in 0..4 {
                        let key = (profile.id.0, table, locality(&row, table));
                        self.semantic.entry(key).or_default().insert(*id);
                        keys.push(key);
                    }
                }
            }
            self.locations
                .insert(*id, (birth, record.source_time_ns, keys));
        }
        self.version = memories.memory_version();
        Ok(())
    }
    fn remove(&mut self, id: MemoryId) {
        self.active.remove(&id);
        if let Some((birth, time, keys)) = self.locations.remove(&id) {
            self.recent.remove(&(birth, id));
            if let Some(time) = time {
                self.times.remove(&(time, id));
            }
            for key in keys {
                remove(&mut self.semantic, &key, id);
            }
        }
        if let Some(terms) = self.source_terms.remove(&id) {
            for term in terms {
                remove(&mut self.terms, &term, id);
            }
        }
    }
}
pub(super) fn tokens(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.len() > 1)
        .take(512)
        .map(str::to_lowercase)
        .take(256)
        .collect()
}
pub(super) fn locality(row: &[f32], table: u8) -> u16 {
    if row.is_empty() {
        return 0;
    }
    let mut code = 0;
    for bit in 0..12 {
        let seed = (bit as u64 + 1) + u64::from(table) * 97;
        let a = row[(mix(seed) % row.len() as u64) as usize];
        let b = row[(mix(seed + 1009) % row.len() as u64) as usize];
        if a + if mix(seed + 2017) & 1 == 0 { b } else { -b } >= 0.0 {
            code |= 1 << bit;
        }
    }
    code
}
#[path = "dream_read_query.rs"]
mod query;

fn remove<K: Ord>(map: &mut BTreeMap<K, BTreeSet<MemoryId>>, key: &K, id: MemoryId) {
    if let Some(ids) = map.get_mut(key) {
        ids.remove(&id);
        if ids.is_empty() {
            map.remove(key);
        }
    }
}

fn mix(mut v: u64) -> u64 {
    v = v.wrapping_add(0x9e3779b97f4a7c15);
    v = (v ^ (v >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    v = (v ^ (v >> 27)).wrapping_mul(0x94d049bb133111eb);
    v ^ (v >> 31)
}
