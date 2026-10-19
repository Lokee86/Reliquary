use super::*;
use crate::{EntityDraft, EntityRef, MemoryDraft, MemoryRef, RelationshipParticipant};
use std::fs;

fn test_path(name: &str) -> std::path::PathBuf {
    let directory = std::env::current_dir().unwrap().join(".test-tmp");
    fs::create_dir_all(&directory).unwrap();
    directory.join(format!(
        "rewrite-semantic-{name}-{}.db",
        uuid::Uuid::new_v4()
    ))
}

fn memory_draft(mutation_id: &str, text: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: text.into(),
        content: text.into(),
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
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

fn entity_draft() -> EntityDraft {
    EntityDraft {
        canonical_name: "Semantic target".into(),
        aliases: vec!["Target".into()],
        kind: "organization".into(),
        summary: "Current semantic entity".into(),
        mutation_id: "rewrite-semantic-entity".into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

fn publish_three_memories_rel(path: &std::path::Path) -> (Cva, [MemoryId; 3]) {
    let mut source = Cva::create_project(path).unwrap();
    let ids = ["a", "b", "c"].map(|label| {
        source
            .publish_memory(None, 0, memory_draft(&format!("memory-{label}"), label))
            .unwrap()
            .0
            .id
    });
    (source, ids)
}

fn relate_three(source: &mut Cva, ids: [MemoryId; 3]) -> (EntityId, crate::RelationshipId) {
    let entity = source.publish_entity(None, 0, entity_draft()).unwrap().0;
    let owner = source.owner_id().unwrap();
    let relationship = source
        .publish_relationship(
            None,
            0,
            RelationshipDraft {
                kind: "supports".into(),
                participants: vec![RelationshipParticipant {
                    entity: EntityRef {
                        owner_id: owner.clone(),
                        entity_id: entity.id,
                    },
                    role: Some("subject".into()),
                }],
                evidence: vec![
                    MemoryRef {
                        owner_id: owner.clone(),
                        memory_id: ids[0],
                    },
                    MemoryRef {
                        owner_id: owner,
                        memory_id: ids[1],
                    },
                ],
                summary: "Evidence-linked relationship".into(),
                mutation_id: "rewrite-semantic-relationship".into(),
                created_at_ns: 10,
                updated_at_ns: 10,
            },
        )
        .unwrap()
        .0;
    source
        .set_memory_relation_with_origin(
            ids[1],
            ids[2],
            crate::GraphRelationKind::Factual,
            true,
            GraphRelationOrigin::User,
            source.graph_version(),
        )
        .unwrap();
    source
        .set_entity_association(ids[0], entity.id, true, source.graph_version())
        .unwrap();
    (entity.id, relationship.id)
}

#[test]
fn relationship_evidence_and_current_graph_references_expand_retained_memories() {
    let path = test_path("closure-rel");
    let (mut source, ids) = publish_three_memories_rel(&path);
    let (entity, relationship) = relate_three(&mut source, ids);
    let closure = semantic_closure_rel(&source, &BTreeSet::from([ids[0]])).unwrap();
    assert_eq!(
        closure.expanded_memory_ids,
        BTreeSet::from([ids[1], ids[2]])
    );
    assert_eq!(
        closure.entities.iter().map(|e| e.id).collect::<Vec<_>>(),
        vec![entity]
    );
    assert_eq!(
        closure
            .relationships
            .iter()
            .map(|r| r.id)
            .collect::<Vec<_>>(),
        vec![relationship]
    );
    assert_eq!(closure.graph_relations.len(), 2);
    drop(source);
    let _ = fs::remove_file(path);
}

#[test]
fn rel_copy_remaps_local_evidence_and_writes_only_a_current_graph_baseline() {
    let source_path = test_path("copy-rel-source");
    let output_path = test_path("copy-rel-output");
    let (mut source, ids) = publish_three_memories_rel(&source_path);
    let (entity, relationship) = relate_three(&mut source, ids);
    let anchor = source
        .create_ego_anchor(
            crate::EgoAnchorPriority::Normal,
            "Retained REL anchor".into(),
        )
        .unwrap();
    source
        .put_ego_web_synthesis(0, source.memory_version(), "Omitted REL synthesis".into())
        .unwrap();
    let retained = BTreeSet::from([ids[0]]);
    let closure = semantic_closure_rel(&source, &retained).unwrap();
    let retained = retained
        .into_iter()
        .chain(closure.expanded_memory_ids.iter().copied())
        .collect::<BTreeSet<_>>();

    let mut output = Cva::create_project(&output_path).unwrap();
    for id in &retained {
        output
            .publish_memory(
                Some(*id),
                0,
                memory_draft(&format!("rewritten-{}", hex32(&id.0)), "rewritten"),
            )
            .unwrap();
    }
    copy_semantic_state_rel(&source, &mut output, &closure, false).unwrap();
    assert_eq!(output.entities()[0].id, entity);
    let copied_relationship = output.relationship(relationship).unwrap();
    assert_eq!(copied_relationship.evidence.len(), 2);
    assert!(
        copied_relationship
            .evidence
            .iter()
            .all(|r| r.owner_id == output.owner_id().unwrap())
    );
    assert!(
        copied_relationship
            .participants
            .iter()
            .all(|r| r.entity.owner_id == output.owner_id().unwrap())
    );
    assert_eq!(output.semantic_graph_relations().len(), 2);
    assert_eq!(
        output.ego_anchor(anchor.id).unwrap().text,
        "Retained REL anchor"
    );
    assert!(output.ego_web_synthesis().is_none());
    drop(output);
    drop(source);
    let reopened = Cva::open(&output_path).unwrap();
    assert_eq!(reopened.semantic_graph_relations().len(), 2);
    assert_eq!(
        reopened.ego_anchor(anchor.id).unwrap().text,
        "Retained REL anchor"
    );
    assert!(reopened.ego_web_synthesis().is_none());
    drop(reopened);
    let _ = fs::remove_file(source_path);
    let _ = fs::remove_file(output_path);
}

#[test]
fn phy_copy_preserves_authoritative_ego_and_omits_synthesis_cache() {
    let source_path = test_path("copy-phy-source");
    let output_path = test_path("copy-phy-output");
    let mut source = Phylactery::create(&source_path).unwrap();
    let memory = source
        .publish_memory(None, 0, memory_draft("phy-memory", "selected"))
        .unwrap()
        .0;
    let entity = source.publish_entity(None, 0, entity_draft()).unwrap().0;
    source
        .set_entity_association(memory.id, entity.id, true, source.graph_version())
        .unwrap();
    let first = source
        .create_ego_identity("First".into(), "First identity".into())
        .unwrap();
    let active = source
        .create_ego_identity("Active".into(), "Active identity".into())
        .unwrap();
    source.activate_ego_identity(active.id).unwrap();
    source
        .put_ego_personality(0, "User personality".into(), source.memory_version())
        .unwrap();
    let anchor = source
        .create_ego_anchor(crate::EgoAnchorPriority::High, "Keep this anchor".into())
        .unwrap();
    source
        .put_ego_web_synthesis(0, source.memory_version(), "Derived cache".into())
        .unwrap();

    let closure = semantic_closure_phy(&source, &BTreeSet::from([memory.id])).unwrap();
    let mut output = Phylactery::create(&output_path).unwrap();
    output
        .publish_memory(
            Some(memory.id),
            0,
            memory_draft("rewritten-phy-memory", "selected"),
        )
        .unwrap();
    copy_semantic_state_phy(&source, &mut output, &closure).unwrap();
    assert_eq!(output.entity(entity.id).unwrap().id, entity.id);
    assert_eq!(output.active_ego_identity_id(), Some(active.id));
    assert_eq!(
        output.ego_identity_by_id(first.id).unwrap().text,
        "First identity"
    );
    assert_eq!(output.ego_personality().unwrap().text, "User personality");
    assert_eq!(
        output.ego_anchor(anchor.id).unwrap().text,
        "Keep this anchor"
    );
    assert!(output.ego_web_synthesis().is_none());
    assert_eq!(output.semantic_graph_relations().len(), 1);
    drop(output);
    drop(source);
    let reopened = Phylactery::open(&output_path).unwrap();
    assert_eq!(reopened.active_ego_identity_id(), Some(active.id));
    assert_eq!(
        reopened.ego_anchor(anchor.id).unwrap().text,
        "Keep this anchor"
    );
    assert!(reopened.ego_web_synthesis().is_none());
    drop(reopened);
    let _ = fs::remove_file(source_path);
    let _ = fs::remove_file(output_path);
}
