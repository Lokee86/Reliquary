//! Versioned authoritative import checkpoint inside the existing Freshness owner.
use super::{FreshnessStore, JournalEntry, Reader, for_each_event, put_bytes, put_record};
use crate::{
    MemoryId,
    freshness::{FreshnessPolicy, FreshnessRecord},
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
pub(crate) const IMPORT_MAGIC: &[u8; 8] = b"CVAFRS03";
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ImportCheckpoint {
    pub owner: [u8; 16],
    pub import_id: [u8; 32],
    pub corpus_digest: [u8; 32],
    pub manifest_digest: [u8; 32],
    pub input_digest: [u8; 32],
    pub cohort: BTreeSet<MemoryId>,
    pub cut: u64,
    pub global_version: u64,
    pub input_bytes: u64,
    pub archive_version: u64,
    pub graph_version: u64,
    pub memory_graph_version: u64,
    pub records: BTreeMap<MemoryId, FreshnessRecord>,
    pub superseded: BTreeSet<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (FreshnessStore, ImportCheckpoint, Vec<u8>) {
        let owner = [1; 16];
        let id = MemoryId([2; 32]);
        let mut store = FreshnessStore::new(Some(owner));
        store
            .ingest(&super::super::encode_policy(
                owner,
                FreshnessPolicy::default_v1(),
            ))
            .unwrap();
        let mut birth = FreshnessRecord::created();
        birth.admit(1).unwrap();
        store
            .ingest(&store.encode_initialize(owner, id, 1, birth))
            .unwrap();
        let proof = crate::FreshnessEventProof {
            graph_version: 0,
            community_generation: None,
            community_graph_version: None,
        };
        let mut post = birth;
        post.reinforce(50, 1001).unwrap();
        let event = store.encode_event(
            owner,
            crate::dream_freshness::initial_pass_id(owner, id),
            1001,
            proof,
            BTreeMap::from([(id, 50)]),
            BTreeMap::from([(id, post)]),
        );
        store.ingest(&event).unwrap();
        let mut replacement = birth;
        replacement
            .settle_with_policy(2001, &FreshnessPolicy::default_v1())
            .unwrap();
        let corpus_digest = [3; 32];
        let cp = ImportCheckpoint {
            owner,
            import_id: import_id(owner, corpus_digest),
            corpus_digest,
            manifest_digest: [4; 32],
            input_digest: [5; 32],
            cohort: BTreeSet::from([id]),
            cut: 2001,
            global_version: 1,
            input_bytes: 1,
            archive_version: 1,
            graph_version: 0,
            memory_graph_version: 0,
            records: BTreeMap::from([(id, replacement)]),
            superseded: BTreeSet::from([crate::dream_freshness::initial_pass_id(owner, id)]),
        };
        (store, cp, event)
    }
    #[test]
    fn checkpoint_credit_excluded_replacement_is_canonical_and_roundtrips() {
        let (mut store, mut cp, _) = fixture();
        let id = *cp.records.keys().next().unwrap();
        cp.records.insert(id, FreshnessRecord::created());
        let bytes = cp.encode();
        assert_eq!(ImportCheckpoint::decode(&bytes).unwrap(), cp);
        store.ingest_import_checkpoint(&bytes).unwrap();
        assert_eq!(store.records()[&id], FreshnessRecord::created());
        assert_eq!(
            store.records_at(cp.cut).unwrap()[&id],
            FreshnessRecord::created()
        );
        let mut corrupt = cp;
        corrupt.records.get_mut(&id).unwrap().score = 99;
        assert!(ImportCheckpoint::decode(&corrupt.encode()).is_err());
    }
    #[test]
    fn checkpoint_corruption_and_conflict_never_partially_replace_records() {
        let (mut store, cp, _) = fixture();
        let bytes = cp.encode();
        assert_eq!(ImportCheckpoint::decode(&bytes).unwrap(), cp);
        let original = store.records().clone();
        for end in 0..bytes.len() {
            assert!(store.ingest_import_checkpoint(&bytes[..end]).is_err());
            assert_eq!(store.records(), &original);
        }
        for at in 0..bytes.len() {
            let mut corrupt = bytes.clone();
            corrupt[at] ^= 1;
            assert!(store.ingest_import_checkpoint(&corrupt).is_err());
            assert_eq!(store.records(), &original);
        }
        store.ingest(&bytes).unwrap();
        let mut conflict = cp.clone();
        conflict.manifest_digest = [9; 32];
        assert!(store.ingest(&conflict.encode()).is_err());
        assert_eq!(store.records(), &cp.records);
    }
    #[test]
    fn legacy_semantic_watermark_does_not_block_checkpoint_without_completed_live_history() {
        let (mut store, cp, _) = fixture();
        store.semantic_activation_seen = true;
        store.ingest_import_checkpoint(&cp.encode()).unwrap();
        assert_eq!(store.import_checkpoint(), Some(&cp));

        let (mut completed, cp, _) = fixture();
        completed.semantic_completed = true;
        assert!(completed.ingest_import_checkpoint(&cp.encode()).is_err());
    }

    #[test]
    fn checkpoint_supersedes_scoring_but_retains_receipt_identity_and_negative_scores() {
        let (mut store, mut cp, old_event) = fixture();
        // Entity/Archive topology may advance the full Graph without advancing
        // the Memory Graph used by graph-qualified Freshness reads.
        cp.graph_version = 10;
        store.ingest(&cp.encode()).unwrap();
        assert_eq!(cp.records.values().next().unwrap().score, -100);
        assert_eq!(store.records_at(cp.cut).unwrap(), cp.records);
        assert_eq!(
            store
                .records_at_cut(&crate::FreshnessEventCut {
                    rel_turn: cp.cut,
                    event_id_inclusive: None,
                    graph_version: 0
                })
                .unwrap(),
            cp.records
        );
        store.ingest(&old_event).unwrap();
        assert_eq!(store.records_at(cp.cut).unwrap(), cp.records);
        assert!(store.records_at(cp.cut - 1).is_err());
        assert!(store.replay_entries().is_err());
        // An exact checkpoint retry cannot overwrite a later resident projection.
        let id = *cp.records.keys().next().unwrap();
        let mut live = cp.records[&id];
        live.reinforce(25, cp.cut + 1).unwrap();
        let event = store
            .prepare_accepted_use_batch(
                cp.owner,
                "post-checkpoint-use".into(),
                cp.cut + 1,
                vec![id],
                crate::FreshnessEventProof {
                    graph_version: 0,
                    community_generation: None,
                    community_graph_version: None,
                },
                vec![BTreeMap::from([(id, 25)])],
            )
            .unwrap();
        store.ingest(&event).unwrap();
        store.ingest(&cp.encode()).unwrap();
        assert_eq!(store.records_at(cp.cut + 1).unwrap()[&id], live);
    }
}
impl ImportCheckpoint {
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut out = IMPORT_MAGIC.to_vec();
        out.extend(self.owner);
        for d in [
            self.import_id,
            self.corpus_digest,
            self.manifest_digest,
            self.input_digest,
            crate::freshness::historical_planner::HISTORICAL_POLICY_DIGEST,
            crate::freshness::semantic_policy::SemanticPolicy::default()
                .digest()
                .expect("frozen F8 policy"),
        ] {
            out.extend(d);
        }
        for n in [
            self.cut,
            self.global_version,
            self.input_bytes,
            self.archive_version,
            self.graph_version,
            self.memory_graph_version,
        ] {
            out.extend(n.to_le_bytes());
        }
        out.extend((self.cohort.len() as u32).to_le_bytes());
        for id in &self.cohort {
            out.extend(id.0);
        }
        out.extend((self.records.len() as u32).to_le_bytes());
        for (id, record) in &self.records {
            out.extend(id.0);
            put_record(&mut out, *record);
        }
        out.extend((self.superseded.len() as u32).to_le_bytes());
        for id in &self.superseded {
            put_bytes(&mut out, id.as_bytes());
        }
        let checksum = Sha256::digest(&out);
        out.extend(checksum);
        out
    }
    pub(crate) fn decode(payload: &[u8]) -> Result<Self, String> {
        if payload.len() < 32 || payload.get(..8) != Some(IMPORT_MAGIC.as_slice()) {
            return Err("invalid import checkpoint framing".into());
        }
        let end = payload.len() - 32;
        if Sha256::digest(&payload[..end]).as_slice() != &payload[end..] {
            return Err("import checkpoint checksum mismatch".into());
        }
        let mut r = Reader {
            bytes: &payload[..end],
            at: 8,
        };
        let owner = r.array()?;
        let import_id = r.array()?;
        let corpus_digest = r.array()?;
        let manifest_digest = r.array()?;
        let input_digest = r.array()?;
        if r.array::<32>()? != crate::freshness::historical_planner::HISTORICAL_POLICY_DIGEST
            || r.array::<32>()?
                != crate::freshness::semantic_policy::SemanticPolicy::default()
                    .digest()
                    .map_err(|e| e.to_string())?
        {
            return Err("unsupported import reconstruction policy".into());
        }
        let cut = r.u64()?;
        let global_version = r.u64()?;
        let input_bytes = r.u64()?;
        let archive_version = r.u64()?;
        let graph_version = r.u64()?;
        let memory_graph_version = r.u64()?;
        let mut cohort = BTreeSet::new();
        let mut previous = None;
        for _ in 0..r.u32()? {
            let id = MemoryId(r.array()?);
            if previous.is_some_and(|old| old >= id) {
                return Err("unordered full import cohort".into());
            }
            previous = Some(id);
            cohort.insert(id);
        }
        let mut records = BTreeMap::new();
        let mut previous = None;
        for _ in 0..r.u32()? {
            let id = MemoryId(r.array()?);
            if previous.is_some_and(|old| old >= id) {
                return Err("unordered import cohort".into());
            }
            previous = Some(id);
            records.insert(id, r.record()?);
        }
        let mut superseded = BTreeSet::new();
        let mut previous: Option<String> = None;
        for _ in 0..r.u32()? {
            let id = String::from_utf8(r.bytes()?).map_err(|e| e.to_string())?;
            if id.is_empty() || previous.as_ref().is_some_and(|old| old >= &id) {
                return Err("unordered import scoring identities".into());
            }
            previous = Some(id.clone());
            superseded.insert(id);
        }
        if r.at != end {
            return Err("trailing import checkpoint bytes".into());
        }
        let cp = Self {
            owner,
            import_id,
            corpus_digest,
            manifest_digest,
            input_digest,
            cohort,
            cut,
            global_version,
            input_bytes,
            archive_version,
            graph_version,
            memory_graph_version,
            records,
            superseded,
        };
        cp.validate()?;
        Ok(cp)
    }
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.cut == 0
            || self.records.is_empty()
            || self.cohort.is_empty()
            || self.cohort.len() > u32::MAX as usize
            || self.records.keys().any(|id| !self.cohort.contains(id))
            || self.records.len() > u32::MAX as usize
            || self.superseded.len() > u32::MAX as usize
            || self.import_id != import_id(self.owner, self.corpus_digest)
            || self.manifest_digest == [0; 32]
            || self.memory_graph_version > self.graph_version
        {
            return Err("invalid import checkpoint identity/cohort".into());
        }
        for record in self.records.values() {
            if record.admitted_at_turn.is_none() {
                // The owner manifest may replace provisional admission for a
                // supported but final credit-excluded Memory. Its sole valid
                // unadmitted projection is the canonical creation record.
                if *record != FreshnessRecord::created() {
                    return Err("invalid unadmitted import replacement record".into());
                }
                continue;
            }
            let mut settled = *record;
            settled
                .settle_with_policy(self.cut, &FreshnessPolicy::default_v1())
                .map_err(|e| e.to_string())?;
            if settled != *record
                || record
                    .admitted_at_turn
                    .is_some_and(|at| at == 0 || at > self.cut)
            {
                return Err("import records are not canonical at their verified cut".into());
            }
        }
        Ok(())
    }
}
pub(crate) fn import_id(owner: [u8; 16], corpus: [u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"reliquary:historical-import:v1\0");
    h.update([7]);
    h.update(2u32.to_le_bytes());
    for (key, value) in [
        ("corpus_digest", corpus.as_slice()),
        ("owner_uuid", owner.as_slice()),
    ] {
        h.update([5]);
        h.update((key.len() as u32).to_le_bytes());
        h.update(key.as_bytes());
        h.update([4]);
        h.update((value.len() as u32).to_le_bytes());
        h.update(value);
    }
    h.finalize().into()
}
impl FreshnessStore {
    pub(crate) fn import_checkpoint(&self) -> Option<&ImportCheckpoint> {
        self.import_checkpoint.as_ref()
    }
    pub(crate) fn provisional_import_events(
        &self,
        cohort: &BTreeSet<MemoryId>,
        cut: u64,
    ) -> Result<BTreeSet<String>, String> {
        let mut ids = BTreeSet::new();
        self.scan_entries(|entry| {
            if super::use_fields(&entry).is_some() {
                return Err("import cannot overwrite legitimate accepted use".into());
            }
            for_each_event(&entry, &mut |event| {
                if let JournalEntry::Event {
                    event_id,
                    turn,
                    provenance,
                    effects,
                    ..
                } = event
                {
                    if *turn > cut || provenance.producer_kind != 1 {
                        return Err(
                            "import scoring is not an isolated provisional Dream history".into(),
                        );
                    }
                    if effects.keys().any(|id| cohort.contains(id)) {
                        if effects.keys().any(|id| !cohort.contains(id)) {
                            return Err(
                                "provisional event crosses the replacement/untouched boundary"
                                    .into(),
                            );
                        }
                        ids.insert(event_id.clone());
                    }
                }
                Ok(())
            })
        })?;
        Ok(ids)
    }
    pub(crate) fn ingest_import_checkpoint(&mut self, payload: &[u8]) -> Result<(), String> {
        let cp = ImportCheckpoint::decode(payload)?;
        if self.owner_uuid != Some(cp.owner)
            || self.policy != FreshnessPolicy::default_v1()
            || !self.policy_persisted
        {
            return Err("import checkpoint owner/base policy mismatch".into());
        }
        if let Some(old) = &self.import_checkpoint {
            return if old == &cp {
                Ok(())
            } else {
                Err("ImportManifestConflict: owner already reconstructed".into())
            };
        }
        if self.semantic_completed {
            return Err("import cannot overwrite completed live semantic history".into());
        }
        if cp.cohort.iter().any(|id| {
            !self
                .births
                .get(id)
                .is_some_and(|(birth, _)| *birth <= cp.cut)
        }) {
            return Err("import cohort lacks accepted birth proof".into());
        }
        if self.provisional_import_events(&cp.records.keys().copied().collect(), cp.cut)?
            != cp.superseded
        {
            return Err("import provisional scoring identity mismatch".into());
        }
        // All validation precedes the one complete projection switch.
        self.records.extend(cp.records.clone());
        self.import_checkpoint = Some(cp);
        Ok(())
    }
}
