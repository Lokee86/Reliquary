use crate::historical_ego_fingerprint::EgoHistory;
use crate::{Cva, EgoAnchorPriority, MemoryDraft, Phylactery};
use reliquary_container::Container;
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);
impl Scratch {
    fn new(label: &str) -> Self {
        let root = std::env::current_dir().unwrap().join(".test-tmp");
        fs::create_dir_all(&root).unwrap();
        Self(root.join(format!(
            "ego-compact-writer-{label}-{}.db",
            uuid::Uuid::new_v4()
        )))
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn payloads(container: &Container) -> Vec<Vec<u8>> {
    let mut result = Vec::new();
    let source = container.read_source().unwrap();
    let mut cursor = source.physical_cursor().unwrap();
    while let Some(frame) = cursor.next_frame().unwrap() {
        result.push(frame.payload);
    }
    result
}

fn assert_compact_only(container: &Container) {
    let ps = payloads(container);
    let ego: Vec<_> = ps
        .iter()
        .filter(|p| {
            [
                b"CVAECR".as_slice(),
                b"CVAEDL",
                b"CVAETM",
                b"CVAEID",
                b"CVAEANC",
                b"CVAEPER",
                b"CVAESYN",
            ]
            .iter()
            .any(|family| p.starts_with(family))
        })
        .collect();
    assert!(!ego.is_empty());
    for p in ego {
        assert!(
            p.starts_with(b"CVAECR01")
                || p.starts_with(b"CVAEDL01")
                || p.starts_with(b"CVAETM01")
                || p.starts_with(b"CVAEIDA1"),
            "unexpected full Ego postimage frame: {:?}",
            &p[..p.len().min(8)]
        );
    }
}

fn ego_history_rel(cva: &Cva) -> EgoHistory {
    EgoHistory::read(&cva.container, &cva.ego).unwrap()
}
fn ego_history_phy(phy: &Phylactery) -> EgoHistory {
    EgoHistory::read(&phy.container, &phy.ego).unwrap()
}

fn memory_draft() -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: "selected memory".into(),
        content: "content".into(),
        scope: "private".into(),
        lifecycle_state: "active".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        mutation_id: "ego-writer-memory".into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

#[test]
fn ordinary_rel_writes_are_compact_reopenable_and_metadata_deltas_stay_small() {
    let path = Scratch::new("rel");
    let large = "unchanged standing text ".repeat(90);
    let mut rel = Cva::create_project(path.path()).unwrap();
    rel.publish_memory(None, 0, memory_draft()).unwrap();
    let anchor = rel
        .create_ego_anchor(EgoAnchorPriority::Normal, large.clone())
        .unwrap();
    let (synthesis, changed) = rel.put_ego_web_synthesis(0, 1, large.clone()).unwrap();
    assert!(changed);
    let before = rel.ego_version();
    rel.sync().unwrap();
    let bytes_before_noop = fs::metadata(path.path()).unwrap().len();
    let (same, changed) = rel
        .put_ego_web_synthesis(synthesis.revision, 1, large.clone())
        .unwrap();
    assert!(!changed);
    assert_eq!(same.revision, synthesis.revision);
    assert_eq!(
        rel.ego_version(),
        before,
        "semantic no-op must not publish an Ego version"
    );
    rel.sync().unwrap();
    assert_eq!(
        fs::metadata(path.path()).unwrap().len(),
        bytes_before_noop,
        "semantic no-op must not append bytes"
    );
    let (updated_anchor, changed) = rel
        .update_ego_anchor(
            anchor.id,
            anchor.revision,
            EgoAnchorPriority::High,
            large.clone(),
        )
        .unwrap();
    assert!(changed);
    let (updated_synthesis, changed) = rel
        .put_ego_web_synthesis(synthesis.revision, 0, large.clone())
        .unwrap();
    assert!(changed, "watermark-only update is a publication");
    assert_eq!(updated_synthesis.source_memory_version, 0);
    assert_eq!(updated_anchor.text, large);
    assert_compact_only(&rel.container);
    let ps = payloads(&rel.container);
    let deltas: Vec<_> = ps.iter().filter(|p| p.starts_with(b"CVAEDL01")).collect();
    assert!(deltas.len() >= 2);
    assert!(
        deltas.iter().all(|p| p.len() < 512),
        "metadata deltas copied unchanged large text"
    );
    rel.delete_ego_anchor(anchor.id, updated_anchor.revision)
        .unwrap();
    let head = rel.ego_version();
    let history = ego_history_rel(&rel);
    let cuts: Vec<_> = (0..=head)
        .map(|i| history.at(i).unwrap().fingerprint)
        .collect();
    assert!(
        payloads(&rel.container)
            .iter()
            .any(|p| p.starts_with(b"CVAETM01"))
    );
    rel.sync().unwrap();
    drop(rel);
    let mut reopened = Cva::open(path.path()).unwrap();
    assert_eq!(reopened.ego_web_synthesis().unwrap().text, large);
    assert_eq!(
        reopened.ego_web_synthesis().unwrap().source_memory_version,
        0
    );
    assert!(reopened.ego_anchor(anchor.id).is_none());
    let reopened_history = ego_history_rel(&reopened);
    for (cut, fp) in cuts.iter().enumerate() {
        assert_eq!(&reopened_history.at(cut as u64).unwrap().fingerprint, fp);
    }
    assert_eq!(reopened.ego_version(), head);
}

#[test]
fn ordinary_phy_writes_preserve_exact_text_activation_and_terminal_semantics() {
    let path = Scratch::new("phy");
    let mut phy = Phylactery::create(path.path()).unwrap();
    phy.publish_memory(None, 0, memory_draft()).unwrap();
    let text = "\t  exact whitespace and personality text  \n".to_owned();
    let first = phy
        .create_ego_identity("Default".into(), text.clone())
        .unwrap();
    let second = phy
        .create_ego_identity("Work".into(), "work identity".into())
        .unwrap();
    // Creation of a subsequent identity is independent of activation; selecting it
    // produces the minimal activation publication.
    assert_eq!(phy.active_ego_identity_id(), Some(first.id));
    assert!(phy.activate_ego_identity(second.id).unwrap());
    let (revised, changed) = phy
        .update_ego_identity(first.id, first.revision, "Renamed".into(), text.clone())
        .unwrap();
    assert!(changed);
    let (personality, changed) = phy.put_ego_personality(0, text.clone(), 0).unwrap();
    assert!(changed);
    let anchor = phy
        .create_ego_anchor(EgoAnchorPriority::Low, text.clone())
        .unwrap();
    let (anchor, changed) = phy
        .update_ego_anchor(
            anchor.id,
            anchor.revision,
            EgoAnchorPriority::High,
            text.clone(),
        )
        .unwrap();
    assert!(changed);
    let (synthesis, changed) = phy.put_ego_web_synthesis(0, 0, text.clone()).unwrap();
    assert!(changed);
    let pre_delete = phy.ego_version();
    assert!(
        phy.delete_ego_identity(second.id, second.revision).is_err(),
        "active Identity deletion is rejected"
    );
    assert_eq!(phy.ego_version(), pre_delete);
    assert!(phy.activate_ego_identity(first.id).unwrap());
    assert!(phy.delete_ego_identity(second.id, second.revision).unwrap());
    assert_compact_only(&phy.container);
    let frames = payloads(&phy.container);
    assert_eq!(
        frames.iter().filter(|p| p.starts_with(b"CVAEIDA1")).count(),
        2
    );
    assert!(frames.iter().any(|p| p.starts_with(b"CVAETM01")));
    assert_eq!(phy.ego_identity_by_id(first.id).unwrap().text, text);
    assert_eq!(
        phy.ego_personality().unwrap().text,
        "\t  exact whitespace and personality text  \n"
    );
    assert_eq!(
        phy.ego_anchor(anchor.id).unwrap().priority,
        EgoAnchorPriority::High
    );
    assert_eq!(
        phy.ego_web_synthesis().unwrap().text,
        "\t  exact whitespace and personality text  \n"
    );
    let head = phy.ego_version();
    let history = ego_history_phy(&phy);
    let cuts: Vec<_> = (0..=head)
        .map(|i| history.at(i).unwrap().fingerprint)
        .collect();
    phy.sync().unwrap();
    drop(phy);
    let reopened = Phylactery::open(path.path()).unwrap();
    assert_eq!(
        reopened.ego_identity_by_id(first.id).unwrap().revision,
        revised.revision
    );
    assert_eq!(
        reopened.ego_identity_by_id(first.id).unwrap().name,
        "Renamed"
    );
    assert_eq!(
        reopened.ego_personality().unwrap().revision,
        personality.revision
    );
    assert_eq!(
        reopened.ego_web_synthesis().unwrap().revision,
        synthesis.revision
    );
    assert_eq!(
        reopened.ego_anchor(anchor.id).unwrap().text,
        "\t  exact whitespace and personality text  \n"
    );
    let after = ego_history_phy(&reopened);
    for (cut, fp) in cuts.iter().enumerate() {
        assert_eq!(&after.at(cut as u64).unwrap().fingerprint, fp);
    }
}

#[test]
fn public_fresh_history_rewrite_emits_compact_rel_and_phy_ego_state() {
    use crate::rewrite::{RewriteRequest, rewrite_file};

    let rel_source_path = Scratch::new("rewrite-rel-source");
    let rel_output_path = Scratch::new("rewrite-rel-output");
    let mut rel_source = Cva::create_project(rel_source_path.path()).unwrap();
    let anchor = rel_source
        .create_ego_anchor(EgoAnchorPriority::High, "REL retained anchor".into())
        .unwrap();
    rel_source
        .put_ego_web_synthesis(0, 0, "REL retained synthesis".into())
        .unwrap();
    rel_source.sync().unwrap();
    drop(rel_source);
    let request = RewriteRequest {
        include_rel_ego_synthesis: true,
        ..RewriteRequest::default()
    };
    rewrite_file(rel_source_path.path(), rel_output_path.path(), &request).unwrap();
    let mut rel_output = Cva::open(rel_output_path.path()).unwrap();
    assert_compact_only(&rel_output.container);
    assert_eq!(rel_output.ego_version(), 2);
    let rewritten_anchor = rel_output.ego_anchor(anchor.id).unwrap();
    assert_eq!(rewritten_anchor.revision, 1);
    assert_eq!(rewritten_anchor.priority, EgoAnchorPriority::High);
    assert_eq!(rewritten_anchor.text, "REL retained anchor");
    let synthesis = rel_output.ego_web_synthesis().unwrap();
    assert_eq!(synthesis.revision, 1);
    assert_eq!(synthesis.source_memory_version, rel_output.memory_version());
    assert_eq!(synthesis.text, "REL retained synthesis");
    let rel_history = ego_history_rel(&rel_output);
    let rel_fingerprints: Vec<_> = (0..=rel_output.ego_version())
        .map(|cut| rel_history.at(cut).unwrap().fingerprint)
        .collect();
    assert!(
        payloads(&rel_output.container)
            .iter()
            .filter(|p| p.starts_with(b"CVAECR")
                || p.starts_with(b"CVAEDL")
                || p.starts_with(b"CVAETM")
                || p.starts_with(b"CVAEIDA"))
            .all(|p| p.starts_with(b"CVAECR01"))
    );
    rel_output.sync().unwrap();
    drop(rel_output);
    let rel_reopened = Cva::open(rel_output_path.path()).unwrap();
    let rel_reopened_history = ego_history_rel(&rel_reopened);
    for (cut, expected) in rel_fingerprints.iter().enumerate() {
        assert_eq!(
            &rel_reopened_history.at(cut as u64).unwrap().fingerprint,
            expected
        );
    }
    assert_eq!(
        rel_reopened.ego_anchor(anchor.id).unwrap().text,
        "REL retained anchor"
    );
    assert_eq!(
        rel_reopened.ego_web_synthesis().unwrap().text,
        "REL retained synthesis"
    );

    let phy_source_path = Scratch::new("rewrite-phy-source");
    let phy_output_path = Scratch::new("rewrite-phy-output");
    let mut phy_source = Phylactery::create(phy_source_path.path()).unwrap();
    let first = phy_source
        .create_ego_identity("Default".into(), "authoritative default".into())
        .unwrap();
    let selected = phy_source
        .create_ego_identity("Selected".into(), "selected exact identity".into())
        .unwrap();
    phy_source.activate_ego_identity(selected.id).unwrap();
    phy_source
        .put_ego_personality(0, "\t exact personality \n".into(), 0)
        .unwrap();
    let anchor = phy_source
        .create_ego_anchor(EgoAnchorPriority::Low, "PHY retained anchor".into())
        .unwrap();
    phy_source
        .put_ego_web_synthesis(0, 0, "derived cache dropped by rewrite".into())
        .unwrap();
    phy_source.sync().unwrap();
    drop(phy_source);
    rewrite_file(
        phy_source_path.path(),
        phy_output_path.path(),
        &RewriteRequest::default(),
    )
    .unwrap();
    let mut phy_output = Phylactery::open(phy_output_path.path()).unwrap();
    assert_compact_only(&phy_output.container);
    let activation_count = usize::from(selected.id > first.id);
    assert_eq!(phy_output.ego_version(), 4 + activation_count as u64); // sorted creations may already select the target
    assert_eq!(phy_output.active_ego_identity_id(), Some(selected.id));
    assert_eq!(phy_output.ego_identity_by_id(first.id).unwrap().revision, 1);
    assert_eq!(
        phy_output.ego_identity_by_id(first.id).unwrap().text,
        "authoritative default"
    );
    assert_eq!(
        phy_output.ego_identity_by_id(selected.id).unwrap().revision,
        1
    );
    assert_eq!(
        phy_output.ego_identity_by_id(selected.id).unwrap().name,
        "Selected"
    );
    assert_eq!(
        phy_output.ego_identity_by_id(selected.id).unwrap().text,
        "selected exact identity"
    );
    assert_eq!(phy_output.ego_personality().unwrap().revision, 1);
    assert_eq!(
        phy_output.ego_personality().unwrap().text,
        "\t exact personality \n"
    );
    assert_eq!(
        phy_output.ego_personality().unwrap().source_memory_version,
        phy_output.memory_version()
    );
    assert_eq!(phy_output.ego_anchor(anchor.id).unwrap().revision, 1);
    assert_eq!(
        phy_output.ego_anchor(anchor.id).unwrap().text,
        "PHY retained anchor"
    );
    assert!(
        phy_output.ego_web_synthesis().is_none(),
        "derived PHY synthesis is omitted"
    );
    let phy_frames = payloads(&phy_output.container);
    assert_eq!(
        phy_frames
            .iter()
            .filter(|p| p.starts_with(b"CVAEIDA1"))
            .count(),
        activation_count,
        "fresh history selects the active identity when sorted creation did not already select it"
    );
    let phy_history = ego_history_phy(&phy_output);
    let phy_fingerprints: Vec<_> = (0..=phy_output.ego_version())
        .map(|cut| phy_history.at(cut).unwrap().fingerprint)
        .collect();
    phy_output.sync().unwrap();
    drop(phy_output);
    let phy_reopened = Phylactery::open(phy_output_path.path()).unwrap();
    let phy_reopened_history = ego_history_phy(&phy_reopened);
    for (cut, expected) in phy_fingerprints.iter().enumerate() {
        assert_eq!(
            &phy_reopened_history.at(cut as u64).unwrap().fingerprint,
            expected
        );
    }
    assert_eq!(phy_reopened.active_ego_identity_id(), Some(selected.id));
    assert_eq!(
        phy_reopened.ego_personality().unwrap().text,
        "\t exact personality \n"
    );
    assert_eq!(
        phy_reopened.ego_anchor(anchor.id).unwrap().text,
        "PHY retained anchor"
    );
    assert!(phy_reopened.ego_web_synthesis().is_none());
}
