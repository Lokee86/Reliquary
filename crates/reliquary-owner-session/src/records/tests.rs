use super::*;
use crate::MemoryId;
use crate::records::dream::{dream_begin_codec::BeginPopulation, population_digest};
use reliquary_container::{Container, ContainerIdentity, ReliquaryScope};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".owner-evidence-test-tmp");
        fs::create_dir_all(&root).unwrap();
        Self(root.join(format!(
            "owner-evidence-{}-{}.rel",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
        if let Some(parent) = self.0.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
}
fn container(path: &PathBuf, owner: u8) -> Container {
    let mut id = [0; 16];
    id[0] = owner;
    Container::create(
        path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        id,
    )
    .unwrap()
}

fn retention(owner: [u8; 16]) -> HistoryPruneManifest {
    HistoryPruneManifest {
        owner_uuid: owner,
        source_global_version: 4,
        source_history_digest: [1; 32],
        descriptors: vec![HistoryRetentionDescriptor {
            target: RetentionTarget {
                domain: RetentionDomain::Memory,
                identity: vec![3],
            },
            tombstone_global_version: Some(4),
            tombstone_domain_version: Some(2),
            tombstone_digest: [2; 32],
            retained_structure_digest: [3; 32],
            payloads: vec![PrunedPayload::MemoryBody([4; 32])],
        }],
    }
}

fn import_start(owner: [u8; 16]) -> ImportStart {
    let key = "ab".repeat(32);
    ImportStart {
        format_version: 2,
        owner_uuid: owner,
        import_id: import_id(owner, &key),
        import_key: key,
        source_namespace: "source/account".into(),
        source_format: "json-v1".into(),
        started_at_ns: 10,
        pre: ImportWatermarks::default(),
    }
}
fn import_complete(
    start: &ImportStart,
    c: &mut Container,
    s: &mut ImportEpochStore,
) -> ImportComplete {
    let settlement = ImportFreshnessSettlement {
        owner_uuid: start.owner_uuid,
        import_id: start.import_id.clone(),
        start_digest: format!("{:x}", Sha256::digest(serde_json::to_vec(start).unwrap())),
        receipt_digest: hex_digest(b"receipt-v1"),
        prefix_digest: import_settlement::source_prefix_digest(c).unwrap(),
        rel_turn: 0,
        global_version: c.latest_version(),
        memory_version: start.pre.memory_version,
        graph_version: start.pre.graph_version,
        freshness_state_digest: hex_digest(b"empty-freshness"),
        freshness_memory_count: 0,
        replay_source_authority: None,
    };
    s.append_settlement(c, settlement.clone()).unwrap();
    ImportComplete {
        format_version: 2,
        owner_uuid: start.owner_uuid,
        import_id: start.import_id.clone(),
        start_digest: settlement.start_digest.clone(),
        receipt_digest: settlement.receipt_digest.clone(),
        completed_at_ns: 11,
        post: ImportWatermarks {
            global_version: c.latest_version(),
            ..start.pre
        },
        historical_freshness_settlement_digest: settlement.digest().unwrap(),
    }
}

#[test]
fn retention_manifest_is_owner_bound_replayable_evidence() {
    let path = Temp::new();
    let owner;
    {
        let mut c = container(&path.0, 1);
        owner = c.owner_id();
        let mut s = OwnerMaintenanceStore::open(&c).unwrap();
        let d = s.append(&mut c, &retention(owner)).unwrap();
        assert!(s.get(&d).unwrap().authorizes(
            &retention(owner).descriptors[0].target,
            &PrunedPayload::MemoryBody([4; 32])
        ));
    }
    let c = Container::open_read_only(&path.0).unwrap();
    let mut s = OwnerMaintenanceStore::open(&c).unwrap();
    assert_eq!(s.manifests().len(), 1);
    let other = Temp::new();
    let mut c = container(&other.0, 2);
    let bad = retention(owner);
    assert!(s.append(&mut c, &bad).is_err());
}

#[test]
fn import_processing_plan_replays_only_monotone_owner_bound_transitions() {
    let path = Temp::new();
    let start;
    let final_binding;
    {
        let mut container = container(&path.0, 5);
        start = import_start(container.owner_id());
        let mut store = ImportEpochStore::open(&container).unwrap();
        store.append_start(&mut container, start.clone()).unwrap();
        let initial = ImportProcessingBinding {
            policy: [8; 32],
            destination: None,
            profile: None,
        };
        store
            .append_processing_binding(&mut container, &start.import_id, initial)
            .unwrap();
        let cut = container.latest_version();
        for incompatible in [
            initial,
            ImportProcessingBinding {
                policy: [9; 32],
                ..initial
            },
        ] {
            assert!(
                store
                    .append_processing_binding(&mut container, &start.import_id, incompatible)
                    .is_err()
            );
        }
        assert_eq!(container.latest_version(), cut);
        final_binding = ImportProcessingBinding {
            destination: Some([6; 16]),
            profile: Some([7; 32]),
            ..initial
        };
        store
            .append_processing_binding(&mut container, &start.import_id, final_binding)
            .unwrap();
        let complete = import_complete(&start, &mut container, &mut store);
        store.append_complete(&mut container, complete).unwrap();
        assert!(
            store
                .append_processing_binding(&mut container, &start.import_id, initial)
                .is_err()
        );
    }
    let container = Container::open_read_only(&path.0).unwrap();
    let store = ImportEpochStore::open(&container).unwrap();
    assert_eq!(
        store.processing_binding(&start.import_id),
        Some(final_binding)
    );
}

#[test]
fn import_epochs_replay_order_digest_and_owner_admission() {
    let path = Temp::new();
    let start;
    {
        let mut c = container(&path.0, 3);
        start = import_start(c.owner_id());
        let mut s = ImportEpochStore::open(&c).unwrap();
        s.append_start(&mut c, start.clone()).unwrap();
        let complete = import_complete(&start, &mut c, &mut s);
        assert!(s.append_complete(&mut c, complete.clone()).unwrap());
        assert!(!s.append_complete(&mut c, complete).unwrap());
    }
    let c = Container::open_read_only(&path.0).unwrap();
    let s = ImportEpochStore::open(&c).unwrap();
    assert!(matches!(
        s.status(&start.import_id),
        Some(ImportEpochStatus::Completed { .. })
    ));
    assert!(s.validate_owner(c.owner_id()).is_ok());
    let wrong = Temp::new();
    let mut wrong_c = container(&wrong.0, 4);
    wrong_c
        .append(&c.read_frames().unwrap()[0].payload)
        .unwrap();
    wrong_c.sync().unwrap();
    assert!(ImportEpochStore::open(&wrong_c).is_err());
}

#[test]
fn dream_first_cycle_requires_exact_population_admission_and_reopens() {
    let path = Temp::new();
    let source = MemoryId([1; 32]);
    let preexisting = BTreeSet::from([MemoryId([2; 32]), MemoryId([3; 32])]);
    let admission;
    {
        let mut c = container(&path.0, 5);
        let owner = c.owner_id();
        admission = DreamBeginAdmission {
            owner_uuid: owner,
            pass_id: "first-pass".into(),
            source_id: source,
            turn: 7,
            graph_version_before: 9,
            preexisting_ids: preexisting.clone(),
            memory_cut: 42,
        };
        let mut s = DreamEvidenceStore::open(&c, [admission.clone()]).unwrap();
        let begin = dream::dream_begin_codec::BeginRecord {
            owner_uuid: owner,
            pass_id: admission.pass_id.clone(),
            source_id: source,
            turn: 7,
            graph_version_before: 9,
            population: BeginPopulation::CompactCut {
                memory_cut: 42,
                digest: population_digest(&preexisting),
            },
        };
        let mut unproven = begin.clone();
        if let BeginPopulation::CompactCut { digest, .. } = &mut unproven.population {
            *digest = [0; 32];
        }
        assert!(s.append_begin(&mut c, unproven).is_err());
        assert!(c.read_frames().unwrap().is_empty());
        s.append_begin(&mut c, begin.clone()).unwrap();
        let accept = s.journal().accept_payload(&admission.pass_id, 7).unwrap();
        s.append_journal(&mut c, &accept).unwrap();
        let settled = s.journal().settled_payload(&admission.pass_id).unwrap();
        s.append_journal(&mut c, &settled).unwrap();
    }
    let c = Container::open_read_only(&path.0).unwrap();
    let mut s = DreamEvidenceStore::open(&c, [admission.clone()]).unwrap();
    assert!(s.journal().is_settled(&admission.pass_id));
    assert_eq!(s.begin(&admission.pass_id).unwrap().source_id, source);
    let other = Temp::new();
    let mut other_c = container(&other.0, 6);
    let wrong_owner = dream::dream_begin_codec::BeginRecord {
        owner_uuid: admission.owner_uuid,
        pass_id: admission.pass_id.clone(),
        source_id: source,
        turn: 7,
        graph_version_before: 9,
        population: BeginPopulation::CompactCut {
            memory_cut: 42,
            digest: population_digest(&preexisting),
        },
    };
    assert!(s.append_begin(&mut other_c, wrong_owner).is_err());
}

#[test]
fn dream_replay_rejects_foreign_and_duplicate_admissions_before_reading_records() {
    let path = Temp::new();
    let c = container(&path.0, 8);
    let owner = c.owner_id();
    let admission = DreamBeginAdmission {
        owner_uuid: owner,
        pass_id: "admitted-pass".into(),
        source_id: MemoryId([1; 32]),
        turn: 7,
        graph_version_before: 9,
        preexisting_ids: BTreeSet::new(),
        memory_cut: 0,
    };

    let mut foreign = admission.clone();
    foreign.owner_uuid = [99; 16];
    assert!(DreamEvidenceStore::open(&c, [foreign]).is_err());

    let mut duplicate = admission.clone();
    duplicate.source_id = MemoryId([2; 32]);
    assert!(DreamEvidenceStore::open(&c, [admission, duplicate]).is_err());
    assert!(c.read_frames().unwrap().is_empty());
}
