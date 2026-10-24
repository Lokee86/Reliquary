use super::*;
use crate::{MemoryDraft, Phylactery};
use std::{fs, path::PathBuf};

fn scratch() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("memory-oracle-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn draft(mutation: &str) -> MemoryDraft {
    MemoryDraft {
        category: "preference".into(),
        memory_type: "user".into(),
        authority_kind: "direct".into(),
        temporal_status: "unknown".into(),
        title: "Exact title".into(),
        content: "Exact é body".into(),
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
        source_time_ns: Some(-7),
        mutation_id: mutation.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

#[test]
fn memory_cuts_preserve_revisions_and_terminal_visibility_across_reopen() {
    let dir = scratch();
    let path = dir.join("owner.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    let first = phy.publish_memory(None, 0, draft("create")).unwrap().0;
    let baseline = MemoryHistory::read(&mut phy.owner.container, &phy.owner.memories).unwrap();
    let before = baseline.at(1).unwrap().fingerprint;
    let mut changed = draft("archive");
    changed.archived = true;
    changed.updated_at_ns = 2;
    let second = phy.publish_memory(Some(first.id), 1, changed).unwrap().0;
    assert_eq!(second.revision, 2);
    phy.tombstone_memory(first.id, second.revision).unwrap();
    let history = MemoryHistory::read(&mut phy.owner.container, &phy.owner.memories).unwrap();
    assert_eq!(before, history.at(1).unwrap().fingerprint);
    assert_eq!(history.at(0).unwrap().current.len(), 0);
    assert_eq!(history.at(1).unwrap().current[&first.id], 1);
    assert_eq!(history.at(2).unwrap().current[&first.id], 2);
    assert!(history.at(3).unwrap().current.is_empty());
    assert!(history.at(4).is_err());
    assert_eq!(
        history.revision(first.id, 1).unwrap(),
        baseline.revision(first.id, 1).unwrap()
    );
    assert_ne!(
        history.revision(first.id, 1).unwrap(),
        history.revision(first.id, 2).unwrap()
    );
    assert!(history.revision(first.id, 3).is_err());
    let retained = phy.memory_revision(first.id, 1).unwrap();
    assert_eq!(retained.content, "Exact é body");
    phy.sync().unwrap();
    drop(phy);
    let mut reopened = Phylactery::open(&path).unwrap();
    let after =
        MemoryHistory::read(&mut reopened.owner.container, &reopened.owner.memories).unwrap();
    for cut in 0..=3 {
        assert_eq!(
            history.at(cut).unwrap().fingerprint,
            after.at(cut).unwrap().fingerprint
        );
    }
    drop(reopened);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn memory_oracle_represents_explicitly_pruned_body_without_fabrication() {
    let dir = scratch();
    let mut phy = Phylactery::create(dir.join("owner.phy")).unwrap();
    let memory = phy.publish_memory(None, 0, draft("create")).unwrap().0;
    phy.tombstone_memory(memory.id, memory.revision).unwrap();
    let output = dir.join("pruned.phy");
    phy.prune_tombstoned_history(
        &output,
        crate::HistoryPruneRequest {
            targets: vec![crate::owner_lifecycle::LifecycleTarget::Memory(memory.id)],
        },
    )
    .unwrap();
    let mut pruned = Phylactery::open(&output).unwrap();
    let history = MemoryHistory::read(&mut pruned.owner.container, &pruned.owner.memories).unwrap();
    assert!(history.revision(memory.id, 1).is_ok());
    assert_eq!(
        history
            .bodies
            .values()
            .filter(|v| v.get("pruned").is_some())
            .count(),
        1
    );
    assert!(matches!(
        pruned.historical_memory_revision(memory.id, 1).unwrap(),
        crate::owner_lifecycle::HistoricalPayload::Pruned { .. }
    ));
    drop(pruned);
    drop(phy);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
#[cfg(feature = "history-oracle-fixtures")]
fn every_authoritative_memory_revision_can_be_fingerprinted() {
    let root = PathBuf::from(std::env::var_os("RELIQUARY_HISTORY_FIXTURES").expect("fixture root"));
    let dir = scratch();
    for (index, corpus) in [
        "chatgpt-first14d",
        "chatgpt-first28d",
        "rhelm-david-r-ellis-pre-relationship",
    ]
    .iter()
    .enumerate()
    {
        for (kind, name) in [("rel", "project.prj.rel"), ("phy", "user.phy")] {
            let path = dir.join(format!("{index}.{kind}"));
            let mut source = fs::File::open(root.join(corpus).join(name)).unwrap();
            let mut destination = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            std::io::copy(&mut source, &mut destination).unwrap();
            drop(destination);
            let (history, expected_current) = if kind == "rel" {
                let mut cva = crate::Cva::open(&path).unwrap();
                (
                    MemoryHistory::read(&mut cva.container, &cva.memories).unwrap(),
                    cva.memories.current_ids(),
                )
            } else {
                let mut phy = Phylactery::open(&path).unwrap();
                (
                    MemoryHistory::read(&mut phy.container, &phy.memories).unwrap(),
                    phy.memories.current_ids(),
                )
            };
            assert_eq!(
                history
                    .at(history.head)
                    .unwrap()
                    .current
                    .keys()
                    .copied()
                    .collect::<Vec<_>>(),
                expected_current
            );
            for revision in &history.revisions {
                assert_eq!(
                    history.revision(revision.id, revision.revision).unwrap(),
                    history.revision(revision.id, revision.revision).unwrap()
                );
            }
            for cut in [0, history.head / 2, history.head] {
                assert_eq!(
                    history.at(cut).unwrap().fingerprint,
                    history.at(cut).unwrap().fingerprint
                );
            }
            eprintln!(
                "Memory fixture {corpus}/{kind}: {} revisions, head {}",
                history.revisions.len(),
                history.head
            );
        }
    }
    fs::remove_dir_all(dir).unwrap();
}
