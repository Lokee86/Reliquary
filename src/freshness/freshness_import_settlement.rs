//! Ordered sparse Historical Freshness settlements and isolated legacy decoding.
use super::{FreshnessStore, JournalEntry, Reader, for_each_event, put_bytes, put_record};
use crate::{
    MemoryId,
    freshness::{FreshnessPolicy, FreshnessRecord},
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
pub(crate) const SETTLEMENT_MAGIC: &[u8; 8] = b"CVAFRS04";
pub(crate) const LEGACY_CHECKPOINT_MAGIC: &[u8; 8] = b"CVAFRS03";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HistoricalImportFreshnessScope {
    pub owner: [u8; 16],
    pub import_id: [u8; 32],
    pub start_evidence_digest: [u8; 32],
    pub base_cut: u64,
    pub settlement_cut: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HistoricalImportFreshnessSettlement {
    pub owner: [u8; 16],
    pub import_id: [u8; 32],
    pub start_evidence_digest: [u8; 32],
    pub corpus_digest: [u8; 32],
    pub manifest_digest: [u8; 32],
    pub input_digest: [u8; 32],
    pub cohort: BTreeSet<MemoryId>,
    pub base_cut: u64,
    pub settlement_cut: u64,
    pub global_version: u64,
    pub input_bytes: u64,
    pub archive_version: u64,
    pub graph_version: u64,
    pub memory_graph_version: u64,
    pub records: BTreeMap<MemoryId, FreshnessRecord>,
    pub superseded: BTreeSet<String>,
}

#[cfg(test)]
pub(crate) fn encode_legacy_test_fixture(
    settlement: &HistoricalImportFreshnessSettlement,
) -> Vec<u8> {
    let mut out = LEGACY_CHECKPOINT_MAGIC.to_vec();
    out.extend(settlement.owner);
    for digest in [
        settlement.import_id,
        settlement.corpus_digest,
        settlement.manifest_digest,
        settlement.input_digest,
        crate::freshness::historical_planner::HISTORICAL_POLICY_DIGEST,
        crate::freshness::semantic_policy::SemanticPolicy::default()
            .digest()
            .unwrap(),
    ] {
        out.extend(digest);
    }
    for value in [
        settlement.settlement_cut,
        settlement.global_version,
        settlement.input_bytes,
        settlement.archive_version,
        settlement.graph_version,
        settlement.memory_graph_version,
    ] {
        out.extend(value.to_le_bytes());
    }
    out.extend((settlement.cohort.len() as u32).to_le_bytes());
    for id in &settlement.cohort {
        out.extend(id.0);
    }
    out.extend((settlement.records.len() as u32).to_le_bytes());
    for (id, record) in &settlement.records {
        out.extend(id.0);
        put_record(&mut out, *record);
    }
    out.extend((settlement.superseded.len() as u32).to_le_bytes());
    for id in &settlement.superseded {
        put_bytes(&mut out, id.as_bytes());
    }
    out.extend(Sha256::digest(&out));
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (FreshnessStore, HistoricalImportFreshnessSettlement, Vec<u8>) {
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
        let cp = HistoricalImportFreshnessSettlement {
            owner,
            import_id: [6; 32],
            start_evidence_digest: [7; 32],
            corpus_digest,
            manifest_digest: [4; 32],
            input_digest: [5; 32],
            cohort: BTreeSet::from([id]),
            base_cut: 0,
            settlement_cut: 2001,
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
        assert_eq!(
            HistoricalImportFreshnessSettlement::decode(&bytes).unwrap(),
            cp
        );
        store.ingest_import_settlement(&bytes).unwrap();
        assert_eq!(store.records()[&id], FreshnessRecord::created());
        assert_eq!(
            store.records_at(cp.settlement_cut).unwrap()[&id],
            FreshnessRecord::created()
        );
        let mut corrupt = cp;
        corrupt.records.get_mut(&id).unwrap().score = 99;
        assert!(HistoricalImportFreshnessSettlement::decode(&corrupt.encode()).is_err());
    }
    #[test]
    fn legacy_checkpoint_decodes_to_one_synthetic_initial_settlement_only() {
        let (mut store, expected, _) = fixture();
        let legacy = encode_legacy_test_fixture(&expected);
        store.ingest_import_settlement(&legacy).unwrap();
        let migrated = store.latest_import_settlement().unwrap();
        assert_eq!(migrated.owner, expected.owner);
        assert_eq!(migrated.base_cut, 0);
        assert_eq!(migrated.settlement_cut, expected.settlement_cut);
        assert_eq!(migrated.import_id, expected.import_id);
        assert_ne!(migrated.start_evidence_digest, [0; 32]);
        assert_eq!(migrated.corpus_digest, expected.corpus_digest);
        assert_eq!(migrated.manifest_digest, expected.manifest_digest);
        assert_eq!(migrated.input_digest, expected.input_digest);
        assert_eq!(migrated.cohort, expected.cohort);
        assert_eq!(migrated.global_version, expected.global_version);
        assert_eq!(migrated.input_bytes, expected.input_bytes);
        assert_eq!(migrated.archive_version, expected.archive_version);
        assert_eq!(migrated.graph_version, expected.graph_version);
        assert_eq!(migrated.memory_graph_version, expected.memory_graph_version);
        assert_eq!(migrated.records, expected.records);
        assert_eq!(migrated.superseded, expected.superseded);
        assert!(migrated.encode().starts_with(SETTLEMENT_MAGIC));
        assert!(!migrated.encode().starts_with(LEGACY_CHECKPOINT_MAGIC));
    }
    #[test]
    fn ordered_sparse_settlements_preserve_untouched_and_pre_start_live_history() {
        let (mut store, mut first, _) = fixture();
        let second_id = MemoryId([8; 32]);
        let mut second_birth = FreshnessRecord::created();
        second_birth.admit(2).unwrap();
        store
            .ingest(&store.encode_initialize(first.owner, second_id, 2, second_birth))
            .unwrap();
        let accepted = store
            .prepare_accepted_use_batch(
                first.owner,
                "accepted-before-import".into(),
                500,
                vec![second_id],
                crate::FreshnessEventProof {
                    graph_version: 0,
                    community_generation: None,
                    community_graph_version: None,
                },
                vec![BTreeMap::from([(second_id, 25)])],
            )
            .unwrap();
        store.ingest(&accepted).unwrap();
        let before_first = store.records_at(500).unwrap()[&second_id];
        first.cohort.insert(second_id);
        store.ingest_import_settlement(&first.encode()).unwrap();
        assert_eq!(store.records_at(500).unwrap()[&second_id], before_first);
        let mut expected_at_first = before_first;
        expected_at_first
            .settle_with_policy(first.settlement_cut, &FreshnessPolicy::default_v1())
            .unwrap();
        assert_eq!(
            store.records_at(first.settlement_cut).unwrap()[&second_id],
            expected_at_first
        );

        let mut second_record = before_first;
        second_record
            .settle_with_policy(3001, &FreshnessPolicy::default_v1())
            .unwrap();
        let second = HistoricalImportFreshnessSettlement {
            owner: first.owner,
            import_id: [9; 32],
            start_evidence_digest: [10; 32],
            corpus_digest: [11; 32],
            manifest_digest: [12; 32],
            input_digest: [13; 32],
            cohort: BTreeSet::from([second_id]),
            base_cut: first.settlement_cut,
            settlement_cut: 3001,
            global_version: 2,
            input_bytes: 2,
            archive_version: 2,
            graph_version: 0,
            memory_graph_version: 0,
            records: BTreeMap::from([(second_id, second_record)]),
            superseded: BTreeSet::new(),
        };
        store.ingest_import_settlement(&second.encode()).unwrap();
        store.ingest_import_settlement(&second.encode()).unwrap();
        assert_eq!(store.import_settlements(), &[first.clone(), second.clone()]);
        assert_eq!(store.records_at(3001).unwrap()[&second_id], second_record);
        let mut expected_first_record = first.records[&MemoryId([2; 32])];
        expected_first_record
            .settle_with_policy(3001, &FreshnessPolicy::default_v1())
            .unwrap();
        assert_eq!(
            store.records_at(3001).unwrap()[&MemoryId([2; 32])],
            expected_first_record
        );

        let mut conflict = second;
        conflict.manifest_digest = [14; 32];
        assert!(store.ingest_import_settlement(&conflict.encode()).is_err());
    }
    #[test]
    fn ordered_sparse_settlements_rebuild_from_persisted_container_frames() {
        use crate::Container;

        let owner = [21; 16];
        let first_id = MemoryId([22; 32]);
        let second_id = MemoryId([23; 32]);
        let mut store = FreshnessStore::new(Some(owner));
        let mut frames = Vec::new();

        let policy = super::super::encode_policy(owner, FreshnessPolicy::default_v1());
        store.ingest(&policy).unwrap();
        frames.push(policy);

        let mut first_birth = FreshnessRecord::created();
        first_birth.admit(1).unwrap();
        let first_birth_frame = store.encode_initialize(owner, first_id, 1, first_birth);
        store.ingest(&first_birth_frame).unwrap();
        frames.push(first_birth_frame);

        let proof = crate::FreshnessEventProof {
            graph_version: 0,
            community_generation: None,
            community_graph_version: None,
        };
        let mut first_live = first_birth;
        first_live.reinforce(50, 1001).unwrap();
        let dream_event = store.encode_event(
            owner,
            crate::dream_freshness::initial_pass_id(owner, first_id),
            1001,
            proof,
            BTreeMap::from([(first_id, 50)]),
            BTreeMap::from([(first_id, first_live)]),
        );
        store.ingest(&dream_event).unwrap();
        frames.push(dream_event.clone());

        let mut second_birth = FreshnessRecord::created();
        second_birth.admit(2).unwrap();
        let second_birth_frame = store.encode_initialize(owner, second_id, 2, second_birth);
        store.ingest(&second_birth_frame).unwrap();
        frames.push(second_birth_frame);

        let accepted_use = store
            .prepare_accepted_use_batch(
                owner,
                "persisted-pre-start-use".into(),
                500,
                vec![second_id],
                proof,
                vec![BTreeMap::from([(second_id, 25)])],
            )
            .unwrap();
        store.ingest(&accepted_use).unwrap();
        frames.push(accepted_use);

        let before_use_settlement = store.records_at(500).unwrap();
        let mut first_replacement = first_birth;
        first_replacement
            .settle_with_policy(2001, &FreshnessPolicy::default_v1())
            .unwrap();
        let first = HistoricalImportFreshnessSettlement {
            owner,
            import_id: [24; 32],
            start_evidence_digest: [25; 32],
            corpus_digest: [26; 32],
            manifest_digest: [27; 32],
            input_digest: [28; 32],
            cohort: BTreeSet::from([first_id, second_id]),
            base_cut: 0,
            settlement_cut: 2001,
            global_version: 1,
            input_bytes: 1,
            archive_version: 1,
            graph_version: 0,
            memory_graph_version: 0,
            records: BTreeMap::from([(first_id, first_replacement)]),
            superseded: BTreeSet::from([crate::dream_freshness::initial_pass_id(owner, first_id)]),
        };
        let first_frame = first.encode();
        store.ingest_import_settlement(&first_frame).unwrap();
        frames.push(first_frame);

        let mut second_replacement = before_use_settlement[&second_id];
        second_replacement
            .settle_with_policy(3001, &FreshnessPolicy::default_v1())
            .unwrap();
        let second = HistoricalImportFreshnessSettlement {
            owner,
            import_id: [29; 32],
            start_evidence_digest: [30; 32],
            corpus_digest: [31; 32],
            manifest_digest: [32; 32],
            input_digest: [33; 32],
            cohort: BTreeSet::from([second_id]),
            base_cut: first.settlement_cut,
            settlement_cut: 3001,
            global_version: 2,
            input_bytes: 2,
            archive_version: 2,
            graph_version: 0,
            memory_graph_version: 0,
            records: BTreeMap::from([(second_id, second_replacement)]),
            superseded: BTreeSet::new(),
        };
        let second_frame = second.encode();
        store.ingest_import_settlement(&second_frame).unwrap();
        frames.push(second_frame.clone());

        let expected_at_500 = store.records_at(500).unwrap();
        let expected_at_2001 = store.records_at(2001).unwrap();
        let expected_at_3001 = store.records_at(3001).unwrap();
        assert_eq!(
            expected_at_500[&second_id],
            before_use_settlement[&second_id]
        );
        let mut expected_second_at_2001 = before_use_settlement[&second_id];
        expected_second_at_2001
            .settle_with_policy(2001, &FreshnessPolicy::default_v1())
            .unwrap();
        assert_eq!(expected_at_2001[&second_id], expected_second_at_2001);
        assert_eq!(expected_at_3001[&second_id], second_replacement);
        let mut expected_first_at_3001 = first_replacement;
        expected_first_at_3001
            .settle_with_policy(3001, &FreshnessPolicy::default_v1())
            .unwrap();
        assert_eq!(expected_at_3001[&first_id], expected_first_at_3001);

        let file = std::env::temp_dir().join(format!(
            "reliquary-settlement-reopen-{}.rel",
            uuid::Uuid::new_v4()
        ));
        let mut container = Container::create(&file).unwrap();
        for frame in &frames {
            container.append(frame).unwrap();
        }
        container.sync().unwrap();
        drop(container);

        let payloads = {
            let mut container = Container::open_read_only_for_scan(&file).unwrap();
            container
                .chunks()
                .unwrap()
                .into_iter()
                .map(|chunk| container.read(chunk).unwrap())
                .collect::<Vec<_>>()
        };
        assert_eq!(payloads, frames);

        let rebuild = || {
            let mut reopened = FreshnessStore::new(Some(owner));
            for payload in &payloads {
                reopened.ingest(payload).unwrap();
            }
            reopened
        };
        let mut reopened = rebuild();
        assert_eq!(
            reopened.import_settlements(),
            &[first.clone(), second.clone()]
        );
        assert_eq!(reopened.records_at(500).unwrap(), expected_at_500);
        assert_eq!(reopened.records_at(2001).unwrap(), expected_at_2001);
        assert_eq!(reopened.records_at(3001).unwrap(), expected_at_3001);

        reopened.ingest_import_settlement(&second_frame).unwrap();
        assert_eq!(
            reopened.import_settlements(),
            &[first.clone(), second.clone()]
        );
        assert_eq!(reopened.records_at(3001).unwrap(), expected_at_3001);

        let reopened_again = rebuild();
        assert_eq!(
            reopened_again.import_settlements(),
            &[first.clone(), second.clone()]
        );
        assert_eq!(reopened_again.records_at(500).unwrap(), expected_at_500);
        assert_eq!(reopened_again.records_at(2001).unwrap(), expected_at_2001);
        assert_eq!(reopened_again.records_at(3001).unwrap(), expected_at_3001);
        let _ = std::fs::remove_file(file);
    }

    #[test]
    fn checkpoint_corruption_and_conflict_never_partially_replace_records() {
        let (mut store, cp, _) = fixture();
        let bytes = cp.encode();
        assert_eq!(
            HistoricalImportFreshnessSettlement::decode(&bytes).unwrap(),
            cp
        );
        let original = store.records().clone();
        for end in 0..bytes.len() {
            assert!(store.ingest_import_settlement(&bytes[..end]).is_err());
            assert_eq!(store.records(), &original);
        }
        for at in 0..bytes.len() {
            let mut corrupt = bytes.clone();
            corrupt[at] ^= 1;
            assert!(store.ingest_import_settlement(&corrupt).is_err());
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
        store.ingest_import_settlement(&cp.encode()).unwrap();
        assert_eq!(store.import_settlement(cp.import_id), Some(&cp));

        let (mut completed, cp, _) = fixture();
        completed.semantic_completed = true;
        completed.ingest_import_settlement(&cp.encode()).unwrap();
    }

    #[test]
    fn checkpoint_supersedes_scoring_but_retains_receipt_identity_and_negative_scores() {
        let (mut store, mut cp, old_event) = fixture();
        // Entity/Archive topology may advance the full Graph without advancing
        // the Memory Graph used by graph-qualified Freshness reads.
        cp.graph_version = 10;
        store.ingest(&cp.encode()).unwrap();
        assert_eq!(cp.records.values().next().unwrap().score, -100);
        assert_eq!(store.records_at(cp.settlement_cut).unwrap(), cp.records);
        assert_eq!(
            store
                .records_at_cut(&crate::FreshnessEventCut {
                    rel_turn: cp.settlement_cut,
                    event_id_inclusive: None,
                    graph_version: 0
                })
                .unwrap(),
            cp.records
        );
        store.ingest(&old_event).unwrap();
        assert_eq!(store.records_at(cp.settlement_cut).unwrap(), cp.records);
        assert_ne!(store.records_at(cp.settlement_cut - 1).unwrap(), cp.records);
        assert!(store.replay_entries().is_err());
        // An exact checkpoint retry cannot overwrite a later resident projection.
        let id = *cp.records.keys().next().unwrap();
        let mut live = cp.records[&id];
        live.reinforce(25, cp.settlement_cut + 1).unwrap();
        let event = store
            .prepare_accepted_use_batch(
                cp.owner,
                "post-checkpoint-use".into(),
                cp.settlement_cut + 1,
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
        assert_eq!(store.records_at(cp.settlement_cut + 1).unwrap()[&id], live);
    }
}
impl HistoricalImportFreshnessSettlement {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_import_scope(
        scope: HistoricalImportFreshnessScope,
        corpus_digest: [u8; 32],
        manifest_digest: [u8; 32],
        input_digest: [u8; 32],
        cohort: BTreeSet<MemoryId>,
        global_version: u64,
        input_bytes: u64,
        archive_version: u64,
        graph_version: u64,
        memory_graph_version: u64,
        records: BTreeMap<MemoryId, FreshnessRecord>,
        superseded: BTreeSet<String>,
    ) -> Self {
        Self {
            owner: scope.owner,
            import_id: scope.import_id,
            start_evidence_digest: scope.start_evidence_digest,
            corpus_digest,
            manifest_digest,
            input_digest,
            cohort,
            base_cut: scope.base_cut,
            settlement_cut: scope.settlement_cut,
            global_version,
            input_bytes,
            archive_version,
            graph_version,
            memory_graph_version,
            records,
            superseded,
        }
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut out = SETTLEMENT_MAGIC.to_vec();
        out.extend(self.owner);
        for d in [
            self.import_id,
            self.start_evidence_digest,
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
            self.base_cut,
            self.settlement_cut,
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
        if payload.len() < 32 || payload.get(..8) != Some(SETTLEMENT_MAGIC.as_slice()) {
            return Err("invalid import settlement framing".into());
        }
        let end = payload.len() - 32;
        if Sha256::digest(&payload[..end]).as_slice() != &payload[end..] {
            return Err("import settlement checksum mismatch".into());
        }
        let mut r = Reader {
            bytes: &payload[..end],
            at: 8,
        };
        let owner = r.array()?;
        let import_id = r.array()?;
        let start_evidence_digest = r.array()?;
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
        let base_cut = r.u64()?;
        let settlement_cut = r.u64()?;
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
            return Err("trailing import settlement bytes".into());
        }
        let cp = Self {
            owner,
            import_id,
            start_evidence_digest,
            corpus_digest,
            manifest_digest,
            input_digest,
            cohort,
            base_cut,
            settlement_cut,
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
        if self.settlement_cut == 0
            || self.base_cut >= self.settlement_cut
            || self.records.is_empty()
            || self.cohort.is_empty()
            || self.cohort.len() > u32::MAX as usize
            || self.records.keys().any(|id| !self.cohort.contains(id))
            || self.records.len() > u32::MAX as usize
            || self.superseded.len() > u32::MAX as usize
            || self.import_id == [0; 32]
            || self.start_evidence_digest == [0; 32]
            || self.manifest_digest == [0; 32]
            || self.memory_graph_version > self.graph_version
        {
            return Err("invalid import settlement identity/cohort".into());
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
                .settle_with_policy(self.settlement_cut, &FreshnessPolicy::default_v1())
                .map_err(|e| e.to_string())?;
            if settled != *record
                || record
                    .admitted_at_turn
                    .is_some_and(|at| at == 0 || at > self.settlement_cut)
            {
                return Err("import records are not canonical at their verified cut".into());
            }
        }
        Ok(())
    }
}

fn decode_legacy_checkpoint(payload: &[u8]) -> Result<HistoricalImportFreshnessSettlement, String> {
    if payload.len() < 32 || payload.get(..8) != Some(LEGACY_CHECKPOINT_MAGIC.as_slice()) {
        return Err("invalid legacy import checkpoint framing".into());
    }
    let end = payload.len() - 32;
    if Sha256::digest(&payload[..end]).as_slice() != &payload[end..] {
        return Err("legacy import checkpoint checksum mismatch".into());
    }
    let mut reader = Reader {
        bytes: &payload[..end],
        at: 8,
    };
    let owner = reader.array()?;
    let import_id = reader.array()?;
    let corpus_digest = reader.array()?;
    let manifest_digest = reader.array()?;
    let input_digest = reader.array()?;
    if reader.array::<32>()? != crate::freshness::historical_planner::HISTORICAL_POLICY_DIGEST
        || reader.array::<32>()?
            != crate::freshness::semantic_policy::SemanticPolicy::default()
                .digest()
                .map_err(|error| error.to_string())?
    {
        return Err("unsupported legacy import reconstruction policy".into());
    }
    let settlement_cut = reader.u64()?;
    let global_version = reader.u64()?;
    let input_bytes = reader.u64()?;
    let archive_version = reader.u64()?;
    let graph_version = reader.u64()?;
    let memory_graph_version = reader.u64()?;
    let mut cohort = BTreeSet::new();
    let mut previous = None;
    for _ in 0..reader.u32()? {
        let id = MemoryId(reader.array()?);
        if previous.is_some_and(|old| old >= id) {
            return Err("unordered legacy import cohort".into());
        }
        previous = Some(id);
        cohort.insert(id);
    }
    let mut records = BTreeMap::new();
    let mut previous = None;
    for _ in 0..reader.u32()? {
        let id = MemoryId(reader.array()?);
        if previous.is_some_and(|old| old >= id) {
            return Err("unordered legacy import records".into());
        }
        previous = Some(id);
        records.insert(id, reader.record()?);
    }
    let mut superseded = BTreeSet::new();
    let mut previous: Option<String> = None;
    for _ in 0..reader.u32()? {
        let id = String::from_utf8(reader.bytes()?).map_err(|error| error.to_string())?;
        if id.is_empty() || previous.as_ref().is_some_and(|old| old >= &id) {
            return Err("unordered legacy import scoring identities".into());
        }
        previous = Some(id.clone());
        superseded.insert(id);
    }
    if reader.at != end {
        return Err("trailing legacy import checkpoint bytes".into());
    }
    let start_evidence_digest = Sha256::digest(
        [
            b"reliquary:legacy-historical-import-start:v1\0".as_slice(),
            &payload[..end],
        ]
        .concat(),
    )
    .into();
    let settlement = HistoricalImportFreshnessSettlement {
        owner,
        import_id,
        start_evidence_digest,
        corpus_digest,
        manifest_digest,
        input_digest,
        cohort,
        base_cut: 0,
        settlement_cut,
        global_version,
        input_bytes,
        archive_version,
        graph_version,
        memory_graph_version,
        records,
        superseded,
    };
    settlement.validate()?;
    Ok(settlement)
}
impl FreshnessStore {
    pub(crate) fn import_settlements(&self) -> &[HistoricalImportFreshnessSettlement] {
        &self.import_settlements
    }
    pub(crate) fn import_settlement(
        &self,
        import_id: [u8; 32],
    ) -> Option<&HistoricalImportFreshnessSettlement> {
        self.import_settlements
            .iter()
            .find(|settlement| settlement.import_id == import_id)
    }
    pub(crate) fn latest_import_settlement(&self) -> Option<&HistoricalImportFreshnessSettlement> {
        self.import_settlements.last()
    }
    pub(crate) fn provisional_import_events(
        &self,
        affected: &BTreeSet<MemoryId>,
        base_cut: u64,
        settlement_cut: u64,
    ) -> Result<BTreeSet<String>, String> {
        let mut ids = BTreeSet::new();
        self.scan_entries(|entry| {
            for_each_event(&entry, &mut |event| {
                if let JournalEntry::Event {
                    event_id,
                    turn,
                    provenance,
                    effects,
                    ..
                } = event
                {
                    if *turn > base_cut
                        && *turn <= settlement_cut
                        && provenance.producer_kind == 1
                        && effects.keys().any(|id| affected.contains(id))
                    {
                        if effects.keys().any(|id| !affected.contains(id)) {
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
    pub(crate) fn ingest_import_settlement(&mut self, payload: &[u8]) -> Result<(), String> {
        let settlement = if payload.starts_with(LEGACY_CHECKPOINT_MAGIC) {
            decode_legacy_checkpoint(payload)?
        } else {
            HistoricalImportFreshnessSettlement::decode(payload)?
        };
        if self.owner_uuid != Some(settlement.owner)
            || self.policy != FreshnessPolicy::default_v1()
            || !self.policy_persisted
        {
            return Err("import settlement owner/base policy mismatch".into());
        }
        if let Some(old) = self.import_settlement(settlement.import_id) {
            return if old == &settlement {
                Ok(())
            } else {
                Err("ImportManifestConflict: import already settled".into())
            };
        }
        if self.import_settlements.last().is_some_and(|previous| {
            (previous.settlement_cut, previous.import_id)
                >= (settlement.settlement_cut, settlement.import_id)
                || settlement.base_cut < previous.settlement_cut
        }) {
            return Err("import settlements are not in accepted epoch order".into());
        }
        if settlement.cohort.iter().any(|id| {
            !self
                .births
                .get(id)
                .is_some_and(|(birth, _)| *birth <= settlement.settlement_cut)
        }) {
            return Err("import settlement cohort lacks accepted birth proof".into());
        }
        if self.provisional_import_events(
            &settlement.records.keys().copied().collect(),
            settlement.base_cut,
            settlement.settlement_cut,
        )? != settlement.superseded
        {
            return Err("import provisional scoring identity mismatch".into());
        }
        self.records.extend(settlement.records.clone());
        self.import_settlements.push(settlement);
        Ok(())
    }
}
