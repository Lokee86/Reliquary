use super::*;
use crate::{MemoryDraft, Phylactery};
use std::{fs, path::PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("dream-pair-oracle-{}", uuid::Uuid::new_v4()));
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
        title: "Dream pair fixture".into(),
        content: mutation.into(),
        scope: "user".into(),
        lifecycle_state: "extracted".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: Some(1),
        mutation_id: mutation.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn ids_rel(cva: &mut Cva) -> (MemoryId, MemoryId, MemoryId) {
    let a = cva.publish_memory(None, 0, draft("rel-a")).unwrap().0.id;
    let b = cva.publish_memory(None, 0, draft("rel-b")).unwrap().0.id;
    let c = cva.publish_memory(None, 0, draft("rel-c")).unwrap().0.id;
    (a, b, c)
}

fn ids_phy(phy: &mut Phylactery) -> (MemoryId, MemoryId, MemoryId) {
    let a = phy.publish_memory(None, 0, draft("phy-a")).unwrap().0.id;
    let b = phy.publish_memory(None, 0, draft("phy-b")).unwrap().0.id;
    let c = phy.publish_memory(None, 0, draft("phy-c")).unwrap().0.id;
    (a, b, c)
}

fn canonical(a: MemoryId, b: MemoryId) -> ([u8; 32], [u8; 32]) {
    if a.0 <= b.0 { (a.0, b.0) } else { (b.0, a.0) }
}

#[test]
fn dream_pair_set_fingerprints_are_canonical_and_survive_reopen_for_both_owners() {
    let fixture = Fixture::new();
    let rel_path = fixture.path("owner.rel");
    let phy_path = fixture.path("owner.phy");

    let mut rel = Cva::create(&rel_path).unwrap();
    let (ra, rb, rc) = ids_rel(&mut rel);
    let rel_empty = reliquary_dream_pairs(&rel).unwrap();
    assert!(rel_empty.pairs.is_empty());
    assert_eq!(rel_empty.fingerprint.local_cut, 0);
    assert_eq!(rel.mark_dream_pair_evaluated(ra, rb).unwrap(), true);
    assert_eq!(rel.mark_dream_pair_evaluated(rb, ra).unwrap(), false);
    assert_eq!(rel.mark_dream_pair_evaluated(rc, rb).unwrap(), true);
    let rel_cut = reliquary_dream_pairs(&rel).unwrap();
    assert_eq!(
        rel_cut.pairs,
        BTreeSet::from([canonical(ra, rb), canonical(rb, rc)])
    );
    assert_eq!(rel_cut.fingerprint.rows, 2);
    assert_ne!(rel_empty.fingerprint, rel_cut.fingerprint);

    let mut phy = Phylactery::create(&phy_path).unwrap();
    let (pa, pb, pc) = ids_phy(&mut phy);
    let phy_empty = phylactery_dream_pairs(&phy).unwrap();
    assert!(phy_empty.pairs.is_empty());
    assert_eq!(phy.mark_dream_pair_evaluated(pb, pa).unwrap(), true);
    assert_eq!(phy.mark_dream_pair_evaluated(pa, pb).unwrap(), false);
    assert_eq!(phy.mark_dream_pair_evaluated(pb, pc).unwrap(), true);
    let phy_cut = phylactery_dream_pairs(&phy).unwrap();
    assert_eq!(
        phy_cut.pairs,
        BTreeSet::from([canonical(pa, pb), canonical(pb, pc)])
    );
    assert_eq!(phy_cut.fingerprint.rows, 2);
    assert_ne!(phy_empty.fingerprint, phy_cut.fingerprint);

    // The API-projected rows agree exactly with each owner's decoded canonical set.
    assert_eq!(rel_cut.pairs.len(), rel.dream_pair_records().len());
    assert_eq!(phy_cut.pairs.len(), phy.dream_pair_records().len());
    rel.sync().unwrap();
    phy.sync().unwrap();
    drop(rel);
    drop(phy);

    let reopened_rel = Cva::open(&rel_path).unwrap();
    let reopened_phy = Phylactery::open(&phy_path).unwrap();
    assert_eq!(
        reliquary_dream_pairs(&reopened_rel).unwrap().fingerprint,
        rel_cut.fingerprint
    );
    assert_eq!(
        phylactery_dream_pairs(&reopened_phy).unwrap().fingerprint,
        phy_cut.fingerprint
    );
}

#[test]
#[cfg(feature = "history-oracle-fixtures")]
fn authoritative_dream_pair_sets_match_owner_apis_and_reopen_for_all_fixtures() {
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
                let first = reliquary_dream_pairs(&rel).unwrap();
                let second = reliquary_dream_pairs(&rel).unwrap();
                let api_pairs: BTreeSet<_> = rel
                    .dream_pair_records()
                    .into_iter()
                    .map(|(left, right)| canonical(left, right))
                    .collect();
                assert_eq!(first.pairs, api_pairs, "{corpus}/{kind} owner API parity");
                assert_eq!(first.fingerprint, second.fingerprint);
                drop(rel);
                let reopened = Cva::open(&path).unwrap();
                assert_eq!(
                    reliquary_dream_pairs(&reopened).unwrap().fingerprint,
                    first.fingerprint,
                    "{corpus}/{kind} reopen parity"
                );
            } else {
                let phy = Phylactery::open(&path).unwrap();
                let first = phylactery_dream_pairs(&phy).unwrap();
                let second = phylactery_dream_pairs(&phy).unwrap();
                let api_pairs: BTreeSet<_> = phy
                    .dream_pair_records()
                    .into_iter()
                    .map(|(left, right)| canonical(left, right))
                    .collect();
                assert_eq!(first.pairs, api_pairs, "{corpus}/{kind} owner API parity");
                assert_eq!(first.fingerprint, second.fingerprint);
                drop(phy);
                let reopened = Phylactery::open(&path).unwrap();
                assert_eq!(
                    phylactery_dream_pairs(&reopened).unwrap().fingerprint,
                    first.fingerprint,
                    "{corpus}/{kind} reopen parity"
                );
            }
        }
    }
}
