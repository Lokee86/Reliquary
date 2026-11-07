//! Joined G4-T03 semantic cohort slice over the accepted Gateway seams.
//!
//! This intentionally exercises generated native state through queue,
//! Episode, and Insomnia publication APIs. It does not seed final owner tables.

#[cfg(test)]
use super::*;
use reliquary_application::{
    DreamClassifier, DreamVerificationPolicy, DreamVerifier, EntityMemoryMention,
    EntityOwnerSession, NativeInsomniaExtraction, TextField, draft_dream_batch_organization,
    extract_native_insomnia, resolve_native_insomnia_evidence,
};
use reliquary_container::ReliquaryScope;
use reliquary_owner_session::{
    DreamMemoryCut,
    model::{
        EntityResolutionReason, EpisodeBoundary, InsomniaEpisodeId, MemoryEntityResolutionStatus,
        MemoryId,
    },
};
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

#[derive(Clone)]
struct CohortEndpoint;

impl GeneralEndpoint for CohortEndpoint {
    fn model(&self) -> &str {
        "g4-t03-scripted"
    }

    fn complete_json(
        &self,
        _: &str,
        payload: &str,
        schema_name: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        match schema_name {
            "insomnia_authority_disposition_ledger" => {
                let evidence_requests = if payload.contains("read_only_archive_evidence") {
                    json!([])
                } else {
                    json!([{"kind":"turn","conversation_id":"lantern-evidence",
                        "node_id":"source","start_node_id":"","end_node_id":"",
                        "query":"","limit":1}])
                };
                Ok(json!({"turns":{"accepted":[
                    {"disposition":"retain","authority_kind":"direct","category":"preference",
                     "type":"communication","lifecycle":"current",
                     "proposition":"The user prefers afternoon meetings.",
                     "authority_source_node_id":"","grounding_source_node_id":"",
                     "reason":"direct user preference"},
                    {"disposition":"retain","authority_kind":"direct","category":"instruction",
                     "type":"project","lifecycle":"current",
                     "proposition":"Use the Lantern release checklist.",
                     "authority_source_node_id":"","grounding_source_node_id":"source",
                     "reason":"project instruction grounded in the archive"},
                    {"disposition":"retain","authority_kind":"direct","category":"fact",
                     "type":"process","lifecycle":"current",
                     "proposition":"Lantern reviews the release checklist weekly.",
                     "authority_source_node_id":"","grounding_source_node_id":"",
                     "reason":"direct project review cadence"}
                ]},"evidence_requests":evidence_requests}))
            }
            "insomnia_memory_ownership" => {
                Ok(json!({"ownership":{"g000":"user","g001":"project","g002":"project"}}))
            }
            "insomnia_memory_wording" => Ok(json!({"candidates":[
                {"authority_kind":"direct","category":"preference","memory_type":"communication",
                 "temporal_status":"current","ownership":"user","title":"Meeting window",
                 "content":"The user prefers afternoon meetings.","source_node_id":"accepted",
                 "source_quote":"I prefer afternoon meetings.","authority_source":null,
                 "grounding_source":null,"routing_mentions":[]},
                {"authority_kind":"direct","category":"instruction","memory_type":"project",
                 "temporal_status":"current","ownership":"project","title":"Release checklist",
                 "content":"Use the Lantern release checklist.","source_node_id":"accepted",
                 "source_quote":"Use the Lantern release checklist.","authority_source":null,
                 "grounding_source":{"conversation_id":"lantern-evidence","node_id":"source",
                     "quote":"The release checklist is the approved Lantern procedure."},
                 "routing_mentions":[]},
                {"authority_kind":"direct","category":"fact","memory_type":"process",
                 "temporal_status":"current","ownership":"project","title":"Release review",
                 "content":"Lantern reviews the release checklist weekly.","source_node_id":"accepted",
                 "source_quote":"Lantern reviews the release checklist weekly.","authority_source":null,
                 "grounding_source":null,
                 "routing_mentions":[]}
            ]})),
            "dream_pair_classification" => Ok(json!({
                "relation":"duplicate_of", "direction":"undirected",
                "evidence":[{"side":"a","quote":"Lantern"},{"side":"b","quote":"Lantern"}]
            })),
            "dream_pair_verification" => Ok(json!({
                "relation_supported":"yes", "direction_supported":"yes",
                "evidence_supported":"yes"
            })),
            "entity_resolution" => Ok(json!({
                "decision":"create_new", "reason":"named_referent",
                "target_candidate_index":-1, "identity_relation":"related_distinct",
                "mention_kind":"application"
            })),
            "entity_admission" => Ok(json!({
                "decision":"create_new", "reason":"first_seen_identity",
                "promotion_policy":"immediate", "entity_kind":"application",
                "entity_summary":"Lantern is the project governed by the release checklist."
            })),
            _ => panic!("unexpected scripted endpoint call: {schema_name}"),
        }
    }
}

struct ExistingEntityEndpoint;

impl GeneralEndpoint for ExistingEntityEndpoint {
    fn model(&self) -> &str {
        "g4-t03-existing-entity"
    }

    fn complete_json(
        &self,
        _: &str,
        _: &str,
        schema_name: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        assert_eq!(schema_name, "entity_resolution");
        Ok(json!({
            "decision":"resolve_existing", "reason":"context_match",
            "target_candidate_index":0, "identity_relation":"same_identity",
            "mention_kind":"application"
        }))
    }
}

struct AmbiguousEntityEndpoint;

impl GeneralEndpoint for AmbiguousEntityEndpoint {
    fn model(&self) -> &str {
        "g4-t03-ambiguous-entity"
    }

    fn complete_json(
        &self,
        _: &str,
        _: &str,
        schema_name: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        assert_eq!(schema_name, "entity_resolution");
        Ok(json!({
            "decision":"unresolved", "reason":"ambiguous",
            "target_candidate_index":-1, "identity_relation":"uncertain",
            "mention_kind":"application"
        }))
    }
}

struct CohortDir(PathBuf);

impl CohortDir {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "g4-t03-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for CohortDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn accepted_turn(
    instance: &Instance,
    queue: &mut ConversationQueue,
    conversation: &str,
    node: &str,
    text: &str,
    parent: Option<&str>,
    timestamp: i64,
) {
    let submission = SubmissionId::new(node).unwrap();
    let id = ConversationSubmissionId::new(conversation, submission.clone()).unwrap();
    let revision = queue.conversation_revision(conversation);
    instance
        .submit_conversation(queue, conversation, submission, text, revision)
        .unwrap();
    let revision = queue.conversation_revision(conversation);
    instance
        .finalize_conversation_submission(
            queue,
            &id,
            revision,
            FinalizeCapture {
                branch_id: "main".into(),
                parent_node_id: parent.map(str::to_owned),
                committed_timestamp_ns: timestamp,
                verified_principal_id: "principal-a".into(),
            },
        )
        .unwrap();
}

#[test]
fn g4_t03_lantern_cascade_routed_insomnia_reopens_with_exact_provenance() {
    let dir = CohortDir::new();
    let lantern_id = [201; 16];
    let cascade_id = [202; 16];
    let phy_a_id = [203; 16];
    let phy_b_id = [204; 16];
    let lantern_path = dir.0.join("rel-lantern.rel");
    let cascade_path = dir.0.join("rel-cascade.rel");
    let phy_a_path = dir.0.join("phy-reader-a.phy");
    let phy_b_path = dir.0.join("phy-reader-b.phy");
    for (path, id) in [(&lantern_path, lantern_id), (&cascade_path, cascade_id)] {
        drop(RelWriter::create_path(path, ReliquaryScope::Project, id).unwrap());
    }
    for (path, id) in [(&phy_a_path, phy_a_id), (&phy_b_path, phy_b_id)] {
        drop(PhyWriter::create_path(path, id).unwrap());
    }
    let admissions = [lantern_id, cascade_id, phy_a_id, phy_b_id]
        .map(|id| GraphAdmission::for_replay(id, [], [], [], [], []));

    let gateway = Gateway::new();
    let lantern = gateway
        .mount_writable_reliquary(&lantern_path, &admissions[0], [], [])
        .unwrap();
    let cascade = gateway
        .mount_writable_reliquary(&cascade_path, &admissions[1], [], [])
        .unwrap();
    let phy_a = gateway
        .mount_writable_phylactery(&phy_a_path, &admissions[2], [])
        .unwrap();
    let phy_b = gateway
        .mount_writable_phylactery(&phy_b_path, &admissions[3], [])
        .unwrap();
    let instance = gateway
        .instance("principal-a", [lantern, cascade], [phy_a])
        .unwrap();
    let outsider = gateway.instance("outsider", [], [phy_b]).unwrap();

    let mut queue = instance
        .open_conversation_queue(&dir.0.join("lantern.queue"), lantern)
        .unwrap();
    let mut cascade_queue = instance
        .open_conversation_queue(&dir.0.join("cascade.queue"), cascade)
        .unwrap();
    accepted_turn(
        &instance,
        &mut queue,
        "lantern-evidence",
        "source",
        "The release checklist is the approved Lantern procedure.",
        None,
        10,
    );
    accepted_turn(
        &instance,
        &mut queue,
        "lantern-release",
        "accepted",
        "I prefer afternoon meetings. Use the Lantern release checklist. Lantern reviews the release checklist weekly.",
        None,
        20,
    );
    accepted_turn(
        &instance,
        &mut cascade_queue,
        "cascade-notes",
        "cascade-turn",
        "Cascade maintains its separate operating plan.",
        None,
        30,
    );
    let episode = instance
        .finalize_conversation_episode(
            lantern,
            "lantern-release",
            "accepted",
            "accepted",
            EpisodeBoundary::Explicit,
            21,
        )
        .unwrap();
    assert!(matches!(
        outsider.open_conversation_queue(&dir.0.join("denied.queue"), lantern),
        Err(GatewayError::UnauthorizedRel { .. })
    ));

    let claim = instance
        .claim_next_insomnia_input(lantern, "g4-t03", 30, 100, 8)
        .unwrap()
        .unwrap();
    let extraction = extract_native_insomnia(CohortEndpoint, &claim, Some(phy_a_id), |requests| {
        instance
            .with_writable_reliquary(lantern, |rel| {
                resolve_native_insomnia_evidence(rel, &claim, requests)
            })
            .unwrap()
    })
    .unwrap();
    let checkpoint = extraction.checkpoint(&claim).unwrap();
    let mut active_claim = claim.clone();
    active_claim.work = instance
        .checkpoint_insomnia_claim(lantern, &claim, 31, checkpoint.clone())
        .unwrap();
    let prepared = instance
        .with_writable_reliquary(lantern, |rel| extraction.prepare(rel, &claim))
        .unwrap()
        .unwrap();
    assert_eq!(prepared.user_memories().count(), 1);
    assert_eq!(prepared.project_memories().count(), 2);

    let rel_before_rejections = fs::metadata(&lantern_path).unwrap().len();
    let phy_before_rejections = fs::metadata(&phy_a_path).unwrap().len();
    assert!(matches!(
        outsider.commit_routed_insomnia_claim(
            lantern,
            phy_a,
            &active_claim,
            &prepared,
            "g4-t03-scripted",
            "v1",
            32
        ),
        Err(GatewayError::UnauthorizedRel { .. })
    ));
    let no_phy_grant = gateway.instance("principal-a", [lantern], []).unwrap();
    assert!(matches!(
        no_phy_grant.commit_routed_insomnia_claim(
            lantern,
            phy_a,
            &active_claim,
            &prepared,
            "g4-t03-scripted",
            "v1",
            32
        ),
        Err(GatewayError::UnauthorizedPhy { .. })
    ));
    let mut forged_claim = active_claim.clone();
    forged_claim.owner_id = cascade_id;
    assert!(matches!(
        instance.commit_routed_insomnia_claim(
            lantern,
            phy_a,
            &forged_claim,
            &prepared,
            "g4-t03-scripted",
            "v1",
            32
        ),
        Err(GatewayError::RoutedMemory(_))
    ));
    assert!(matches!(
        instance.commit_routed_insomnia_claim(
            lantern,
            phy_a,
            &active_claim,
            &prepared,
            "g4-t03-scripted",
            "v1",
            200
        ),
        Err(GatewayError::OwnerSession(_))
    ));
    assert_eq!(
        fs::metadata(&lantern_path).unwrap().len(),
        rel_before_rejections
    );
    assert_eq!(
        fs::metadata(&phy_a_path).unwrap().len(),
        phy_before_rejections
    );
    let completion = instance
        .commit_routed_insomnia_claim(
            lantern,
            phy_a,
            &active_claim,
            &prepared,
            "g4-t03-scripted",
            "v1",
            32,
        )
        .unwrap();
    assert_eq!(completion, CompletionCheck::Apply);

    let project_id = instance
        .with_writable_reliquary(lantern, |rel| {
            rel.insomnia_receipt(InsomniaEpisodeId(episode.0), 1)
                .unwrap()
                .memory_ids[0]
        })
        .unwrap();
    let dream_cuts = instance
        .with_writable_reliquary(lantern, |rel| {
            let receipt = rel
                .insomnia_receipt(InsomniaEpisodeId(episode.0), 1)
                .unwrap();
            receipt
                .memory_ids
                .iter()
                .map(|id| {
                    let record = rel.current_memory(MemoryId(id.0)).unwrap().record;
                    DreamMemoryCut {
                        id: record.id,
                        revision: record.revision,
                        body_id: record.body_id,
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap();
    assert_eq!(dream_cuts.len(), 2);
    instance
        .with_rel_writer(lantern, |writer| {
            writer.begin_initial_dream_frontier(
                "g4-lantern-first-pass".into(),
                dream_cuts[0],
                vec![dream_cuts[1]],
                33,
            )
        })
        .unwrap()
        .unwrap();
    let dream = instance
        .with_writable_reliquary(lantern, |rel| {
            draft_dream_batch_organization(
                &EntityOwnerSession::Rel(rel),
                lantern_id,
                dream_cuts[0].id,
                &[dream_cuts[1].id],
                &DreamClassifier::new(CohortEndpoint),
                Some(&DreamVerifier::new(CohortEndpoint)),
                DreamVerificationPolicy::default(),
            )
        })
        .unwrap()
        .unwrap();
    assert!(
        instance
            .publish_dream_batch(lantern, [0xd4; 32], &dream, 34)
            .unwrap()
    );
    assert!(
        !instance
            .publish_dream_batch(lantern, [0xd4; 32], &dream, 34)
            .unwrap()
    );
    let entity_input = EntityMemoryMention {
        owner_id: lantern_id,
        memory_id: MemoryId(project_id.0),
        field: TextField::Content,
        start_byte: 8,
        end_byte: 15,
        text: "Lantern".into(),
    };
    assert!(matches!(
        outsider.resolve_and_publish_entity_mention(entity_input.clone(), CohortEndpoint, 33),
        Err(GatewayError::UnauthorizedRel { .. } | GatewayError::UnauthorizedPhy { .. })
    ));
    let entity_resolution = instance
        .resolve_and_publish_entity_mention(entity_input, CohortEndpoint, 33)
        .unwrap();
    let created_entity = match &entity_resolution.status {
        MemoryEntityResolutionStatus::Resolved { entity_id, .. } => *entity_id,
        ref status => panic!("new Entity mention was not resolved: {status:?}"),
    };
    let known_resolution = instance
        .resolve_and_publish_entity_mention(
            EntityMemoryMention {
                owner_id: lantern_id,
                memory_id: dream_cuts[1].id,
                field: TextField::Content,
                start_byte: 0,
                end_byte: 7,
                text: "Lantern".into(),
            },
            ExistingEntityEndpoint,
            34,
        )
        .unwrap();
    assert!(matches!(
        &known_resolution.status,
        MemoryEntityResolutionStatus::Resolved {
            entity_id,
            reason: EntityResolutionReason::ContextMatch,
        } if *entity_id == created_entity
    ));
    let ambiguous_resolution = instance
        .resolve_and_publish_entity_mention(
            EntityMemoryMention {
                owner_id: lantern_id,
                memory_id: dream_cuts[1].id,
                field: TextField::Content,
                start_byte: 20,
                end_byte: 27,
                text: "release".into(),
            },
            AmbiguousEntityEndpoint,
            35,
        )
        .unwrap();
    assert!(matches!(
        &ambiguous_resolution.status,
        MemoryEntityResolutionStatus::Ambiguous { .. }
            | MemoryEntityResolutionStatus::Unresolved {
                reason: EntityResolutionReason::Ambiguous,
            }
    ));

    drop(no_phy_grant);
    drop(outsider);
    drop(queue);
    drop(cascade_queue);
    drop(instance);
    drop(gateway);

    let reopened_rel = RelSession::open_path(&lantern_path, &admissions[0], [], []).unwrap();
    let reopened_cascade = RelSession::open_path(&cascade_path, &admissions[1], [], []).unwrap();
    let reopened_phy_a = PhySession::open_path(&phy_a_path, &admissions[2], []).unwrap();
    let reopened_phy_b = PhySession::open_path(&phy_b_path, &admissions[3], []).unwrap();
    assert_eq!(reopened_rel.owner_id(), lantern_id);
    assert_eq!(reopened_cascade.owner_id(), cascade_id);
    assert_eq!(reopened_phy_a.owner_id(), phy_a_id);
    assert_eq!(reopened_phy_b.owner_id(), phy_b_id);
    let episode_receipt = reopened_rel
        .insomnia_receipt(InsomniaEpisodeId(episode.0), 1)
        .unwrap();
    assert_eq!(episode_receipt.memory_ids.len(), 2);
    assert_eq!(episode_receipt.external_memory_refs.len(), 1);
    assert!(
        reopened_rel
            .initial_dream_frontier("g4-lantern-first-pass")
            .is_some()
    );
    assert!(reopened_rel.dream_pair_evaluated(
        MemoryId(episode_receipt.memory_ids[0].0),
        MemoryId(episode_receipt.memory_ids[1].0)
    ));
    let user_id = MemoryId(episode_receipt.external_memory_refs[0].memory_id.0);
    assert_eq!(
        episode_receipt.external_memory_refs[0].owner_id,
        crate::hex(&phy_a_id)
    );
    let user = reopened_phy_a.memory_history(user_id).unwrap().unwrap();
    let user_record = &user.revisions[0].record;
    assert!(user_record.source_episode_id.is_none());
    assert_eq!(
        user_record.source_ref.as_ref().unwrap().source_episode_id,
        episode.0
    );
    assert_eq!(user_record.source_time_ns, Some(20));
    assert_eq!(
        user_record.source_ref.as_ref().unwrap().source_node_id,
        "accepted"
    );
    assert!(reopened_phy_b.memory_history(user_id).unwrap().is_none());

    let project = reopened_rel
        .memory_history(MemoryId(episode_receipt.memory_ids[0].0))
        .unwrap()
        .unwrap();
    let project_record = &project.revisions[0].record;
    assert_eq!(project_record.source_episode_id, Some(episode.0));
    assert_eq!(project_record.source_time_ns, Some(20));
    assert_eq!(
        project_record.grounding_source_node_id.as_deref(),
        Some("source")
    );
    assert_eq!(
        project_record.grounding_source_conversation_id.as_deref(),
        Some("lantern-evidence")
    );
    assert_eq!(
        reopened_rel.mention(entity_resolution.key),
        Some(&entity_resolution)
    );
    assert_eq!(
        reopened_rel.mention(known_resolution.key),
        Some(&known_resolution)
    );
    assert_eq!(
        reopened_rel.mention(ambiguous_resolution.key),
        Some(&ambiguous_resolution)
    );
    assert!(
        reopened_cascade
            .archive_node("cascade-notes", "cascade-turn")
            .is_some()
    );
    assert!(
        reopened_cascade
            .archive_node("lantern-release", "accepted")
            .is_none()
    );
    assert!(
        reopened_rel
            .archive_node("lantern-release", "accepted")
            .is_some()
    );
    assert!(matches!(
        NativeInsomniaExtraction::reopen_checkpoint(&claim, Some([222; 16]), &checkpoint),
        Err(_)
    ));
}

const G4_AUTHORED_MANIFEST: &str = include_str!("../../../tests/fixtures/g4-native-rel21-v1.json");

fn accepted_authored_turn(
    instance: &Instance,
    queue: &mut ConversationQueue,
    conversation: &str,
    node: &str,
    text: &str,
    branch: &str,
    parent: Option<&str>,
    principal: &str,
    timestamp: i64,
) {
    let submission = SubmissionId::new(node).unwrap();
    let id = ConversationSubmissionId::new(conversation, submission.clone()).unwrap();
    let revision = queue.conversation_revision(conversation);
    instance
        .submit_conversation(queue, conversation, submission, text, revision)
        .unwrap();
    let revision = queue.conversation_revision(conversation);
    instance
        .finalize_conversation_submission(
            queue,
            &id,
            revision,
            FinalizeCapture {
                branch_id: branch.into(),
                parent_node_id: parent.map(str::to_owned),
                committed_timestamp_ns: timestamp,
                verified_principal_id: principal.into(),
            },
        )
        .unwrap();
}

/// G4-T02 replay: accepted queue submissions are regenerated from the authored
/// manifest and then compared through independent owner sessions.
#[test]
fn g4_t02_replays_authored_12_conversation_48_turn_rel21_cohort() {
    let manifest: Value = serde_json::from_str(G4_AUTHORED_MANIFEST).unwrap();
    assert_eq!(manifest["fixture_seed"], "g4-rel21-v1");
    let dir = CohortDir::new();
    let owner_ids = [[211; 16], [212; 16], [213; 16], [214; 16]];
    let rel_paths = [dir.0.join("rel-lantern.rel"), dir.0.join("rel-cascade.rel")];
    let phy_paths = [
        dir.0.join("phy-reader-a.phy"),
        dir.0.join("phy-reader-b.phy"),
    ];
    drop(RelWriter::create_path(&rel_paths[0], ReliquaryScope::Project, owner_ids[0]).unwrap());
    drop(RelWriter::create_path(&rel_paths[1], ReliquaryScope::Connection, owner_ids[1]).unwrap());
    for (path, owner) in phy_paths.iter().zip(owner_ids[2..].iter().copied()) {
        drop(PhyWriter::create_path(path, owner).unwrap());
    }
    let admissions = owner_ids.map(|owner| GraphAdmission::for_replay(owner, [], [], [], [], []));
    let gateway = Gateway::new();
    let rel_lantern = gateway
        .mount_writable_reliquary(&rel_paths[0], &admissions[0], [], [])
        .unwrap();
    let rel_cascade = gateway
        .mount_writable_reliquary(&rel_paths[1], &admissions[1], [], [])
        .unwrap();
    let phy_a = gateway
        .mount_writable_phylactery(&phy_paths[0], &admissions[2], [])
        .unwrap();
    let phy_b = gateway
        .mount_writable_phylactery(&phy_paths[1], &admissions[3], [])
        .unwrap();
    let principal_a = gateway
        .instance("principal-a", [rel_lantern], [phy_a])
        .unwrap();
    let principal_b = gateway
        .instance("principal-b", [rel_cascade], [phy_b])
        .unwrap();
    let outsider = gateway.instance("outsider", [], []).unwrap();
    let mut lantern_queue = principal_a
        .open_conversation_queue(&dir.0.join("lantern.queue"), rel_lantern)
        .unwrap();
    let mut cascade_queue = principal_b
        .open_conversation_queue(&dir.0.join("cascade.queue"), rel_cascade)
        .unwrap();

    let mut timestamp = 1_i64;
    let mut actual_turns = 0usize;
    let scripts = manifest["conversation_scripts"].as_array().unwrap();
    assert_eq!(scripts.len(), 12);
    for script in scripts {
        let owner_alias = script["owner"].as_str().unwrap();
        let (instance, queue, principal) = if owner_alias == "rel-lantern" {
            (&principal_a, &mut lantern_queue, "principal-a")
        } else {
            assert_eq!(owner_alias, "rel-cascade");
            (&principal_b, &mut cascade_queue, "principal-b")
        };
        let conversation = script["conversation_id"].as_str().unwrap();
        let turns = script["turns"].as_array().unwrap();
        let mut previous = None::<String>;
        for turn in turns {
            assert_eq!(turn["accepted"], true);
            let node = turn["id"].as_str().unwrap();
            let text = turn["content"].as_str().unwrap();
            let fork_parent = script["branches"]
                .as_array()
                .unwrap()
                .iter()
                .find(|branch| {
                    branch["child_turn_ids"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|child| child.as_str() == Some(node))
                })
                .and_then(|branch| branch["parent_turn_id"].as_str());
            let parent = fork_parent.or(previous.as_deref());
            let branch = script["branches"]
                .as_array()
                .unwrap()
                .iter()
                .find(|candidate| {
                    candidate["child_turn_ids"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|child| child.as_str() == Some(node))
                })
                .map(|candidate| {
                    if candidate["canonical_child_id"].as_str() == Some(node) {
                        "main".to_owned()
                    } else {
                        format!("alternate-{}", candidate["id"].as_str().unwrap())
                    }
                })
                .unwrap_or_else(|| "main".to_owned());
            accepted_authored_turn(
                instance,
                queue,
                conversation,
                node,
                text,
                &branch,
                parent,
                principal,
                timestamp,
            );
            previous = Some(node.to_owned());
            timestamp += 1;
            actual_turns += 1;
        }
    }
    assert_eq!(actual_turns, 48);
    assert!(matches!(
        outsider.open_conversation_queue(&dir.0.join("denied-cohort.queue"), rel_lantern),
        Err(GatewayError::UnauthorizedRel { .. })
    ));

    drop(lantern_queue);
    drop(cascade_queue);
    drop(principal_a);
    drop(principal_b);
    drop(outsider);
    drop(gateway);

    let lantern = RelSession::open_path(&rel_paths[0], &admissions[0], [], []).unwrap();
    let cascade = RelSession::open_path(&rel_paths[1], &admissions[1], [], []).unwrap();
    let phy_a = PhySession::open_path(&phy_paths[0], &admissions[2], []).unwrap();
    let phy_b = PhySession::open_path(&phy_paths[1], &admissions[3], []).unwrap();
    assert_eq!(lantern.owner_id(), owner_ids[0]);
    assert_eq!(cascade.owner_id(), owner_ids[1]);
    assert_eq!(phy_a.owner_id(), owner_ids[2]);
    assert_eq!(phy_b.owner_id(), owner_ids[3]);

    let mut observed_turns = 0usize;
    let mut observed_forks = 0usize;
    for script in scripts {
        let owner = if script["owner"] == "rel-lantern" {
            &lantern
        } else {
            &cascade
        };
        let conversation = script["conversation_id"].as_str().unwrap();
        for turn in script["turns"].as_array().unwrap() {
            let node = turn["id"].as_str().unwrap();
            let actual = owner
                .resolved_archive_turn(conversation, node)
                .unwrap()
                .expect("accepted authored turn is present after owner reopen");
            assert_eq!(actual.content, turn["content"].as_str().unwrap());
            // Gateway finalization accepted the manifest-authored parent above.
            observed_turns += 1;
        }
        observed_forks += script["branches"].as_array().unwrap().len();
    }
    assert_eq!(observed_turns, 48);
    assert_eq!(observed_forks, 3);

    // Authored semantic witnesses are checked against the exact accepted source
    // turn and declared REL/PHY route, not text inferred by the test endpoint.
    for source in manifest["source_references"].as_array().unwrap() {
        let script = scripts
            .iter()
            .find(|script| script["id"] == source["script_id"])
            .unwrap();
        let owner = if script["owner"] == "rel-lantern" {
            &lantern
        } else {
            &cascade
        };
        let conversation = script["conversation_id"].as_str().unwrap();
        let node = source["turn_id"].as_str().unwrap();
        let exact_span = source["exact_span"].as_str().unwrap();
        let actual = owner
            .resolved_archive_turn(conversation, node)
            .unwrap()
            .unwrap();
        assert!(actual.content.contains(exact_span), "{}", source["id"]);
        let route = &source["expected_route"];
        match route["owner_kind"].as_str().unwrap() {
            "REL" => assert!(matches!(
                route["owner"].as_str().unwrap(),
                "rel-lantern" | "rel-cascade"
            )),
            "PHY" => {
                assert_eq!(route["class"], "user");
                assert!(matches!(
                    route["principal"].as_str().unwrap(),
                    "principal-a" | "principal-b"
                ));
            }
            kind => panic!("unexpected authored route kind {kind}"),
        }
    }
}
