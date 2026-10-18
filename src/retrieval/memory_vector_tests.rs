use crate::archive_history_codec::encode_archive_format;
use crate::compatibility_profile_codec::encode_format as encode_profile_format;
use crate::insomnia::codec::encode_format as encode_insomnia_format;
use crate::memory_codec::encode_format as encode_memory_format;
use crate::packed_vector_codec::encode_format as encode_packed_format;
use crate::{
    Cva, CvaError, MemoryDraft, MemoryError, MemoryVectorError, SimulatedEmbeddingEndpoint,
    VectorNormalization,
};
use std::fs;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::current_dir()
        .unwrap()
        .join(".verification-tmp")
        .join(format!("reliquary-memory-vectors-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn draft(mutation_id: &str, lifecycle: &str, title: &str, content: &str) -> MemoryDraft {
    MemoryDraft {
        category: "preference".into(),
        memory_type: "project".into(),
        authority_kind: "unknown".into(),
        temporal_status: "unknown".into(),
        title: title.into(),
        content: content.into(),
        scope: "private".into(),
        lifecycle_state: lifecycle.into(),
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
fn memory_body_is_embedded_once_per_profile_across_metadata_revisions() {
    let path = test_path("reuse.cva");
    let mut cva = Cva::create(&path).unwrap();
    let (memory, _) = cva
        .publish_memory(
            None,
            0,
            draft(
                "m1",
                "extracted",
                "Preferred editor",
                "The user prefers Helix.",
            ),
        )
        .unwrap();
    let body_id = cva.memory_body_id(memory.id).unwrap();
    let endpoint = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 7);
    let profile = cva.establish_compatibility_profile(&endpoint).unwrap();
    let global_before_vectors = cva.container.latest_version();
    let memory_version_before_vectors = cva.memory_version();

    let first = cva
        .build_missing_memory_vectors(profile.id, &endpoint)
        .unwrap();
    assert_eq!(cva.container.latest_version(), global_before_vectors);
    assert_eq!(cva.memory_version(), memory_version_before_vectors);
    assert_eq!(first.embedded, 1);
    assert_eq!(first.already_present, 0);
    let first_set_id = first.created_set.unwrap();
    assert!(cva.memory_vector_location(profile.id, body_id).is_some());
    let first_set = cva.memory_vectors(first_set_id).unwrap();
    assert_eq!(
        cva.put_memory_vectors(
            profile.id,
            first_set.packed_vector_id,
            first_set.memory_body_ids.clone(),
        )
        .unwrap(),
        first_set_id
    );
    let packed_after_first = cva.packed_vector_stats();

    let mut revised = draft(
        "m1-metadata",
        "reinforced",
        "Preferred editor",
        "The user prefers Helix.",
    );
    revised.updated_at_ns = 20;
    revised.category = "decision".into();
    cva.publish_memory(Some(memory.id), 1, revised).unwrap();
    assert_eq!(cva.memory_body_id(memory.id).unwrap(), body_id);

    let second = cva
        .build_missing_memory_vectors(profile.id, &endpoint)
        .unwrap();
    assert_eq!(second.embedded, 0);
    assert_eq!(second.already_present, 1);
    assert_eq!(second.created_set, None);
    assert_eq!(cva.packed_vector_stats(), packed_after_first);
}

#[test]
fn semantic_memory_content_cannot_mutate_in_place() {
    let path = test_path("immutable-body.cva");
    let mut cva = Cva::create(&path).unwrap();
    let (memory, _) = cva
        .publish_memory(None, 0, draft("m1", "extracted", "Rule", "Keep A."))
        .unwrap();
    let changed = draft("m1-changed", "reinforced", "Rule", "Keep B.");
    assert!(matches!(
        cva.publish_memory(Some(memory.id), 1, changed),
        Err(MemoryError::SemanticMutation)
    ));
}

#[test]
fn a_new_profile_gets_a_distinct_embedding_for_the_same_memory_body() {
    let path = test_path("profiles.cva");
    let mut cva = Cva::create(&path).unwrap();
    let (memory, _) = cva
        .publish_memory(None, 0, draft("m1", "extracted", "Rule", "Keep A."))
        .unwrap();
    let body_id = cva.memory_body_id(memory.id).unwrap();
    let endpoint_a = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 7);
    let endpoint_b = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 9001);
    let profile_a = cva.establish_compatibility_profile(&endpoint_a).unwrap();
    let profile_b = cva.establish_compatibility_profile(&endpoint_b).unwrap();
    assert_ne!(profile_a.id, profile_b.id);

    cva.build_missing_memory_vectors(profile_a.id, &endpoint_a)
        .unwrap();
    cva.build_missing_memory_vectors(profile_b.id, &endpoint_b)
        .unwrap();
    assert!(cva.memory_vector_location(profile_a.id, body_id).is_some());
    assert!(cva.memory_vector_location(profile_b.id, body_id).is_some());
    assert_eq!(cva.memory_vector_stats().bindings, 2);
}

#[test]
fn memory_vector_store_requires_its_format_marker() {
    let path = test_path("missing-format.cva");
    let mut container = crate::Container::create(&path).unwrap();
    container.append(&encode_archive_format()).unwrap();
    container.append(&encode_memory_format()).unwrap();
    container.append(&encode_insomnia_format()).unwrap();
    container.append(&encode_packed_format()).unwrap();
    container.append(&encode_profile_format()).unwrap();
    container.sync().unwrap();
    drop(container);
    assert!(matches!(
        Cva::open(path),
        Err(CvaError::MemoryVectors(MemoryVectorError::MissingFormat))
    ));
}

#[test]
fn memory_vector_bindings_reopen_with_the_cva() {
    let path = test_path("reopen.cva");
    let mut cva = Cva::create(&path).unwrap();
    let (memory, _) = cva
        .publish_memory(None, 0, draft("m1", "extracted", "Rule", "Keep A."))
        .unwrap();
    let body_id = cva.memory_body_id(memory.id).unwrap();
    let endpoint = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 7);
    let profile = cva.establish_compatibility_profile(&endpoint).unwrap();
    let result = cva
        .build_missing_memory_vectors(profile.id, &endpoint)
        .unwrap();
    let set_id = result.created_set.unwrap();
    cva.sync().unwrap();
    drop(cva);

    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.memory_vector_stats().bindings, 1);
    let location = reopened
        .memory_vector_location(profile.id, body_id)
        .unwrap();
    assert_eq!(location.set_id, set_id);
    let set = reopened.memory_vectors(set_id).unwrap();
    assert_eq!(set.memory_body_ids, vec![body_id]);
}

#[test]
fn partial_memory_vector_coverage_stays_inactive_after_reopen() {
    let path = test_path("partial-coverage.cva");
    let mut cva = Cva::create(&path).unwrap();
    let (first, _) = cva
        .publish_memory(None, 0, draft("m1", "extracted", "Rule one", "Keep A."))
        .unwrap();
    let (second, _) = cva
        .publish_memory(None, 0, draft("m2", "extracted", "Rule two", "Keep B."))
        .unwrap();
    let endpoint = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 7);
    let profile = cva.establish_compatibility_profile(&endpoint).unwrap();
    let first_body = cva.memory_body_id(first.id).unwrap();
    let second_body = cva.memory_body_id(second.id).unwrap();
    let packed = crate::PackedVectors::from_bytes(
        crate::VectorSchema::new(8, crate::ScalarType::F32).unwrap(),
        vec![0; 8 * 4],
    )
    .unwrap();
    let packed_id = cva.put_packed_vectors(packed).unwrap();
    cva.put_memory_vectors(profile.id, packed_id, vec![first_body])
        .unwrap();
    assert!(
        cva.active_memory_compatibility_profile(profile.id)
            .is_none()
    );
    cva.sync().unwrap();
    drop(cva);

    let reopened = Cva::open(&path).unwrap();
    assert!(
        reopened
            .active_memory_compatibility_profile(profile.id)
            .is_none()
    );
    assert!(
        reopened
            .memory_vector_location(profile.id, first_body)
            .is_some()
    );
    assert!(
        reopened
            .memory_vector_location(profile.id, second_body)
            .is_none()
    );
}

#[test]
fn packed_vector_prune_requires_every_mapping_profile_retired_and_retirement_keeps_bytes() {
    let path = test_path("packed-retirement-eligibility.cva");
    let mut cva = Cva::create(&path).unwrap();
    let (memory, _) = cva
        .publish_memory(None, 0, draft("prune-m1", "extracted", "Rule", "Keep A."))
        .unwrap();
    let body_id = cva.memory_body_id(memory.id).unwrap();
    let endpoint_a = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 7);
    let endpoint_b = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 9001);
    let profile_a = cva.establish_compatibility_profile(&endpoint_a).unwrap();
    let profile_b = cva.establish_compatibility_profile(&endpoint_b).unwrap();
    let packed = crate::PackedVectors::from_bytes(
        crate::VectorSchema::new(8, crate::ScalarType::F32).unwrap(),
        vec![0; 8 * 4],
    )
    .unwrap();
    let packed_id = cva.put_packed_vectors(packed.clone()).unwrap();
    cva.put_memory_vectors(profile_a.id, packed_id, vec![body_id])
        .unwrap();
    cva.put_memory_vectors(profile_b.id, packed_id, vec![body_id])
        .unwrap();

    assert!(
        !cva.prunable_packed_vector_profiles()
            .iter()
            .any(|(id, _)| *id == packed_id)
    );
    cva.retire_compatibility_profile(profile_a.id).unwrap();
    assert!(
        !cva.prunable_packed_vector_profiles()
            .iter()
            .any(|(id, _)| *id == packed_id)
    );
    assert_eq!(cva.packed_vectors(packed_id).unwrap(), packed);
    cva.retire_compatibility_profile(profile_b.id).unwrap();

    let candidates = cva.prunable_packed_vector_profiles();
    let anchor = [profile_a.id, profile_b.id]
        .into_iter()
        .min_by_key(|id| id.0)
        .unwrap();
    assert!(candidates.contains(&(packed_id, anchor)));
    let descriptor = cva
        .packed_vector_retention_descriptor(anchor, packed_id, cva.latest_global_version())
        .unwrap();
    assert_eq!(descriptor.target.identity, anchor.0.to_vec());
    assert_eq!(
        descriptor.payloads,
        vec![crate::history_prune_manifest::PrunedPayload::PackedVectors(
            packed_id.0
        )]
    );
    assert_eq!(cva.packed_vectors(packed_id).unwrap(), packed);

    cva.sync().unwrap();
    drop(cva);
    let mut reopened = Cva::open(&path).unwrap();
    assert!(reopened.compatibility_profile_is_retired(profile_a.id));
    assert!(reopened.compatibility_profile_is_retired(profile_b.id));
    assert_eq!(reopened.packed_vectors(packed_id).unwrap(), packed);
    assert!(
        reopened
            .memory_vector_location(profile_a.id, body_id)
            .is_some()
    );
    assert!(
        reopened
            .memory_vector_location(profile_b.id, body_id)
            .is_some()
    );
}

#[test]
fn transformed_packed_prune_keeps_archive_and_memory_vector_structure() {
    let path = test_path("packed-transform-source.cva");
    let output = test_path("packed-transform-output.cva");
    let mut cva = Cva::create(&path).unwrap();
    let (memory, _) = cva
        .publish_memory(None, 0, draft("prune-e2e", "extracted", "Rule", "Keep A."))
        .unwrap();
    let body_id = cva.memory_body_id(memory.id).unwrap();
    cva.append_node(
        "n1".into(),
        "c1".into(),
        None,
        "user".into(),
        1,
        "source turn",
    )
    .unwrap();
    let fragments = cva
        .materialize_path_fragments("c1", "n1", crate::FragmentConfig::default(), true)
        .unwrap()
        .into_iter()
        .map(|fragment| fragment.id)
        .collect::<Vec<_>>();
    assert_eq!(fragments.len(), 1);

    let endpoint = SimulatedEmbeddingEndpoint::new(512, VectorNormalization::L2, 7);
    let profile = cva.establish_compatibility_profile(&endpoint).unwrap();
    let matrix = crate::PackedVectors::from_bytes(
        crate::VectorSchema::new(512, crate::ScalarType::F32).unwrap(),
        vec![1; 512 * 4],
    )
    .unwrap();
    let packed_id = cva.put_packed_vectors(matrix).unwrap();
    let memory_set = cva
        .put_memory_vectors(profile.id, packed_id, vec![body_id])
        .unwrap();
    let archive_set = cva.put_archive_vectors(packed_id, fragments).unwrap();
    let generation = cva
        .publish_vector_generation(profile.id, archive_set, cva.archive_version())
        .unwrap();
    cva.retire_compatibility_profile(profile.id).unwrap();

    let report = cva
        .prune_tombstoned_history(
            &output,
            crate::HistoryPruneRequest {
                targets: vec![
                    crate::owner_lifecycle::LifecycleTarget::CompatibilityProfile(profile.id),
                ],
            },
        )
        .unwrap();
    assert_eq!(report.manifest_digest.len(), 32);
    assert!(report.output_file_bytes < report.source_file_bytes);
    drop(cva);

    let mut reopened = Cva::open(&output).unwrap();
    assert!(matches!(
        reopened.packed_vectors(packed_id),
        Err(crate::PackedVectorError::PayloadUnavailable(
            crate::owner_lifecycle::PayloadUnavailable::Pruned
        ))
    ));
    assert_eq!(
        reopened.memory_vectors(memory_set).unwrap().memory_body_ids,
        vec![body_id]
    );
    assert_eq!(
        reopened
            .archive_vectors(archive_set)
            .unwrap()
            .fragment_ids
            .len(),
        1
    );
    assert_eq!(
        reopened.historical_vector_generation(profile.id),
        Some(generation)
    );
    assert!(reopened.current_vector_generation(profile.id).is_none());
    assert_eq!(
        reopened
            .memory_vector_location(profile.id, body_id)
            .unwrap()
            .set_id,
        memory_set
    );
}
