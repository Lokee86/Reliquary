use super::*;
use crate::imports::{ImportBranch, ImportConversation, ImportMessage, NativeImport};
use crate::{Cva, MemoryDraft};
use std::{collections::BTreeSet, fs, path::PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::current_dir()
            .unwrap()
            .join(".test-tmp")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn rel(&self) -> PathBuf {
        self.0.join("scope.rel")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn import() -> NativeImport {
    NativeImport {
        schema_version: 1,
        source_namespace: "scope-test/account".into(),
        source_format: "scope-test-v1".into(),
        conversations: vec![ImportConversation {
            id: "conversation".into(),
            title: None,
            messages: vec![
                message("u1", None, "user", "first"),
                message("a1", Some("u1"), "assistant", "middle"),
                message("u2", Some("a1"), "user", "last"),
            ],
            branches: vec![ImportBranch {
                id: "main".into(),
                leaf_message_id: "u2".into(),
                canonical: true,
            }],
        }],
    }
}

fn message(id: &str, parent_id: Option<&str>, role: &str, content: &str) -> ImportMessage {
    ImportMessage {
        id: id.into(),
        revision: Some("1".into()),
        parent_id: parent_id.map(str::to_owned),
        replaces: None,
        deleted: false,
        role: role.into(),
        timestamp_ns: 100,
        content: content.into(),
        artifacts: vec![],
    }
}

fn existing_memory(cva: &mut Cva) -> crate::MemoryId {
    cva.publish_memory(
        None,
        0,
        MemoryDraft {
            category: "decision".into(),
            memory_type: "project".into(),
            authority_kind: "direct".into(),
            temporal_status: "current".into(),
            title: "Pre-import memory".into(),
            content: "Retained before the import starts".into(),
            scope: "private".into(),
            lifecycle_state: "knowledge".into(),
            archived: false,
            superseded_by: None,
            parent_id: None,
            source_node_id: None,
            source_episode_id: None,
            source_time_ns: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            mutation_id: "scope-test-existing-memory".into(),
            created_at_ns: 10,
            updated_at_ns: 10,
        },
    )
    .unwrap()
    .0
    .id
}

#[test]
fn scope_is_stable_across_retry_and_reopen_and_excludes_preexisting_memory() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let mut cva = Cva::create(&path).unwrap();
    let existing = existing_memory(&mut cva);
    let input = import();
    cva.import_native(&input, 200).unwrap();
    let first = cva.import_epoch_scope().unwrap();
    let first_fingerprint = (
        first.start.clone(),
        first.start_offset,
        first.marker_end,
        first.start_digest,
        first.base_cut,
        first.source_nodes.clone(),
        first.episodes.clone(),
        first.cohort.clone(),
    );
    assert_eq!(first.source_nodes.len(), 3);
    assert_eq!(first.episodes.len(), 1);
    assert!(!first.cohort.contains(&existing));
    drop(first);
    drop(cva);

    let mut reopened = Cva::open(&path).unwrap();
    reopened.import_native(&input, 300).unwrap();
    let retried = reopened.import_epoch_scope().unwrap();
    assert_eq!(
        first_fingerprint,
        (
            retried.start,
            retried.start_offset,
            retried.marker_end,
            retried.start_digest,
            retried.base_cut,
            retried.source_nodes,
            retried.episodes,
            retried.cohort,
        )
    );
    drop(reopened);
}

#[test]
fn episode_attribution_requires_every_path_node_to_belong_to_receipt() {
    let fixture = Fixture::new();
    let mut cva = Cva::create(fixture.rel()).unwrap();
    cva.import_native(&import(), 200).unwrap();
    let episode = cva
        .episodes()
        .into_iter()
        .find(|episode| episode.origin == EpisodeOrigin::Import)
        .unwrap();
    let endpoints_only = BTreeSet::from([
        (
            episode.conversation_id.clone(),
            episode.start_node_id.clone(),
        ),
        (episode.conversation_id.clone(), episode.end_node_id.clone()),
    ]);
    assert!(!episode_matches_source(&mut cva, &episode, &endpoints_only).unwrap());

    let scope = cva.import_epoch_scope().unwrap();
    assert!(episode_matches_source(&mut cva, &episode, &scope.source_nodes).unwrap());
}

#[test]
fn empty_receipt_source_set_is_valid_for_zero_result_import() {
    let fixture = Fixture::new();
    let mut cva = Cva::create(fixture.rel()).unwrap();
    let empty = NativeImport {
        schema_version: 1,
        source_namespace: "scope-test/empty".into(),
        source_format: "scope-test-v1".into(),
        conversations: vec![],
    };
    cva.import_native(&empty, 200).unwrap();
    let scope = cva.import_epoch_scope().unwrap();
    assert!(scope.source_nodes.is_empty());
    assert!(scope.episodes.is_empty());
    assert!(scope.cohort.is_empty());
}
