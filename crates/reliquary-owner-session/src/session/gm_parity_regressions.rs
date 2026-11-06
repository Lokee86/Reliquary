//! Deliberately failing GM parity regressions discovered by independent audit.
use super::*;
use sha2::{Digest, Sha256};

/// Dispose of diagnostics even when a deliberately red regression panics.
struct AuditFixtures(Vec<std::path::PathBuf>);
impl Drop for AuditFixtures {
    fn drop(&mut self) {
        for path in &self.0 {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[test]
fn gm_audit_rewrite_must_retain_memory_linked_relationship_and_entity() {
    use reliquary_perception_store::{
        EntityDraft, RelationshipDraft, RelationshipEntityRef, RelationshipMemoryRef,
        RelationshipParticipant,
    };
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".owner-session-test-tmp");
    std::fs::create_dir_all(&base).unwrap();
    let source_path = base.join(format!("gm-audit-linked-source-{}.phy", std::process::id()));
    let output_path = base.join(format!("gm-audit-linked-output-{}.phy", std::process::id()));
    let _cleanup = AuditFixtures(vec![source_path.clone(), output_path.clone()]);
    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_file(&output_path);
    let owner = [0xc1; 16];
    drop(PhyWriter::create_path(&source_path, owner).unwrap());
    let (memory_id, entity_id, relationship_id) = {
        let mut container = Container::open_writer_as(&source_path, FileKind::Phylactery).unwrap();
        let mut memories = MemoryStore::open(&container).unwrap();
        let memory = memories
            .publish(
                &mut container,
                None,
                0,
                MemoryDraft {
                    title: "Audit evidence".into(),
                    content: "Retained evidence for a person".into(),
                    category: "fact".into(),
                    memory_type: "user".into(),
                    authority_kind: "direct".into(),
                    temporal_status: "current".into(),
                    scope: "user".into(),
                    lifecycle_state: "active".into(),
                    archived: false,
                    parent_id: None,
                    superseded_by: None,
                    source_node_id: None,
                    content_source_conversation_id: None,
                    content_source_node_id: None,
                    grounding_source_conversation_id: None,
                    grounding_source_node_id: None,
                    source_episode_id: None,
                    source_ref: None,
                    source_time_ns: None,
                    temporal_inference: None,
                    mutation_id: "audit-memory".into(),
                    created_at_ns: 1,
                    updated_at_ns: 1,
                },
            )
            .unwrap()
            .revision
            .record
            .id;
        let mut perception = PerceptionStore::open(&container).unwrap();
        let entity = perception
            .publish_entity(
                &mut container,
                EntityDraft {
                    canonical_name: "Retained person".into(),
                    aliases: vec![],
                    kind: "person".into(),
                    summary: "retained biography".into(),
                    mutation_id: "audit-entity".into(),
                    created_at_ns: 1,
                    updated_at_ns: 1,
                },
            )
            .unwrap();
        let source_owner = "c1c1c1c1-c1c1-c1c1-c1c1-c1c1c1c1c1c1".to_string();
        let relationship = perception
            .publish_relationship(
                &mut container,
                RelationshipDraft {
                    kind: "association".into(),
                    participants: vec![RelationshipParticipant {
                        entity: RelationshipEntityRef {
                            owner_id: source_owner.clone(),
                            entity_id: entity.id,
                        },
                        role: None,
                    }],
                    evidence: vec![RelationshipMemoryRef {
                        owner_id: source_owner,
                        memory_id: memory.0,
                    }],
                    summary: "retained relationship".into(),
                    mutation_id: "audit-relationship".into(),
                    created_at_ns: 1,
                    updated_at_ns: 1,
                },
            )
            .unwrap();
        (memory, entity.id, relationship.id)
    };
    let source_digest: [u8; 32] = Sha256::digest(std::fs::read(&source_path).unwrap()).into();
    let request = FreshHistoryRequest {
        source_owner_id: owner,
        source_digest,
        plan_digest: [0xb2; 32],
        identity: FreshHistoryIdentity::NewOwner([0xc2; 16]),
        retained_memory_ids: vec![memory_id],
        retain_all_revisions: true,
        retain_revisions_since_memory_version: None,
        retain_derived_state: true,
        archive: FreshHistoryArchive::Omit,
        freshness: FreshHistoryFreshness::Reset,
        profile_ids: vec![],
    };
    let admission = GraphAdmission::for_replay(owner, [memory_id.0], [], [], [], []);
    rewrite_memory_history_copy(&source_path, &output_path, &request, &admission).unwrap();
    let output = Container::open_read_only(&output_path).unwrap();
    let after = PerceptionStore::open(&output).unwrap();
    assert!(
        after.relationship(relationship_id).is_some(),
        "M01 silently dropped a Relationship supported by a retained Memory"
    );
    assert!(
        after.entity(entity_id).is_some(),
        "M01 silently dropped the Relationship's retained local Entity"
    );
    let _ = std::fs::remove_file(output_path);
    let _ = std::fs::remove_file(source_path);
}

#[test]
fn gm_audit_historical_proof_distinguishes_retained_intermediate_entity_revisions() {
    use reliquary_perception_store::EntityDraft;
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".owner-session-test-tmp");
    std::fs::create_dir_all(&dir).unwrap();
    let left = dir.join(format!(
        "gm-audit-revisions-left-{}.phy",
        std::process::id()
    ));
    let right = dir.join(format!(
        "gm-audit-revisions-right-{}.phy",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&left);
    let _ = std::fs::remove_file(&right);
    let _cleanup = AuditFixtures(vec![left.clone(), right.clone()]);
    let owner = [0xd1; 16];
    for (path, intermediate) in [(&left, "intermediate-A"), (&right, "intermediate-B")] {
        drop(PhyWriter::create_path(path, owner).unwrap());
        let mut container = Container::open_writer_as(path, FileKind::Phylactery).unwrap();
        let mut perception = PerceptionStore::open(&container).unwrap();
        let create = |mutation: &str, summary: &str, updated: i64| EntityDraft {
            canonical_name: "Same entity".into(),
            aliases: vec![],
            kind: "person".into(),
            summary: summary.into(),
            mutation_id: mutation.into(),
            created_at_ns: 1,
            updated_at_ns: updated,
        };
        let e = perception
            .publish_entity(&mut container, create("same-create", "initial", 1))
            .unwrap();
        perception
            .update_entity(&mut container, e.id, 1, create("same-mid", intermediate, 2))
            .unwrap();
        perception
            .update_entity(
                &mut container,
                e.id,
                2,
                create("same-final", "same final", 3),
            )
            .unwrap();
    }
    assert_ne!(
        std::fs::read(&left).unwrap(),
        std::fs::read(&right).unwrap()
    );
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let a = PhySession::open_path(&left, &admission, []).unwrap();
    let b = PhySession::open_path(&right, &admission, []).unwrap();
    let cut = a.current_historical_cut();
    assert_eq!(cut, b.current_historical_cut());
    let proof_a = a.historical_proof(owner, cut).unwrap();
    let proof_b = b.historical_proof(owner, cut).unwrap();
    assert_ne!(
        proof_a.digest, proof_b.digest,
        "M04 proof ignores retained intermediate Entity revision history"
    );
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
}
#[test]
fn gm_audit_reclamation_handles_native_external_index_discard() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".owner-session-test-tmp");
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join(format!(
        "gm-audit-external-index-source-{}.rel",
        std::process::id()
    ));
    let output = dir.join(format!(
        "gm-audit-external-index-output-{}.rel",
        std::process::id()
    ));
    let _cleanup = AuditFixtures(vec![source.clone(), output.clone()]);
    let _ = std::fs::remove_file(&source);
    let _ = std::fs::remove_file(&output);
    let owner = [0xe2; 16];
    drop(
        RelWriter::create_path(&source, reliquary_container::ReliquaryScope::Project, owner)
            .unwrap(),
    );
    let mut container = Container::open_writer_as(&source, FileKind::Reliquary).unwrap();
    let content = vec![0x5a; 512];
    let mut orphan = b"RELFILE1".to_vec();
    orphan.extend_from_slice(&Sha256::digest(&content));
    orphan.extend_from_slice(&(content.len() as u32).to_le_bytes());
    orphan.extend_from_slice(&content);
    container.append(&orphan).unwrap();
    container
        .append(&crate::records::external_index::compact_index::discard())
        .unwrap();
    container.sync().unwrap();
    drop(container);
    let before = std::fs::read(&source).unwrap();
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    RelSession::open_path(&source, &admission, [], []).unwrap();
    let plan = crate::VerifiedOwnerCopyPlan::prepare(&source).unwrap();
    let report = super::reclaim_storage_copy(&plan, &source, &output, &admission).unwrap();
    assert_eq!(report.removed_frames, 1);
    assert_eq!(std::fs::read(&source).unwrap(), before);
    RelSession::open_path(&output, &admission, [], []).unwrap();
}

#[test]
fn gm_audit_reclamation_removes_unactivated_archive_vector_mapping_and_backing() {
    use reliquary_container::ReliquaryScope;
    use reliquary_vector_store::{AcceptedArchiveMapping, ArchiveFragmentId};
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".owner-session-test-tmp");
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join(format!(
        "gm-audit-orphan-archive-source-{}.rel",
        std::process::id()
    ));
    let output = dir.join(format!(
        "gm-audit-orphan-archive-output-{}.rel",
        std::process::id()
    ));
    let _cleanup = AuditFixtures(vec![source.clone(), output.clone()]);
    let _ = std::fs::remove_file(&source);
    let _ = std::fs::remove_file(&output);
    let owner = [0xe1; 16];
    drop(RelWriter::create_path(&source, ReliquaryScope::Project, owner).unwrap());
    {
        let mut container = Container::open_writer_as(&source, FileKind::Reliquary).unwrap();
        let mut packed = PackedVectorStore::open(&container).unwrap();
        let packed_id = packed
            .put_f32_rows(&mut container, &[vec![0.5; 128]])
            .unwrap();
        let mut archive_vectors = ArchiveVectorStore::open(&container, &packed).unwrap();
        archive_vectors
            .put(
                &mut container,
                &packed,
                AcceptedArchiveMapping {
                    packed_vector_id: packed_id,
                    packed_rows: 1,
                    fragment_ids: vec![ArchiveFragmentId([0xf2; 32])],
                    max_fragment_archive_version: 0,
                },
            )
            .unwrap();
        container.sync().unwrap();
    }
    let original = std::fs::read(&source).unwrap();
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    RelSession::open_path(&source, &admission, [], []).unwrap();
    let plan = crate::VerifiedOwnerCopyPlan::prepare(&source).unwrap();
    let report = super::reclaim_storage_copy(&plan, &source, &output, &admission).unwrap();
    assert_eq!(std::fs::read(&source).unwrap(), original);
    assert_eq!(
        report.removed_frames, 2,
        "M02 failed to reclaim a never-activated Archive-vector mapping and its exclusive packed backing"
    );
}
