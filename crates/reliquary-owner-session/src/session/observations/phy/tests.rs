use super::*;
use crate::{DreamMemoryCut, VerifiedOwnerCopyPlan};
use reliquary_memory_core::MemorySourceRef;
use reliquary_perception_store::{EntityResolutionReason, MemoryEntityResolutionStatus};

const OWNER: [u8; 16] = [7; 16];

fn fixture_directory() -> tempfile::TempDir {
    let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    std::fs::create_dir_all(&parent).unwrap();
    tempfile::tempdir_in(parent).unwrap()
}

fn local_memory(
    writer: &mut PhyWriter,
    mutation: &str,
    content: &str,
    direct: bool,
) -> MemoryRevision {
    writer
        .common
        .memories
        .publish(
            &mut writer.container,
            None,
            0,
            MemoryDraft {
                title: "Private notes".into(),
                content: content.into(),
                category: "fact".into(),
                memory_type: "user".into(),
                authority_kind: "direct".into(),
                temporal_status: "current".into(),
                scope: "user".into(),
                lifecycle_state: "knowledge".into(),
                archived: false,
                parent_id: None,
                superseded_by: None,
                source_node_id: None,
                content_source_conversation_id: None,
                content_source_node_id: None,
                grounding_source_conversation_id: None,
                grounding_source_node_id: None,
                source_episode_id: None,
                source_ref: direct.then(|| MemorySourceRef {
                    owner_id: "07".repeat(16),
                    principal_id: None,
                    source_episode_id: [1; 32],
                    source_node_id: mutation.into(),
                    content_source_conversation_id: None,
                    content_source_node_id: None,
                    grounding_source_conversation_id: None,
                    grounding_source_node_id: None,
                }),
                source_time_ns: Some(1),
                temporal_inference: None,
                mutation_id: mutation.into(),
                created_at_ns: 1,
                updated_at_ns: 1,
            },
        )
        .unwrap()
        .revision
}

fn accepted_memory(
    source: &mut PhyWriter,
    writer: &mut PhyWriter,
    mutation: &str,
    content: &str,
    direct: bool,
) -> ObservationCitation {
    let accepted = local_memory(source, mutation, content, direct);
    assert!(
        source
            .observation_citation_text(&ObservationCitation {
                owner_id: source.owner_id,
                history_generation: source.history_generation,
                source: ObservationSource::Memory(accepted.record.id.0),
                revision: accepted.record.revision,
                body_id: accepted.record.body_id.0,
                field: MemoryTextField::Content,
                start_byte: 0,
                end_byte: content.len() as u32,
            })
            .is_err(),
        "a local Memory needs a retained-import or Dream settlement"
    );
    writer
        .import_accepted_memory_revision(writer.container.latest_version() + 1, 1, &accepted)
        .unwrap();
    ObservationCitation {
        owner_id: writer.owner_id,
        history_generation: writer.history_generation,
        source: ObservationSource::Memory(accepted.record.id.0),
        revision: accepted.record.revision,
        body_id: accepted.record.body_id.0,
        field: MemoryTextField::Content,
        start_byte: 0,
        end_byte: content.len() as u32,
    }
}

fn memory_id(citation: ObservationCitation) -> MemoryId {
    let ObservationSource::Memory(id) = citation.source else {
        panic!("expected Memory")
    };
    MemoryId(id)
}

fn draft(proposition: &str, citations: Vec<ObservationCitation>) -> ObservationDraft {
    let receptors = [
        ObservationReceptorKind::Support,
        ObservationReceptorKind::Challenge,
        ObservationReceptorKind::Qualification,
        ObservationReceptorKind::Change,
    ]
    .into_iter()
    .map(|kind| ObservationReceptor {
        kind,
        description: format!("Evidence about the claim: {kind:?}"),
        embedding_profile: [5; 32],
        vector: vec![0.0, 1.0],
    })
    .collect();
    ObservationDraft {
        proposition: proposition.into(),
        profile: ObservationProfile::Ordinary,
        status: ObservationStatus::Observed,
        time_scope: ObservationTimeScope::Current,
        validity: None,
        citations,
        reviewed: Vec::new(),
        receptors,
    }
}

fn request(
    writer: &PhyWriter,
    key: u8,
    operations: Vec<ObservationOperation>,
) -> PhyObservationBatch {
    PhyObservationBatch {
        cut: writer.phy_observation_cut(),
        batch: ObservationBatch {
            owner_id: writer.owner_id,
            history_generation: writer.history_generation,
            input_fingerprint: [key; 32],
            policy: ObservationPolicy {
                version: 1,
                general_route: [1; 32],
                decision_route: [2; 32],
            },
            operations,
            progress: None,
        },
    }
}

fn create(writer: &mut PhyWriter, key: u8, draft: ObservationDraft) -> ObservationId {
    let input = request(writer, key, vec![ObservationOperation::Create { draft }]);
    writer.publish_observation_batch(input).unwrap().revisions[0].0
}

fn participant(
    writer: &mut PhyWriter,
    citation: ObservationCitation,
    name: &str,
    start: u32,
) -> ObservationParticipant {
    let key = MemoryEntityMentionKey {
        memory_id: memory_id(citation).0,
        field: citation.field,
        start_byte: start,
        end_byte: start + name.len() as u32,
    };
    let resolved = writer
        .commit_entity_mention(EntityMentionCommit {
            owner_id: OWNER,
            memory_revision: citation.revision,
            body_id: MemoryBodyId(citation.body_id),
            key,
            surface: name.into(),
            expected_revision: 0,
            updated_at_ns: 1,
            status: MemoryEntityResolutionStatus::Unresolved {
                reason: EntityResolutionReason::FirstSeenIdentity,
            },
            target_revisions: Vec::new(),
            new_entity: Some(("person".into(), format!("Person named {name}"))),
        })
        .unwrap();
    let MemoryEntityResolutionStatus::Resolved { entity_id, .. } = resolved.status else {
        panic!("participant was not resolved");
    };
    ObservationParticipant {
        entity_id,
        revision: 1,
        role: None,
        mention: ObservationMention {
            key,
            revision: resolved.revision,
            retained_ordinal: None,
        },
    }
}

fn profile() -> reliquary_vector_store::CompatibilityProfile {
    use reliquary_vector_store::{
        CompatibilityProbeReference, CompatibilityProfile, EmbeddingMode, VectorNormalization,
    };
    let mut profile = CompatibilityProfile {
        id: CompatibilityProfileId([0; 32]),
        dimensions: 2,
        normalization: VectorNormalization::L2,
        probe_suite_version: 1,
        compatibility_policy_version: 3,
        references: [
            EmbeddingMode::Query,
            EmbeddingMode::Query,
            EmbeddingMode::Document,
            EmbeddingMode::Document,
        ]
        .into_iter()
        .map(|mode| CompatibilityProbeReference {
            mode,
            vector: vec![1.0, 0.0],
        })
        .collect(),
    };
    profile.id = reliquary_vector_store::profile::compatibility_profile_id(&profile);
    profile
}

#[test]
fn phy_observations_preserve_local_claims_pending_work_and_retained_proof() {
    let temp = fixture_directory();
    let path = temp.path().join("owner.phy");
    let mut writer = PhyWriter::create_path(&path, OWNER).unwrap();
    let mut source = PhyWriter::create_path(temp.path().join("accepted.phy"), [8; 16]).unwrap();
    let relation_text = "Áda and Bob are family.";
    let grounding = accepted_memory(&mut source, &mut writer, "relational", relation_text, true);
    let alternate = accepted_memory(
        &mut source,
        &mut writer,
        "ordinary",
        "Áda lives nearby.",
        false,
    );
    let participants = vec![
        participant(&mut writer, grounding, "Áda", 0),
        participant(&mut writer, grounding, "Bob", 9),
    ];
    let manual = writer
        .create_relationship(CreateRelationshipRequest {
            owner_id: OWNER,
            kind: "family".into(),
            participants: participants
                .iter()
                .map(|p| RelationshipParticipantEvidence {
                    entity_id: p.entity_id,
                    entity_revision: p.revision,
                    role: None,
                })
                .collect(),
            source_memory_id: memory_id(grounding),
            source_memory_revision: 1,
            source_body_id: MemoryBodyId(grounding.body_id),
            source_field: MemoryTextField::Content,
            start_byte: 0,
            end_byte: relation_text.len(),
            source_quote: relation_text.into(),
            mutation_id: "manual-family".into(),
            updated_at_ns: 1,
        })
        .unwrap();

    drop(writer);
    let old = PhySession::open_path_current(&path).unwrap();
    assert_eq!(old.relationship_revision(manual.id, 1), Some(&manual));
    assert_eq!(old.observation_publication_version(), 0);
    let early = old.current_historical_cut();
    let early_proof = old.historical_proof(OWNER, early).unwrap();
    drop(old);

    let mut writer = PhyWriter::open_path_current(&path).unwrap();
    let invalid = draft(
        "An invalid citation",
        vec![ObservationCitation {
            start_byte: 1,
            ..grounding
        }],
    );
    let foreign = draft(
        "A foreign citation",
        vec![ObservationCitation {
            owner_id: [8; 16],
            ..grounding
        }],
    );
    for draft in [invalid, foreign] {
        let bad = request(&writer, 6, vec![ObservationOperation::Create { draft }]);
        assert!(writer.publish_observation_batch(bad).is_err());
    }
    let first = create(
        &mut writer,
        1,
        draft("Áda lives close by.", vec![alternate]),
    );
    let mut relational = draft("Áda and Bob are related.", vec![grounding]);
    relational.profile = ObservationProfile::Relational(participants.clone());
    let mut foreign_participant = relational.clone();
    let ObservationProfile::Relational(participants) = &mut foreign_participant.profile else {
        unreachable!()
    };
    participants[0].entity_id = EntityId([99; 32]);
    let bad = request(
        &writer,
        9,
        vec![ObservationOperation::Create {
            draft: foreign_participant,
        }],
    );
    assert!(writer.publish_observation_batch(bad).is_err());
    let relation = create(&mut writer, 2, relational);
    let relation_cite = writer
        .observation(relation)
        .unwrap()
        .revision
        .full_citation();
    let derived = create(
        &mut writer,
        3,
        draft("Family members live in the area.", vec![relation_cite]),
    );
    assert_eq!(
        writer.observation(derived).unwrap().standing,
        ObservationStanding::Current
    );
    assert_eq!(
        writer.observation_citation_text(&alternate).unwrap(),
        "Áda lives nearby."
    );

    let profile = profile();
    writer
        .publish_compatibility_profile(profile.clone())
        .unwrap();
    let vector = ObservationClaimVector {
        citation: relation_cite,
        vector: vec![1.0, 0.0],
    };
    writer
        .publish_observation_claim_vectors(profile.id, vec![vector.clone()])
        .unwrap();
    writer
        .publish_observation_claim_vectors(profile.id, vec![vector])
        .unwrap();
    assert_eq!(
        writer
            .observation_claim_vector(profile.id, writer.observation(relation).unwrap().revision)
            .unwrap(),
        Some(vec![1.0, 0.0])
    );

    let mut pending = request(
        &writer,
        4,
        vec![
            ObservationOperation::Comparison {
                comparison: ObservationComparison {
                    id: relation,
                    target_revision: 1,
                    citation: alternate,
                    outcome: ObservationComparisonOutcome::MaterialQualification,
                },
            },
            ObservationOperation::Pending {
                id: relation,
                expected_revision: 1,
                reason: ObservationPendingReason::NewEvidence,
                evidence: vec![alternate],
            },
        ],
    );
    let bytes = b"private audit progress".to_vec();
    pending.batch.progress = Some(ObservationAuditProgress {
        previous: None,
        fingerprint: Sha256::digest(&bytes).into(),
        bytes,
    });
    let pending_receipt = writer.publish_observation_batch(pending.clone()).unwrap();
    assert!(pending_receipt.awards.is_empty());
    assert_eq!(
        writer.observation(derived).unwrap().standing,
        ObservationStanding::Pending(ObservationPendingReason::UpstreamReview)
    );
    let before = pending.cut;
    drop(writer);

    let mut writer = PhyWriter::open_path_current(&path).unwrap();
    assert_eq!(
        writer.publish_observation_batch(pending.clone()).unwrap(),
        pending_receipt
    );
    assert_eq!(
        writer.observation_progress(),
        pending.batch.progress.as_ref()
    );
    assert_eq!(
        writer.observation_pending_after(None, 4)[0].triggering_evidence,
        [alternate]
    );
    assert!(
        writer
            .observation_claim_vector(profile.id, writer.observation(relation).unwrap().revision)
            .unwrap()
            .is_none()
    );
    let mut wrong_cut = pending;
    wrong_cut.cut.observation = writer.observation_publication_version();
    assert!(writer.publish_observation_batch(wrong_cut).is_err());
    let reaffirm = request(
        &writer,
        5,
        vec![ObservationOperation::Reaffirm {
            id: relation,
            expected_revision: 1,
            reviewed: vec![alternate],
        }],
    );
    writer.publish_observation_batch(reaffirm).unwrap();
    assert_eq!(
        writer.observation(derived).unwrap().standing,
        ObservationStanding::Current
    );
    assert!(writer.phy_observation_cut().observation > before.observation);

    writer
        .common
        .memories
        .tombstone(&mut writer.container, memory_id(alternate), 1)
        .unwrap();
    writer.container.sync().unwrap();
    writer.writer_lease_proof = writer.container.writer_lease_proof();
    assert_eq!(
        writer.observation(first).unwrap().standing,
        ObservationStanding::Pending(ObservationPendingReason::ChangedSupport)
    );
    assert!(!writer.observation_revision_source_eligible(first, 1));
    assert!(
        writer
            .historical_observation_citation_text(&alternate)
            .is_err()
    );
    let mut corrected = draft(
        "A family member is named in private notes.",
        vec![grounding],
    );
    corrected.reviewed.push(alternate);
    let correction = request(
        &writer,
        11,
        vec![ObservationOperation::Successor {
            id: first,
            expected_revision: 1,
            draft: corrected,
            change: ObservationChange::Correction,
            credited_activity: None,
        }],
    );
    assert_eq!(
        writer
            .publish_observation_batch(correction)
            .unwrap()
            .revisions,
        [(first, 2)]
    );
    assert_eq!(
        writer.observation_revision(first, 1).unwrap().standing,
        ObservationStanding::Superseded
    );
    assert!(writer.observation_revision_source_eligible(first, 2));
    drop(writer);

    let original = PhySession::open_path_current(&path).unwrap();
    assert_eq!(
        original.historical_proof(OWNER, early).unwrap(),
        early_proof
    );
    let head = original.current_historical_cut();
    let proof = original.historical_proof(OWNER, head).unwrap();
    assert!(matches!(
        original.historical_proof(
            OWNER,
            HistoricalOwnerCut {
                graph_version: 0,
                perception_version: 0,
                ..head
            }
        ),
        Err(HistoricalProofError::InconsistentCut(_))
    ));
    assert_eq!(original.relationship_revision(manual.id, 1), Some(&manual));
    assert_eq!(
        original
            .historical_observation_citation_text(&relation_cite)
            .unwrap(),
        "Áda and Bob are related."
    );
    assert!(!original.observation_revision_source_eligible(first, 1));
    drop(original);

    let copy = temp.path().join("copy.phy");
    let plan = VerifiedOwnerCopyPlan::prepare(&path).unwrap();
    plan.copy_to(&path, &copy).unwrap();
    let copied = PhySession::open_path_current(&copy).unwrap();
    assert_eq!(copied.historical_proof(OWNER, head).unwrap(), proof);
    assert_eq!(copied.historical_proof(OWNER, early).unwrap(), early_proof);
    let prune = temp.path().join("prune.phy");
    let result = prune_terminal_memory_copy(
        &plan,
        &path,
        &prune,
        &[memory_id(alternate)],
        &GraphAdmission::for_replay(OWNER, [], [], [], [], []),
    );
    assert!(
        result
            .unwrap_err()
            .contains("required by retained Observation")
    );
    assert!(!prune.exists());
}

#[test]
fn phy_observation_replay_rejects_unclocked_or_wrong_prefix_records() {
    let temp = fixture_directory();
    let path = temp.path().join("wrong-cut.phy");
    let mut writer = PhyWriter::create_path(&path, OWNER).unwrap();
    let input = request(&writer, 1, Vec::new());
    let native = PhyObservationRecord {
        cut: PhyObservationCut {
            audit: PhyAuditCut {
                graph: 1,
                ..input.cut.audit
            },
            ..input.cut
        },
        batch: input.batch,
        lineages: Vec::new(),
        source_availability: [0; 32],
        claim_vectors: None,
    };
    writer
        .container
        .append_publication_envelope(&native.encode().unwrap(), None)
        .unwrap();
    writer.container.sync().unwrap();
    drop(writer);
    let error = PhySession::open_path_current(&path)
        .err()
        .expect("invalid cut was admitted");
    assert!(error.to_string().contains("physical cut"));

    let path = temp.path().join("unclocked.phy");
    let mut writer = PhyWriter::create_path(&path, OWNER).unwrap();
    writer.container.append(PHY_MARKER).unwrap();
    writer.container.sync().unwrap();
    drop(writer);
    assert!(PhySession::open_path_current(&path).is_err());

    let path = temp.path().join("wrong-owner.rel");
    let mut writer = RelWriter::create_current_path(&path, OWNER).unwrap();
    writer
        .container
        .append_publication_envelope(&native.encode().unwrap(), None)
        .unwrap();
    writer.container.sync().unwrap();
    drop(writer);
    assert!(RelSession::open_path_current(&path).is_err());
}

#[test]
fn phy_observation_replay_uses_the_physical_dream_settlement_prefix() {
    let temp = fixture_directory();
    let path = temp.path().join("dream.phy");
    let mut writer = PhyWriter::create_path(&path, OWNER).unwrap();
    let text = "The private workshop is held on Monday.";
    let MemoryRevision { record, .. } = local_memory(&mut writer, "workshop", text, false);
    writer.writer_lease_proof = writer.container.writer_lease_proof();
    let citation = ObservationCitation {
        owner_id: OWNER,
        history_generation: writer.history_generation,
        source: ObservationSource::Memory(record.id.0),
        revision: record.revision,
        body_id: record.body_id.0,
        field: MemoryTextField::Content,
        start_byte: 0,
        end_byte: text.len() as u32,
    };
    let unsettled = request(
        &writer,
        1,
        vec![ObservationOperation::Create {
            draft: draft("The workshop meets Mondays.", vec![citation]),
        }],
    );
    assert!(writer.publish_observation_batch(unsettled.clone()).is_err());
    let source = DreamMemoryCut {
        id: record.id,
        revision: record.revision,
        body_id: record.body_id,
    };
    writer
        .begin_initial_dream_pass("workshop-first".into(), source, 0)
        .unwrap();
    writer
        .commit_empty_dream_pass("workshop-first", source, 0, 0)
        .unwrap();
    assert_eq!(writer.phy_observation_cut().audit.settlements, 1);
    assert!(matches!(
        writer.publish_observation_batch(unsettled),
        Err(OwnerSessionError::ManagementStale)
    ));
    let id = create(
        &mut writer,
        1,
        draft("The workshop meets Mondays.", vec![citation]),
    );
    drop(writer);

    let reopened = PhySession::open_path_current(&path).unwrap();
    assert_eq!(reopened.observation_citation_text(&citation).unwrap(), text);
    assert_eq!(
        reopened.observation(id).unwrap().standing,
        ObservationStanding::Current
    );
}
