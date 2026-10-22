use super::*;
use crate::{MemoryDraft, Phylactery};
use std::{fs, path::PathBuf};

const DAY_NS: i64 = 24 * 60 * 60 * 1_000_000_000;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("dream-processing-oracle-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn draft(mutation: &str) -> MemoryDraft {
    MemoryDraft {
        category: "preference".into(),
        memory_type: "user".into(),
        authority_kind: "direct".into(),
        temporal_status: "unknown".into(),
        title: "Dream processing fixture".into(),
        content: mutation.into(),
        scope: "user".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: Some(0),
        mutation_id: mutation.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

#[test]
fn current_dream_processing_projection_matches_owner_records_and_reopens() {
    let fixture = Fixture::new();
    let rel_path = fixture.path("owner.rel");
    let phy_path = fixture.path("owner.phy");
    let mut rel = Cva::create(&rel_path).unwrap();
    let rel_id = rel
        .publish_memory(None, 0, draft("rel-memory"))
        .unwrap()
        .0
        .id;
    let rel_empty = reliquary_dream_processing(&rel).unwrap();
    assert!(rel_empty.records.is_empty());
    assert_eq!(rel_empty.fingerprint.domain, "dream_processing");
    assert_eq!(rel_empty.fingerprint.local_cut, 0);
    assert_eq!(rel_empty.fingerprint.event_cut, None);
    assert_eq!(rel.mark_dream_processed(rel_id, 2, 123).unwrap(), true);
    let rel_cut = reliquary_dream_processing(&rel).unwrap();
    assert_eq!(rel_cut.records, rel.processing_epochs.records());
    assert_eq!(rel_cut.records.len(), 1);
    assert_eq!(rel_cut.records[0].0, rel_id);
    assert_eq!(rel_cut.records[0].2.satisfied_through_epoch, 2);
    assert_eq!(rel_cut.records[0].2.checkpoint_at_ns, Some(123));
    assert_eq!(rel.dream_was_processed(rel_id), true);
    assert_eq!(rel.dream_eligible_epoch(rel_id, 60 * DAY_NS).unwrap(), None);
    assert_eq!(
        rel.dream_eligible_epoch(rel_id, 90 * DAY_NS).unwrap(),
        Some(3)
    );
    assert_eq!(rel.mark_dream_processed(rel_id, 2, 123).unwrap(), false);
    assert_eq!(
        reliquary_dream_processing(&rel).unwrap().fingerprint,
        rel_cut.fingerprint
    );
    rel.sync().unwrap();
    drop(rel);
    let reopened_rel = Cva::open(&rel_path).unwrap();
    assert_eq!(
        reliquary_dream_processing(&reopened_rel)
            .unwrap()
            .fingerprint,
        rel_cut.fingerprint
    );

    let mut phy = Phylactery::create(&phy_path).unwrap();
    let phy_id = phy
        .publish_memory(None, 0, draft("phy-memory"))
        .unwrap()
        .0
        .id;
    let phy_empty = phylactery_dream_processing(&phy).unwrap();
    assert!(phy_empty.records.is_empty());
    assert_eq!(phy.mark_dream_processed(phy_id, 3, -456).unwrap(), true);
    let phy_cut = phylactery_dream_processing(&phy).unwrap();
    assert_eq!(phy_cut.records, phy.processing_epochs.records());
    assert_eq!(phy_cut.records.len(), 1);
    assert_eq!(phy_cut.records[0].2.satisfied_through_epoch, 3);
    assert_eq!(phy_cut.records[0].2.checkpoint_at_ns, Some(-456));
    assert!(phy.dream_was_processed(phy_id));
    assert_eq!(phy.dream_eligible_epoch(phy_id, 90 * DAY_NS).unwrap(), None);
    assert_eq!(
        phy.dream_eligible_epoch(phy_id, 120 * DAY_NS).unwrap(),
        Some(4)
    );
    phy.sync().unwrap();
    drop(phy);
    let reopened_phy = Phylactery::open(&phy_path).unwrap();
    assert_eq!(
        phylactery_dream_processing(&reopened_phy)
            .unwrap()
            .fingerprint,
        phy_cut.fingerprint
    );
}

#[test]
#[cfg(feature = "history-oracle-fixtures")]
fn authoritative_owner_processing_epochs_match_current_records_and_reopen() {
    let root = PathBuf::from(std::env::var_os("RELIQUARY_HISTORY_FIXTURES").expect("fixture root"));
    let fixture = Fixture::new();
    for (index, corpus) in [
        "chatgpt-first14d",
        "chatgpt-first28d",
        "rhelm-david-r-ellis-pre-relationship",
    ]
    .iter()
    .enumerate()
    {
        for (kind, name) in [("rel", "project.prj.rel"), ("phy", "user.phy")] {
            let path = fixture.path(&format!("{index}.{kind}"));
            let mut source = fs::File::open(root.join(corpus).join(name)).unwrap();
            let mut destination = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            std::io::copy(&mut source, &mut destination).unwrap();
            drop(destination);

            if kind == "rel" {
                let rel = Cva::open(&path).unwrap();
                let first = reliquary_dream_processing(&rel).unwrap();
                let second = reliquary_dream_processing(&rel).unwrap();
                assert_eq!(first.records, rel.processing_epochs.records());
                assert_eq!(first.fingerprint, second.fingerprint);
                let dream_lanes = first
                    .records
                    .iter()
                    .filter(|(_, lane, _)| *lane == crate::dream_cooldown::DREAM_PROCESSING_LANE)
                    .count();
                eprintln!(
                    "{corpus}/rel Dream lane={dream_lanes} all lanes={}",
                    first.records.len()
                );
                drop(rel);
                let reopened = Cva::open(&path).unwrap();
                assert_eq!(
                    reliquary_dream_processing(&reopened).unwrap().fingerprint,
                    first.fingerprint,
                    "{corpus}/rel reopen parity"
                );
            } else {
                let phy = Phylactery::open(&path).unwrap();
                let first = phylactery_dream_processing(&phy).unwrap();
                let second = phylactery_dream_processing(&phy).unwrap();
                assert_eq!(first.records, phy.processing_epochs.records());
                assert_eq!(first.fingerprint, second.fingerprint);
                let dream_lanes = first
                    .records
                    .iter()
                    .filter(|(_, lane, _)| *lane == crate::dream_cooldown::DREAM_PROCESSING_LANE)
                    .count();
                eprintln!(
                    "{corpus}/phy Dream lane={dream_lanes} all lanes={}",
                    first.records.len()
                );
                drop(phy);
                let reopened = Phylactery::open(&path).unwrap();
                assert_eq!(
                    phylactery_dream_processing(&reopened).unwrap().fingerprint,
                    first.fingerprint,
                    "{corpus}/phy reopen parity"
                );
            }
        }
    }
}
