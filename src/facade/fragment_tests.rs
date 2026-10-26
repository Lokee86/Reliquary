use crate::{Branch, Cva, FragmentConfig};
use std::fs;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-fragments-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn append_chain(archive: &mut Cva, conversation: &str, from: usize, to: usize) {
    for index in from..to {
        let parent = (index > 0).then(|| format!("n{}", index - 1));
        archive
            .append_node(
                format!("n{index}"),
                conversation.to_owned(),
                parent,
                if index % 2 == 0 { "user" } else { "assistant" }.to_owned(),
                index as i64,
                &format!("turn {index}"),
            )
            .unwrap();
    }
}

#[test]
fn live_fragmenting_is_append_only() {
    let path = test_path("append.cva");
    let mut archive = Cva::create(&path).unwrap();
    append_chain(&mut archive, "c1", 0, 8);
    let config = FragmentConfig::default();
    let first = archive
        .materialize_path_fragments("c1", "n7", config, false)
        .unwrap();
    assert_eq!(first.len(), 1);
    let stable = first[0].id;

    append_chain(&mut archive, "c1", 8, 14);
    let second = archive
        .materialize_path_fragments("c1", "n13", config, false)
        .unwrap();
    assert_eq!(second.len(), 1);
    assert_eq!(archive.stats().fragments, 2);
    assert_eq!(
        archive
            .fragments()
            .iter()
            .filter(|f| f.id == stable)
            .count(),
        1
    );
}

#[test]
fn closed_tail_is_materialized_without_copying_text() {
    let path = test_path("tail.cva");
    let mut archive = Cva::create(&path).unwrap();
    append_chain(&mut archive, "c1", 0, 10);
    let created = archive
        .materialize_path_fragments("c1", "n9", FragmentConfig::default(), true)
        .unwrap();
    assert_eq!(created.len(), 2);
    let fragments = archive.fragments();
    assert_eq!(fragments.len(), 2);
    let tail = fragments
        .iter()
        .find(|fragment| fragment.end_node_id == "n9")
        .unwrap();
    assert_eq!(tail.start_node_id, "n2");
    let tail_id = tail.id;
    archive.sync().unwrap();
    drop(archive);

    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.stats().fragments, 2);
    assert!(reopened.fragment_text(tail_id).unwrap().contains("turn 9"));
}

#[test]
fn branches_reuse_shared_prefix_fragments() {
    let path = test_path("branches.cva");
    let mut archive = Cva::create(&path).unwrap();
    append_chain(&mut archive, "c1", 0, 10);
    archive
        .append_node(
            "left".into(),
            "c1".into(),
            Some("n9".into()),
            "user".into(),
            10,
            "left",
        )
        .unwrap();
    archive
        .append_node(
            "right".into(),
            "c1".into(),
            Some("n9".into()),
            "user".into(),
            10,
            "right",
        )
        .unwrap();
    for (id, leaf) in [("left-branch", "left"), ("right-branch", "right")] {
        archive
            .append_branch(Branch {
                id: id.into(),
                conversation_id: "c1".into(),
                leaf_node_id: leaf.into(),
                canonical: id == "left-branch",
            })
            .unwrap();
        archive
            .materialize_branch_fragments("c1", id, FragmentConfig::default(), true)
            .unwrap();
    }
    let fragments = archive.fragments();
    assert_eq!(
        fragments
            .iter()
            .filter(|f| f.start_node_id == "n0" && f.end_node_id == "n7")
            .count(),
        1
    );
}
