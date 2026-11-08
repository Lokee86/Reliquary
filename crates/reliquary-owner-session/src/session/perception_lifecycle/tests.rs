use super::support::owner_hex;
use super::*;
use reliquary_container::ReliquaryScope;
use reliquary_memory_core::{MemoryDraft, MemorySourceRef};
use reliquary_perception_store::{EntityDraft, EntityResolutionReason};
use std::{fs, path::PathBuf};

struct Temp(PathBuf);
impl Temp {
    fn new(suffix: &str) -> Self {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&dir).unwrap();
        Self(dir.join(format!(
            "perception-lifecycle-{}-{suffix}.rel",
            std::process::id()
        )))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn memory_draft(
    owner: [u8; 16],
    mutation: &str,
    content: &str,
    archived: bool,
    superseded_by: Option<MemoryId>,
    at: i64,
) -> MemoryDraft {
    MemoryDraft {
        title: "Accepted source".into(),
        content: content.into(),
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "project".into(),
        lifecycle_state: if archived { "archived" } else { "current" }.into(),
        archived,
        parent_id: None,
        superseded_by,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_ref: Some(MemorySourceRef {
            owner_id: owner_hex(&owner),
            principal_id: Some("principal-a".into()),
            source_episode_id: [7; 32],
            source_node_id: "accepted-turn".into(),
            content_source_conversation_id: Some("conversation".into()),
            content_source_node_id: Some("accepted-turn".into()),
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
        }),
        source_time_ns: Some(at),
        temporal_inference: None,
        mutation_id: mutation.into(),
        created_at_ns: at,
        updated_at_ns: at,
    }
}

fn publish_memory(writer: &mut RelWriter, draft: MemoryDraft, at: i64) -> MemoryRevision {
    let result = writer
        .session
        .common
        .memories
        .publish_versioned_creation(&mut writer.container, draft, at)
        .unwrap();
    writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    result.revision
}

fn retirement_request(
    owner: [u8; 16],
    source: &MemoryRevision,
    successor: &MemoryRevision,
) -> MemoryRetirementRequest {
    MemoryRetirementRequest {
        owner_id: owner,
        memory_id: source.record.id,
        memory_revision: source.record.revision,
        body_id: source.record.body_id,
        superseded_by: successor.record.id,
        successor_revision: successor.record.revision,
        successor_body_id: successor.record.body_id,
    }
}

#[test]
fn memory_retirement_requires_current_owner_and_accepted_supersession_then_reopens() {
    let temp = Temp::new("memory");
    let owner = [71; 16];
    let mut writer = RelWriter::create_path(&temp.0, ReliquaryScope::Project, owner).unwrap();
    let successor_id = reliquary_memory_core::memory_id("successor");
    let old = publish_memory(
        &mut writer,
        memory_draft(
            owner,
            "old",
            "The former source says Mira coordinates Lantern.",
            true,
            Some(successor_id),
            10,
        ),
        10,
    );
    let successor = publish_memory(
        &mut writer,
        memory_draft(
            owner,
            "successor",
            "Mira now coordinates Lantern.",
            false,
            None,
            20,
        ),
        20,
    );
    let request = retirement_request(owner, &old, &successor);

    let before = fs::metadata(&temp.0).unwrap().len();
    let mut wrong_owner = request.clone();
    wrong_owner.owner_id = [72; 16];
    assert!(writer.retire_memory(wrong_owner).is_err());
    let mut stale = request.clone();
    stale.memory_revision += 1;
    assert!(writer.retire_memory(stale).is_err());
    assert_eq!(fs::metadata(&temp.0).unwrap().len(), before);

    let tombstone = writer.retire_memory(request).unwrap();
    assert_eq!(tombstone.revision, old.record.revision + 1);
    drop(writer);
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let reopened = RelSession::open_path(&temp.0, &admission, [], []).unwrap();
    assert_eq!(
        reopened
            .memory_history(old.record.id)
            .unwrap()
            .unwrap()
            .tombstone,
        Some(tombstone)
    );
    assert!(reopened.current_memory(successor.record.id).is_ok());
}

#[test]
fn entity_retirement_rejects_active_mentions_and_reopens_after_supported_unresolve() {
    let temp = Temp::new("entity");
    let owner = [73; 16];
    let mut writer = RelWriter::create_path(&temp.0, ReliquaryScope::Project, owner).unwrap();
    let memory = publish_memory(
        &mut writer,
        memory_draft(
            owner,
            "source",
            "Mira reviews the Lantern checklist.",
            false,
            None,
            10,
        ),
        10,
    );
    let entity = writer
        .session
        .common
        .perception
        .publish_entity(
            &mut writer.container,
            EntityDraft {
                canonical_name: "Mira".into(),
                aliases: vec![],
                kind: "person".into(),
                summary: "Synthetic contact".into(),
                mutation_id: "entity-mira".into(),
                created_at_ns: 10,
                updated_at_ns: 10,
            },
        )
        .unwrap();
    writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    let key = MemoryEntityMentionKey {
        memory_id: memory.record.id.0,
        field: MemoryTextField::Content,
        start_byte: 0,
        end_byte: 4,
    };
    writer
        .commit_entity_mention(EntityMentionCommit {
            owner_id: owner,
            memory_revision: memory.record.revision,
            body_id: memory.record.body_id,
            key,
            surface: "Mira".into(),
            expected_revision: 0,
            updated_at_ns: 11,
            status: MemoryEntityResolutionStatus::Resolved {
                entity_id: entity.id,
                reason: EntityResolutionReason::NamedReferent,
            },
            target_revisions: vec![(entity.id, entity.revision)],
            new_entity: None,
        })
        .unwrap();
    let request = EntityRetirementRequest {
        owner_id: owner,
        entity_id: entity.id,
        entity_revision: entity.revision,
        source_memory_id: memory.record.id,
        source_memory_revision: memory.record.revision,
        source_body_id: memory.record.body_id,
        source_field: MemoryTextField::Content,
        start_byte: 0,
        end_byte: 4,
        source_quote: "Mira".into(),
    };
    let before = fs::metadata(&temp.0).unwrap().len();
    assert!(writer.retire_entity(request.clone()).is_err());
    assert_eq!(fs::metadata(&temp.0).unwrap().len(), before);
    let withdrawn = writer
        .withdraw_entity_mention(WithdrawEntityMentionRequest {
            owner_id: owner,
            memory_id: memory.record.id,
            memory_revision: memory.record.revision,
            body_id: memory.record.body_id,
            key,
            source_surface: "Mira".into(),
            expected_mention_revision: 1,
            target_revisions: vec![(entity.id, entity.revision)],
            withdrawn_at_ns: 12,
        })
        .unwrap();
    assert_eq!(
        withdrawn.status,
        MemoryEntityResolutionStatus::Unresolved {
            reason: EntityResolutionReason::UserWithdrawal,
        }
    );
    let tombstone = writer.retire_entity(request).unwrap();
    drop(writer);
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let reopened = RelSession::open_path(&temp.0, &admission, [], []).unwrap();
    assert_eq!(reopened.entity_tombstone(entity.id), Some(tombstone));
    assert_eq!(
        reopened.mention(key).unwrap().status,
        MemoryEntityResolutionStatus::Unresolved {
            reason: EntityResolutionReason::UserWithdrawal,
        }
    );
}

#[test]
fn relationship_creation_requires_exact_source_and_retirement_preserves_tombstone() {
    let temp = Temp::new("relationship");
    let owner = [74; 16];
    let mut writer = RelWriter::create_path(&temp.0, ReliquaryScope::Project, owner).unwrap();
    let memory = publish_memory(
        &mut writer,
        memory_draft(
            owner,
            "source",
            "Mira coordinates the Lantern review.",
            false,
            None,
            10,
        ),
        10,
    );
    let mut entities = Vec::new();
    for (name, mutation) in [("Mira", "entity-mira"), ("Lantern", "entity-lantern")] {
        entities.push(
            writer
                .session
                .common
                .perception
                .publish_entity(
                    &mut writer.container,
                    EntityDraft {
                        canonical_name: name.into(),
                        aliases: vec![],
                        kind: "referent".into(),
                        summary: format!("{name} is source-bound."),
                        mutation_id: mutation.into(),
                        created_at_ns: 10,
                        updated_at_ns: 10,
                    },
                )
                .unwrap(),
        );
        writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    }
    let request = CreateRelationshipRequest {
        owner_id: owner,
        kind: "coordinates".into(),
        participants: entities
            .iter()
            .map(|entity| RelationshipParticipantEvidence {
                entity_id: entity.id,
                entity_revision: entity.revision,
                role: Some("participant".into()),
            })
            .collect(),
        source_memory_id: memory.record.id,
        source_memory_revision: memory.record.revision,
        source_body_id: memory.record.body_id,
        source_field: MemoryTextField::Content,
        start_byte: 0,
        end_byte: "Mira coordinates the Lantern review.".len(),
        source_quote: "Mira coordinates the Lantern review.".into(),
        mutation_id: "relationship-mira-lantern".into(),
        updated_at_ns: 11,
    };
    let before = fs::metadata(&temp.0).unwrap().len();
    let mut forged = request.clone();
    forged.source_quote = "forged evidence".into();
    forged.end_byte = forged.source_quote.len();
    assert!(writer.create_relationship(forged).is_err());
    assert_eq!(fs::metadata(&temp.0).unwrap().len(), before);
    let relationship = writer.create_relationship(request.clone()).unwrap();
    let retirement = RelationshipRetirementRequest {
        owner_id: owner,
        relationship_id: relationship.id,
        relationship_revision: relationship.revision,
        source_memory_id: memory.record.id,
        source_memory_revision: memory.record.revision,
        source_body_id: memory.record.body_id,
        source_field: MemoryTextField::Content,
        start_byte: request.start_byte,
        end_byte: request.end_byte,
        source_quote: request.source_quote,
    };
    let mut stale = retirement.clone();
    stale.relationship_revision += 1;
    assert!(writer.retire_relationship(stale).is_err());
    let tombstone = writer.retire_relationship(retirement).unwrap();
    drop(writer);
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let reopened = RelSession::open_path(&temp.0, &admission, [], []).unwrap();
    assert_eq!(
        reopened.relationship_tombstone(relationship.id),
        Some(tombstone)
    );
    assert_eq!(
        reopened
            .relationship_revision(relationship.id, relationship.revision)
            .unwrap()
            .summary,
        "Mira coordinates the Lantern review."
    );
}

#[test]
fn user_correction_and_retirement_survive_reopen_and_history_pruning() {
    use crate::UserMemoryCorrection;
    use crate::records::VerifiedOwnerCopyPlan;
    use reliquary_graph::{RelationChange, RelationKind, RelationOrigin};

    let temp = Temp::new("user-memory-source");
    let output = Temp::new("user-memory-pruned");
    let owner = [171; 16];
    let correction_key = format!("management:{}", "a".repeat(64));
    let retirement_key = format!("management:{}", "b".repeat(64));
    let body = "original sensitive accepted Memory ".repeat(150);
    let (original, replacement, request, terminal) = {
        let mut writer = RelWriter::create_path(&temp.0, ReliquaryScope::Project, owner).unwrap();
        let original = publish_memory(
            &mut writer,
            memory_draft(owner, "original", &body, false, None, 10),
            10,
        );
        let id = original.record.id;
        let result = writer
            .correct_user_memory(
                "operator",
                correction_key.clone(),
                [31; 32],
                UserMemoryCorrection {
                    id,
                    revision: original.record.revision,
                    body: original.record.body_id,
                    title: "Corrected title".into(),
                    content: "Corrected accepted body".into(),
                    created_at_ns: 20,
                },
            )
            .unwrap();
        let replacement = result.replacement;
        let archived = writer.session.common.memories.current(id).unwrap();
        assert_eq!(result.archived_revision, archived.record.revision);
        assert_eq!(archived.payload, original.payload);
        let request = retirement_request(owner, &archived, &replacement);
        let grant = GraphAdmission::for_changes(owner, [id.0, replacement.record.id.0], []);
        let edge = RelationChange {
            source: NodeRef::Memory(id.0),
            target: NodeRef::Memory(replacement.record.id.0),
            kind: RelationKind::Topical,
            active: true,
            origin: RelationOrigin::User,
        };
        writer
            .session
            .common
            .graph
            .put(&mut writer.container, &grant, &[edge])
            .unwrap();
        writer.session.writer_lease_proof = writer.container.writer_lease_proof();
        let bytes = fs::metadata(&temp.0).unwrap().len();
        assert!(
            writer
                .retire_user_memory(
                    "operator",
                    retirement_key.clone(),
                    [32; 32],
                    request.clone(),
                )
                .is_err()
        );
        assert_eq!(fs::metadata(&temp.0).unwrap().len(), bytes);
        writer
            .session
            .common
            .graph
            .put(
                &mut writer.container,
                &grant,
                &[RelationChange {
                    active: false,
                    ..edge
                }],
            )
            .unwrap();
        writer.session.writer_lease_proof = writer.container.writer_lease_proof();
        let (terminal, retired) = writer
            .retire_user_memory(
                "operator",
                retirement_key.clone(),
                [32; 32],
                request.clone(),
            )
            .unwrap();
        assert!(retired);
        (original, replacement, request, terminal)
    };
    let grant = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let plan = VerifiedOwnerCopyPlan::prepare(&temp.0).unwrap();
    super::super::prune_terminal_memory_copy(
        &plan,
        &temp.0,
        &output.0,
        &[original.record.id],
        &grant,
    )
    .unwrap();
    assert!(
        !fs::read(&output.0)
            .unwrap()
            .windows(body.len())
            .any(|part| part == body.as_bytes())
    );
    let mut reopened = RelWriter::open_path_current(&output.0).unwrap();
    let (replayed, retired) = reopened
        .retire_user_memory("operator", retirement_key, [32; 32], request)
        .unwrap();
    assert_eq!(replayed, terminal);
    assert!(!retired);
    assert_eq!(
        reopened
            .session
            .common
            .memories
            .current(replacement.record.id)
            .unwrap(),
        replacement
    );
    let prior = reopened
        .session
        .common
        .memories
        .history(original.record.id)
        .unwrap();
    assert!(
        prior
            .revisions
            .iter()
            .all(|revision| revision.payload == PayloadState::Absent)
    );
}
