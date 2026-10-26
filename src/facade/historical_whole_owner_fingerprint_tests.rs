use super::*;
use crate::{
    Cva, EgoAnchorPriority, EpisodeBoundary, EpisodeConfig, EpisodeOrigin, GraphRelationOrigin,
    MemoryDraft, MemoryId, SemanticGraphRelationChange, SemanticGraphRelationKind, SemanticNodeRef,
};
use std::{fs, path::PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("whole-owner-oracle-{}", uuid::Uuid::new_v4()));
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

fn episode(cva: &mut Cva) -> crate::Episode {
    cva.append_node(
        "user".into(),
        "whole-owner".into(),
        None,
        "user".into(),
        10,
        "remember two facts",
    )
    .unwrap();
    cva.append_node(
        "assistant".into(),
        "whole-owner".into(),
        Some("user".into()),
        "assistant".into(),
        20,
        "noted",
    )
    .unwrap();
    cva.materialize_path_episodes(
        "whole-owner",
        "assistant",
        EpisodeConfig::default(),
        EpisodeOrigin::Live,
        Some((EpisodeBoundary::Inactivity, 30)),
    )
    .unwrap()
    .created
    .into_iter()
    .next()
    .unwrap()
}

fn memory(cva: &mut Cva, episode: &crate::Episode, mutation: &str) -> MemoryId {
    cva.publish_memory(
        None,
        0,
        MemoryDraft {
            category: "fact".into(),
            memory_type: "project".into(),
            authority_kind: "unknown".into(),
            temporal_status: "unknown".into(),
            title: mutation.into(),
            content: format!("content {mutation}"),
            scope: "private".into(),
            lifecycle_state: "active".into(),
            archived: false,
            superseded_by: None,
            parent_id: None,
            source_node_id: Some("user".into()),
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: Some(episode.id),
            source_time_ns: None,
            mutation_id: mutation.into(),
            created_at_ns: 30,
            updated_at_ns: 30,
        },
    )
    .unwrap()
    .0
    .id
}

fn assert_same_cut(left: &WholeOwnerCut, right: &WholeOwnerCut) {
    assert_eq!(left.fingerprint, right.fingerprint);
    assert_eq!(left.cuts, right.cuts);
    assert_eq!(left.ego_version, right.ego_version);
    assert_eq!(left.current_memories, right.current_memories);
    assert_eq!(left.visible_graph, right.visible_graph);
}

#[test]
fn whole_owner_global_cut_resolves_interleaved_memory_graph_and_terminal_state() {
    let fixture = Fixture::new();
    let path = fixture.path("owner.rel");
    let mut cva = Cva::create_project(&path).unwrap();

    // Ego has no global ticket. A publication before the first global owner
    // ticket is part of the accepted physical prefix observed at G=0.
    cva.create_ego_anchor(EgoAnchorPriority::Normal, "before first ticket".into())
        .unwrap();
    let zero_cut = whole_owner_at(&mut cva, 0).unwrap();
    assert_eq!(zero_cut.ego_version, 1);
    let episode = episode(&mut cva);

    let first = memory(&mut cva, &episode, "first");
    let first_global = cva.latest_global_version();
    let first_cut = whole_owner_at(&mut cva, first_global).unwrap();
    assert_eq!(first_cut.ego_version, 1);
    assert_eq!(first_cut.cuts.memory, 1);
    assert_eq!(first_cut.cuts.graph, 0);
    assert_eq!(first_cut.cuts.memory_graph, 0);
    assert_eq!(first_cut.current_memories, BTreeMap::from([(first, 1)]));
    assert!(first_cut.visible_graph.is_empty());

    let second = memory(&mut cva, &episode, "second");
    let second_global = cva.latest_global_version();
    let second_cut = whole_owner_at(&mut cva, second_global).unwrap();
    assert!(second_global > first_global);
    assert_eq!(second_cut.cuts.memory, 2);
    assert_eq!(
        second_cut.current_memories,
        BTreeMap::from([(first, 1), (second, 1)])
    );
    assert!(second_cut.visible_graph.is_empty());

    let relation = cva
        .set_semantic_relations_with_origin(
            &[SemanticGraphRelationChange {
                source: SemanticNodeRef::memory(first),
                target: SemanticNodeRef::memory(second),
                kind: SemanticGraphRelationKind::Memory(crate::GraphRelationKind::Factual),
                active: true,
            }],
            GraphRelationOrigin::User,
            cva.graph_version(),
        )
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let graph_global = cva.latest_global_version();
    let graph_cut = whole_owner_at(&mut cva, graph_global).unwrap();
    assert_eq!(graph_cut.cuts.graph, 1);
    assert_eq!(graph_cut.cuts.memory_graph, 1);
    assert_eq!(graph_cut.visible_graph, vec![relation]);

    // Typed state remains at its own G. A new publication's recovery intent is
    // appended before its ticket, so a live-head digest is not a frozen prefix.
    let later = memory(&mut cva, &episode, "later");
    let later_global = cva.latest_global_version();
    assert!(later_global > graph_global);
    cva.create_ego_anchor(EgoAnchorPriority::High, "after later ticket".into())
        .unwrap();
    let later_cut = whole_owner_at(&mut cva, later_global).unwrap();
    assert_eq!(later_cut.ego_version, 2);
    let prior = whole_owner_at(&mut cva, graph_global).unwrap();
    assert_eq!(prior.cuts, graph_cut.cuts);
    assert_eq!(prior.current_memories, graph_cut.current_memories);
    assert_eq!(prior.visible_graph, graph_cut.visible_graph);
    assert_eq!(prior.ego_version, graph_cut.ego_version);
    assert_ne!(
        prior.fingerprint, graph_cut.fingerprint,
        "the pre-ticket recovery intent belongs to the prior frontier"
    );

    cva.tombstone_memory(first, 1).unwrap();
    let terminal_global = cva.latest_global_version();
    let terminal_cut = whole_owner_at(&mut cva, terminal_global).unwrap();
    assert!(terminal_global > later_global);
    assert_eq!(terminal_cut.cuts.memory, 4);
    assert_eq!(
        terminal_cut.current_memories,
        BTreeMap::from([(second, 1), (later, 1)])
    );
    assert!(terminal_cut.visible_graph.is_empty());
    assert_eq!(terminal_cut.cuts.graph, graph_cut.cuts.graph);

    // Freeze the source before recording expected historical fingerprints.
    let zero_cut = whole_owner_at(&mut cva, 0).unwrap();
    let first_cut = whole_owner_at(&mut cva, first_global).unwrap();
    let second_cut = whole_owner_at(&mut cva, second_global).unwrap();
    let graph_cut = whole_owner_at(&mut cva, graph_global).unwrap();
    let later_cut = whole_owner_at(&mut cva, later_global).unwrap();

    cva.sync().unwrap();
    drop(cva);
    let mut reopened = Cva::open(&path).unwrap();
    assert_same_cut(&whole_owner_at(&mut reopened, 0).unwrap(), &zero_cut);
    assert_same_cut(
        &whole_owner_at(&mut reopened, first_global).unwrap(),
        &first_cut,
    );
    assert_same_cut(
        &whole_owner_at(&mut reopened, second_global).unwrap(),
        &second_cut,
    );
    assert_same_cut(
        &whole_owner_at(&mut reopened, graph_global).unwrap(),
        &graph_cut,
    );
    assert_same_cut(
        &whole_owner_at(&mut reopened, later_global).unwrap(),
        &later_cut,
    );
    assert_same_cut(
        &whole_owner_at(&mut reopened, terminal_global).unwrap(),
        &terminal_cut,
    );
}

#[test]
fn global_cut_rejects_a_file_whose_repository_binding_is_only_visible_later() {
    let fixture = Fixture::new();
    let path = fixture.path("late-reference.rel");
    let mut cva = Cva::create_project(&path).unwrap();
    let reference = crate::ProjectFileRef {
        revision: crate::ProjectRevisionRef {
            repository: crate::ProjectRepositoryRef {
                kind: crate::ProjectRepositoryKind::Git,
                repository_id: "oracle-repo".into(),
                project_path: ".".into(),
            },
            revision: "fixed-revision".into(),
        },
        path: "file.txt".into(),
        content_hash: Some([9; 32]),
    };
    let file = reliquary_archive::file_store::build_stored_file_from_content_id(
        "file.txt".into(),
        None,
        crate::ContentId([9; 32]),
        10,
    )
    .unwrap();
    cva.archive
        .register_file(&mut cva.container, file.clone())
        .unwrap();
    let file_global = cva.latest_global_version();
    cva.container.allocate_version().unwrap();
    cva.project_files
        .put(&mut cva.container, file.id, reference)
        .unwrap();
    cva.sync().unwrap();
    drop(cva);
    let mut cva = Cva::open(&path).unwrap();
    assert!(matches!(
        whole_owner_at(&mut cva, file_global),
        Err(GlobalProjectionError::MissingArchiveReference)
    ));
    let head = cva.latest_global_version();
    assert!(whole_owner_at(&mut cva, head).is_ok());
}
