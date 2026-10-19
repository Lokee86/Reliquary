use crate::{
    Container, Cva, CvaError, EpisodeId, GraphRelationKind, MemoryDraft, MemoryError,
    MemorySourceRef, Phylactery, PhylacteryError, SimulatedEmbeddingEndpoint, VectorNormalization,
};
use std::fs;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("reliquary-phylactery-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn draft(mutation_id: &str, title: &str, content: &str) -> MemoryDraft {
    MemoryDraft {
        category: "preference".into(),
        memory_type: "user".into(),
        authority_kind: "direct".into(),
        temporal_status: "unknown".into(),
        title: title.into(),
        content: content.into(),
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
        source_time_ns: None,
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

#[test]
fn phylactery_identity_and_unsourced_memory_reopen() {
    let path = test_path("user.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    assert_eq!(
        phy.container.identity().unwrap().file_kind,
        crate::FileKind::Phylactery
    );
    let (memory, created) = phy
        .publish_memory(
            None,
            0,
            draft("user:m1", "Editor", "The user prefers Helix."),
        )
        .unwrap();
    assert!(created);
    phy.sync().unwrap();
    drop(phy);

    let mut reopened = Phylactery::open(&path).unwrap();
    assert_eq!(reopened.memory_stats().memories, 1);
    assert_eq!(reopened.memory(memory.id).unwrap().content, memory.content);
}

#[test]
fn phylactery_profile_persists_and_normalizes() {
    let path = test_path("profile.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    assert_eq!(phy.profile(), crate::PhylacteryProfile::default());

    assert!(phy.set_profile(Some("  Example User  ".into())).unwrap());
    assert_eq!(
        phy.profile(),
        crate::PhylacteryProfile {
            display_name: Some("Example User".into()),
        }
    );
    assert!(!phy.set_profile(Some("Example User".into())).unwrap());
    assert!(phy.set_profile(Some("   ".into())).unwrap());
    assert_eq!(phy.profile().display_name, None);
    assert!(matches!(
        phy.set_profile(Some(
            "x".repeat(crate::MAX_PHYLACTERY_PROFILE_NAME_BYTES + 1)
        )),
        Err(PhylacteryError::Profile(_))
    ));
    assert!(phy.set_profile(Some("Example User".into())).unwrap());

    phy.sync().unwrap();
    drop(phy);
    let reopened = Phylactery::open(&path).unwrap();
    assert_eq!(
        reopened.profile(),
        crate::PhylacteryProfile {
            display_name: Some("Example User".into()),
        }
    );
}

#[test]
fn phylactery_profile_reads_legacy_two_field_record() {
    let path = test_path("legacy-profile.phy");
    let mut phy = Phylactery::create(&path).unwrap();

    let mut payload = b"CVAPHYP1".to_vec();
    payload.push(1);
    payload.extend_from_slice(&(12_u32).to_le_bytes());
    payload.extend_from_slice(b"Example User");
    payload.push(1);
    payload.extend_from_slice(&(13_u32).to_le_bytes());
    payload.extend_from_slice(b"legacy_handle");
    phy.container.append(&payload).unwrap();
    phy.sync().unwrap();
    drop(phy);

    let reopened = Phylactery::open(&path).unwrap();
    assert_eq!(
        reopened.profile(),
        crate::PhylacteryProfile {
            display_name: Some("Example User".into()),
        }
    );
}

#[test]
fn source_reference_survives_metadata_revision_and_reopen() {
    let path = test_path("source-ref-revision.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    let source_ref = MemorySourceRef {
        owner_id: "rel-00000000-0000-0000-0000-000000000001".into(),
        principal_id: None,
        source_episode_id: EpisodeId([9; 32]),
        source_node_id: "u1".into(),
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
    };
    let (first, _) = phy
        .publish_memory_with_source_ref(
            None,
            0,
            draft("user:source-ref", "Editor", "The user prefers Helix."),
            source_ref.clone(),
        )
        .unwrap();

    let mut revised = draft(
        "user:source-ref:knowledge",
        "Editor",
        "The user prefers Helix.",
    );
    revised.lifecycle_state = "knowledge".into();
    revised.updated_at_ns = 20;
    let (second, _) = phy.publish_memory(Some(first.id), 1, revised).unwrap();
    assert_eq!(second.source_ref.as_ref(), Some(&source_ref));
    phy.sync().unwrap();
    drop(phy);

    let mut reopened = Phylactery::open(&path).unwrap();
    assert_eq!(
        reopened.memory(first.id).unwrap().source_ref,
        Some(source_ref)
    );
}

#[test]
fn temporal_status_backfill_preserves_phy_source_reference() {
    let path = test_path("temporal-status.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    let source_ref = MemorySourceRef {
        owner_id: "rel-00000000-0000-0000-0000-000000000001".into(),
        principal_id: None,
        source_episode_id: EpisodeId([8; 32]),
        source_node_id: "u1".into(),
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
    };
    let (first, _) = phy
        .publish_memory_with_source_ref(
            None,
            0,
            draft(
                "user:temporal",
                "Preference",
                "The user prefers concise replies.",
            ),
            source_ref.clone(),
        )
        .unwrap();
    let (updated, created) = phy
        .set_memory_temporal_status(
            first.id,
            first.revision,
            "current",
            "temporal-backfill".into(),
        )
        .unwrap();
    assert!(created);
    assert_eq!(updated.temporal_status, "current");
    assert_eq!(updated.source_ref, Some(source_ref.clone()));
    phy.sync().unwrap();
    drop(phy);

    let mut reopened = Phylactery::open(path).unwrap();
    let memory = reopened.memory(first.id).unwrap();
    assert_eq!(memory.temporal_status, "current");
    assert_eq!(memory.source_ref, Some(source_ref));
}

#[test]
fn phylactery_rejects_rel_local_provenance() {
    let path = test_path("provenance.phy");
    let mut phy = Phylactery::create(path).unwrap();
    let mut sourced = draft("user:sourced", "Preference", "Keep this.");
    sourced.source_node_id = Some("project-node".into());
    assert!(matches!(
        phy.publish_memory(None, 0, sourced),
        Err(MemoryError::InvalidProvenance)
    ));
}

#[test]
fn reliquary_and_phylactery_reject_each_others_file_kind() {
    let phy_path = test_path("wrong.phy");
    drop(Phylactery::create(&phy_path).unwrap());
    assert!(matches!(
        Cva::open(&phy_path),
        Err(CvaError::InvalidContainerIdentity(_))
    ));

    let rel_path = test_path("wrong.prj.rel");
    drop(Cva::create_project(&rel_path).unwrap());
    assert!(matches!(
        Phylactery::open(&rel_path),
        Err(PhylacteryError::InvalidContainerIdentity(_))
    ));

    let legacy_path = test_path("legacy.cva");
    drop(Container::create(&legacy_path).unwrap());
    assert!(matches!(
        Phylactery::open(&legacy_path),
        Err(PhylacteryError::InvalidContainerIdentity(_))
    ));
}

#[test]
fn phylactery_rejects_rel_only_owner_records() {
    let path = test_path("foreign-owner.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    phy.container.append(b"CVAAFMT2foreign").unwrap();
    phy.sync().unwrap();
    drop(phy);

    assert!(matches!(
        Phylactery::open(&path),
        Err(PhylacteryError::InvalidContainerIdentity(_))
    ));
}

#[test]
fn graph_profiles_and_memory_vectors_persist() {
    let path = test_path("state.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    let (a, _) = phy
        .publish_memory(
            None,
            0,
            draft("user:a", "Editor", "The user prefers Helix."),
        )
        .unwrap();
    let (b, _) = phy
        .publish_memory(
            None,
            0,
            draft("user:b", "Shell", "The user prefers Nushell."),
        )
        .unwrap();
    phy.set_memory_relation(a.id, b.id, GraphRelationKind::Topical, true, 0)
        .unwrap();

    let endpoint = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 7);
    let profile = phy.establish_compatibility_profile(&endpoint).unwrap();
    let result = phy
        .build_missing_memory_vectors(profile.id, &endpoint)
        .unwrap();
    assert_eq!(result.embedded, 2);
    phy.sync().unwrap();
    drop(phy);

    let reopened = Phylactery::open(&path).unwrap();
    assert_eq!(reopened.graph_stats().active_relations, 1);
    assert_eq!(reopened.compatibility_profile_stats().profiles, 1);
    assert_eq!(reopened.memory_vector_stats().bindings, 2);
}

#[test]
fn phylactery_rejects_recent_grouped_completion_carriers() {
    for magic in [b"CVAINSC6", b"CVAINSC7", b"CVAINSC8"] {
        let path = test_path(&format!("foreign-{}.phy", String::from_utf8_lossy(magic)));
        let mut phy = Phylactery::create(&path).unwrap();
        let mut payload = vec![0; if magic == b"CVAINSC8" { 85 } else { 84 }];
        payload[..8].copy_from_slice(magic);
        if magic == b"CVAINSC8" {
            payload[8] = 6;
        }
        phy.container.append(&payload).unwrap();
        phy.sync().unwrap();
        drop(phy);
        assert!(matches!(
            Phylactery::open(&path),
            Err(PhylacteryError::InvalidContainerIdentity(_))
        ));
    }
}
