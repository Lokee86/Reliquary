//! Joined G4-T04 vector, context, retrieval, and accepted-use scenario.
//!
//! The fixture creates Archive evidence through accepted queue and Insomnia
//! workflows. Authored multi-result ordering and the documented missing-community
//! fallback share the same owner cohort as host-status, revocation and reopen.

#[cfg(test)]
use super::*;
use reliquary_application::{
    CompactionBudget, ContextRole, ContextTokenCounter, ContextTurn, ContextView,
    EmbeddingEndpoint, EmbeddingEndpointError, ProjectCorrelationRequest, RetrievalAction,
    RetrievalConfig, VectorNormalization, extract_native_insomnia,
    resolve_native_insomnia_evidence,
};
use reliquary_container::ReliquaryScope;
use reliquary_owner_session::model::{
    EpisodeBoundary, InsomniaEpisodeId, MemoryId, PayloadState, ProjectRepositoryKind,
    ProjectRepositoryRef,
};
use reliquary_switchboard::{
    CredentialId, DecisionEndpoint, DecisionEndpointError, DecisionModelEndpoint, GeneralEndpoint,
    GeneralEndpointError, ModelSwitchboardConfig,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

fn git(root: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().to_owned()
}

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("g4-t04-{}-{}", std::process::id(), Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone)]
struct InsomniaScript;
impl GeneralEndpoint for InsomniaScript {
    fn model(&self) -> &str {
        "g4-t04-insomnia"
    }
    fn complete_json(
        &self,
        _: &str,
        _: &str,
        schema: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        match schema {
            "insomnia_authority_disposition_ledger" => Ok(json!({
                "turns":{"accepted":[
                    {"disposition":"retain","authority_kind":"direct","category":"fact",
                    "type":"project","lifecycle":"current",
                    "proposition":"Lantern release uses the amber checklist.",
                    "authority_source_node_id":"","grounding_source_node_id":"",
                    "reason":"direct source statement"},
                    {"disposition":"retain","authority_kind":"direct","category":"preference",
                    "type":"communication","lifecycle":"current",
                    "proposition":"My preferred meeting window is afternoons.",
                    "authority_source_node_id":"","grounding_source_node_id":"",
                    "reason":"direct user preference"}
                ]},"evidence_requests":[]
            })),
            "insomnia_memory_ownership" => {
                Ok(json!({"ownership":{"g000":"project","g001":"user"}}))
            }
            "insomnia_memory_wording" => Ok(json!({"candidates":[{
                "authority_kind":"direct","category":"fact","memory_type":"project",
                "temporal_status":"current","ownership":"project","title":"Amber checklist",
                "content":"Lantern release uses the amber checklist.","source_node_id":"accepted",
                "source_quote":"Lantern release uses the amber checklist.",
                "authority_source":null,"grounding_source":null,"routing_mentions":[]
            },{
                "authority_kind":"direct","category":"preference","memory_type":"communication",
                "temporal_status":"current","ownership":"user","title":"Meeting window",
                "content":"My preferred meeting window is afternoons.","source_node_id":"accepted",
                "source_quote":"My preferred meeting window is afternoons.","authority_source":null,
                "grounding_source":null,"routing_mentions":[]
            }]})),
            _ => panic!("unexpected Insomnia schema: {schema}"),
        }
    }
}

pub(crate) struct Embed {
    pub(crate) calls: AtomicUsize,
    pub(crate) dimensions: u32,
}
impl EmbeddingEndpoint for Embed {
    fn dimensions(&self) -> u32 {
        self.dimensions
    }
    fn normalization(&self) -> VectorNormalization {
        VectorNormalization::L2
    }
    fn supports_bounded_embedding(&self) -> bool {
        true
    }
    fn embed(
        &self,
        _: reliquary_owner_session::model::EmbeddingMode,
        _: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        panic!("unbounded embedding is not allowed")
    }
    fn embed_bounded(
        &self,
        _: reliquary_owner_session::model::EmbeddingMode,
        inputs: &[String],
        max_bytes: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        assert!(inputs.iter().map(String::len).sum::<usize>() <= max_bytes);
        Ok(inputs
            .iter()
            .map(|text| {
                let mut row = vec![0.0; self.dimensions as usize];
                row[usize::from(!text.contains("amber checklist"))] = 1.0;
                row
            })
            .collect())
    }
}

struct Approve;
impl DecisionEndpoint for Approve {
    fn model(&self) -> &str {
        "g4-t04-fixed-decision"
    }
    fn evaluate(&self, state: &Value, _: &Value) -> Result<Value, DecisionEndpointError> {
        let candidates = state["candidates"].as_array().expect("candidate list");
        Ok(
            json!({"version":1,"approved":candidates.iter().map(|row| row["memory"].clone()).collect::<Vec<_>>()}),
        )
    }
}

pub(crate) fn retrieval_route() -> MemoryRetrievalRoute {
    let config = ModelSwitchboardConfig {
        memory_retrieval_decision: Some(DecisionModelEndpoint {
            model: "g4-fixed-decision".into(),
            url: "local://g4".into(),
            credential_id: CredentialId::new("g4-fixture").unwrap(),
        }),
        ..Default::default()
    };
    MemoryRetrievalRoute::resolve(&config, Some(Arc::new(Approve)), None, None)
        .unwrap()
        .unwrap()
}

#[derive(Default)]
pub(crate) struct Host {
    operations: Mutex<HashMap<String, MemoryDeliveryOperation>>,
    calls: AtomicUsize,
    reject: AtomicBool,
    mismatch: AtomicBool,
    unknown: AtomicBool,
    revoke_on_delivery: Mutex<Option<(Arc<HostAdmissionControl>, String)>>,
}
impl TrustedHostAdapter for Host {
    fn configured_capabilities(&self) -> BTreeSet<RuntimeCapability> {
        [RuntimeCapability::MemoryDelivery].into()
    }
    fn connect(&self) -> Result<HostConnection, String> {
        Ok(HostConnection {
            capabilities: [(
                RuntimeCapability::MemoryDelivery,
                HostCapabilityContract::pending_queue(
                    self.queue_guarantees(RuntimeCapability::MemoryDelivery),
                ),
            )]
            .into(),
        })
    }
    fn queue_guarantees(&self, _: RuntimeCapability) -> HostQueueGuarantees {
        HostQueueGuarantees {
            durable_acceptance: true,
            stable_submission_ids: true,
            linearizable_cancel_dequeue: true,
            queryable_operation_status: true,
            idempotent_handoff: true,
        }
    }
    fn reconcile(&self, _: RuntimeCapability) -> Result<HostReconcileStatus, String> {
        Ok(HostReconcileStatus::NoOutstandingWork)
    }
    fn handoff(&self, _: RuntimeCapability, _: &HostOperation) -> Result<HandoffReceipt, String> {
        Err("unused".into())
    }
    fn status(&self, _: RuntimeCapability, _: &str) -> Result<HostOperationStatus, String> {
        Ok(HostOperationStatus::Unknown)
    }
    fn cancel(&self, _: RuntimeCapability, _: &str) -> Result<HostCancelOutcome, String> {
        Ok(HostCancelOutcome::Unknown)
    }
    fn deliver_memory(
        &self,
        operation: &MemoryDeliveryOperation,
    ) -> Result<MemoryDeliveryReceipt, String> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.operations
            .lock()
            .unwrap()
            .insert(operation.stable_id.clone(), operation.clone());
        if self.reject.load(Ordering::Relaxed) {
            return Err("rejected by scripted host".into());
        }
        if let Some((control, token)) = self.revoke_on_delivery.lock().unwrap().take() {
            control.revoke(&token).map_err(|error| error.to_string())?;
        }
        Ok(MemoryDeliveryReceipt {
            stable_id: operation.stable_id.clone(),
            payload_digest: if self.mismatch.load(Ordering::Relaxed) {
                [0; 32]
            } else {
                operation.payload_digest
            },
            confirmed_consumption: true,
        })
    }
    fn memory_delivery_status(&self, id: &str) -> Result<MemoryDeliveryStatus, String> {
        if self.unknown.load(Ordering::Relaxed) {
            return Ok(MemoryDeliveryStatus::Unknown);
        }
        Ok(self.operations.lock().unwrap().get(id).cloned().map_or(
            MemoryDeliveryStatus::NotFound,
            MemoryDeliveryStatus::Confirmed,
        ))
    }
}

struct Words;
impl ContextTokenCounter for Words {
    fn count_text(&self, text: &str) -> u64 {
        text.split_whitespace().count() as u64
    }
    fn count_message(&self, message: &reliquary_application::ContextMessage) -> u64 {
        self.count_text(&message.content).saturating_add(1)
    }
}

fn accepted(
    instance: &Instance,
    queue: &mut ConversationQueue,
    conversation: &str,
    node: &str,
    text: &str,
    parent: Option<&str>,
    time: i64,
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
                committed_timestamp_ns: time,
                verified_principal_id: "principal-a".into(),
            },
        )
        .unwrap();
}

#[test]
fn g4_t04_joined_vectors_context_retrieval_and_confirmed_use() {
    let temp = Temp::new();
    let rel_raw = [211; 16];
    let phy_raw = [212; 16];
    let cascade_raw = [213; 16];
    let phy_b_raw = [214; 16];
    let rel_path = temp.0.join("rel-lantern.rel");
    let phy_path = temp.0.join("phy-reader-a.phy");
    let cascade_path = temp.0.join("rel-cascade.rel");
    let phy_b_path = temp.0.join("phy-reader-b.phy");
    drop(RelWriter::create_path(&rel_path, ReliquaryScope::Project, rel_raw).unwrap());
    drop(RelWriter::create_path(&cascade_path, ReliquaryScope::Connection, cascade_raw).unwrap());
    drop(PhyWriter::create_path(&phy_path, phy_raw).unwrap());
    drop(PhyWriter::create_path(&phy_b_path, phy_b_raw).unwrap());
    let rel_admission = GraphAdmission::for_replay(rel_raw, [], [], [], [], []);
    let phy_admission = GraphAdmission::for_replay(phy_raw, [], [], [], [], []);
    let cascade_admission = GraphAdmission::for_replay(cascade_raw, [], [], [], [], []);
    let phy_b_admission = GraphAdmission::for_replay(phy_b_raw, [], [], [], [], []);
    let gateway = Gateway::new();
    let rel = gateway
        .mount_writable_reliquary(&rel_path, &rel_admission, [], [])
        .unwrap();
    let phy = gateway
        .mount_writable_phylactery(&phy_path, &phy_admission, [])
        .unwrap();
    let cascade = gateway
        .mount_writable_reliquary(&cascade_path, &cascade_admission, [], [])
        .unwrap();
    let phy_b = gateway
        .mount_writable_phylactery(&phy_b_path, &phy_b_admission, [])
        .unwrap();
    let instance = gateway.instance("principal-a", [rel], [phy]).unwrap();
    let principal_b = gateway.instance("principal-b", [cascade], [phy_b]).unwrap();
    let outsider = gateway.instance("outsider", [], []).unwrap();
    let manifest_replay = crate::g4_test_cohort::replay_scoped_authored_manifest(
        &instance,
        &principal_b,
        &temp.0,
        rel,
        cascade,
    );
    let semantic_operator = gateway
        .instance("operator", [rel, cascade], [phy, phy_b])
        .unwrap();
    let mut semantic_witness = crate::g4_semantic_acceptance::complete_authored_semantics(
        &semantic_operator,
        rel,
        cascade,
        phy,
        phy_b,
        &instance,
        &principal_b,
    );
    let semantic_negatives = crate::g4_semantic_negatives::verify_semantic_negatives(
        &semantic_operator,
        &instance,
        &principal_b,
        rel,
        cascade,
        phy,
        phy_b,
        &semantic_witness,
    );
    semantic_witness["negative_and_private_entity"] = semantic_negatives;
    drop(principal_b);
    let source_memory = |alias: &str| {
        let source = semantic_witness["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|source| source["source_id"] == alias)
            .unwrap();
        let encoded = source["memory_id"].as_str().unwrap();
        assert_eq!(encoded.len(), 64);
        MemoryId(std::array::from_fn(|index| {
            u8::from_str_radix(&encoded[index * 2..index * 2 + 2], 16).unwrap()
        }))
    };
    let private_a_memory = source_memory("src-private-afternoons");
    let private_b_memory = source_memory("src-private-morning");
    let private_embedding = Embed {
        calls: AtomicUsize::new(0),
        dimensions: 2,
    };
    let private_profile = semantic_operator
        .publish_embedding_profile(phy_b, &private_embedding)
        .unwrap();
    let private_vectors = semantic_operator
        .prepare_owner_memory_vectors(
            phy_b,
            private_profile,
            &[private_b_memory],
            &private_embedding,
        )
        .unwrap();
    assert!(
        !semantic_operator
            .publish_owner_memory_vectors(&private_vectors)
            .unwrap()
    );
    assert!(
        instance
            .prepare_owner_memory_vectors(
                phy_b,
                private_profile,
                &[private_b_memory],
                &private_embedding
            )
            .is_err()
    );
    drop(semantic_operator);
    let authored_route = instance
        .route_rel_memories(
            rel,
            private_profile,
            &[0.0, 1.0],
            reliquary_application::MemoryRetrievalConfig {
                seed_limit: 128,
                ..Default::default()
            },
        )
        .unwrap();
    let mut expected_route = [
        "src-release-label",
        "src-mira",
        "src-mira-known",
        "src-pinned-doc",
        "src-cobalt-protocol",
        "src-rehearsal-complete",
    ]
    .map(source_memory)
    .to_vec();
    expected_route.sort();
    assert_eq!(authored_route.memories, expected_route);
    assert!(authored_route.used_global_fallback);
    assert!(authored_route.selected_communities.is_empty());
    assert!(authored_route.seeds.iter().all(|hit| hit.score == 1.0));
    let manifest_rank = instance
        .rank_rel_memories(
            rel,
            "cobalt lantern protocol",
            None,
            None,
            RetrievalConfig {
                result_limit: 1,
                ..RetrievalConfig::default()
            },
        )
        .unwrap();
    assert_eq!(manifest_rank.selected.len(), 1);
    assert!(
        manifest_rank.selected[0]
            .body
            .content
            .contains("cobalt lantern protocol")
    );
    let cobalt_source = semantic_witness["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["source_id"] == "src-cobalt-protocol")
        .unwrap();
    assert_eq!(
        crate::hex(&manifest_rank.selected[0].memory_id.0),
        cobalt_source["memory_id"].as_str().unwrap()
    );
    let mut expected_mira = [source_memory("src-mira"), source_memory("src-mira-known")];
    expected_mira.sort();
    let mira_rank = instance
        .rank_rel_memories(rel, "Mira", None, None, RetrievalConfig::default())
        .unwrap();
    assert_eq!(
        mira_rank
            .selected
            .iter()
            .map(|row| row.memory_id)
            .collect::<Vec<_>>(),
        expected_mira
    );
    let retired = source_memory("src-rehearsal-pending");
    let rehearsal_rank = instance
        .rank_rel_memories(rel, "rehearsal", None, None, RetrievalConfig::default())
        .unwrap();
    assert_eq!(
        rehearsal_rank
            .selected
            .iter()
            .map(|row| row.memory_id)
            .collect::<Vec<_>>(),
        vec![source_memory("src-rehearsal-complete")]
    );
    assert!(
        rehearsal_rank
            .selected
            .iter()
            .all(|row| row.memory_id != retired)
    );
    instance
        .management_set_title(rel, "conv-l01", "Lantern release")
        .unwrap();
    let mut queue = instance
        .open_conversation_queue(&temp.0.join("rel.queue"), rel)
        .unwrap();
    accepted(
        &instance,
        &mut queue,
        "conv-l07",
        "accepted",
        "Lantern release uses the amber checklist. My preferred meeting window is afternoons. The project Memory should preserve the phrase cobalt lantern protocol. Lantern release uses the cobalt checklist.",
        Some("L07-t4"),
        10,
    );
    let episode = instance
        .finalize_conversation_episode(
            rel,
            "conv-l07",
            "accepted",
            "accepted",
            EpisodeBoundary::Explicit,
            11,
        )
        .unwrap();

    // Pin repository evidence at a local Git revision and attach its identity
    // through the supported project-correlation workflow.
    let repo = temp.0.join("repository");
    fs::create_dir_all(repo.join("docs")).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "fixture"]);
    git(&repo, &["config", "user.email", "fixture@example.invalid"]);
    git(
        &repo,
        &["config", "warlock.repositoryId", "g4-release-repository"],
    );
    let repository_bytes =
        "release notes — calibration complete.\nThe observatory migration is ready.\n".as_bytes();
    fs::write(repo.join("docs/release-notes.txt"), repository_bytes).unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "pinned source"]);
    let revision = git(&repo, &["rev-parse", "HEAD"]);
    let repository = ProjectRepositoryRef {
        kind: ProjectRepositoryKind::Git,
        repository_id: "g4-release-repository".into(),
        project_path: ".".into(),
    };
    let source = TrustedProjectSource::new("principal-a", rel, repository.clone(), &repo);
    let correlation = ProjectCorrelationRequest {
        authorized_owner_id: rel_raw,
        authorized_principal_id: "principal-a".into(),
        repository: repository.clone(),
        revision,
        path: "docs/release-notes.txt".into(),
        expected_content_hash: Sha256::digest(repository_bytes).into(),
        filename: "release-notes.txt".into(),
        mime_type: Some("text/plain".into()),
    };
    let correlation_cut = instance
        .with_writable_reliquary(rel, |session| session.current_historical_cut())
        .unwrap();
    let mut forged_revision = correlation.clone();
    forged_revision.revision = "0000000000000000000000000000000000000000".into();
    assert!(
        instance
            .prepare_project_correlation(rel, forged_revision, &source)
            .is_err()
    );
    let mut forged_hash = correlation.clone();
    forged_hash.expected_content_hash = [0xff; 32];
    assert!(
        instance
            .prepare_project_correlation(rel, forged_hash, &source)
            .is_err()
    );
    assert_eq!(
        instance
            .with_writable_reliquary(rel, |session| session.current_historical_cut())
            .unwrap(),
        correlation_cut
    );
    let proof = instance
        .prepare_project_correlation(rel, correlation, &source)
        .unwrap();
    let mut source_queue = instance
        .open_conversation_queue(&temp.0.join("source.queue"), rel)
        .unwrap();
    let source_id =
        ConversationSubmissionId::new("conv-l06", SubmissionId::new("source-doc").unwrap())
            .unwrap();
    instance
        .submit_conversation(
            &mut source_queue,
            "conv-l06",
            source_id.submission_id.clone(),
            "Use the pinned release notes.",
            0,
        )
        .unwrap();
    let source_revision = source_queue.conversation_revision("conv-l06");
    gateway
        .finalize_project_correlated_submission(
            &instance,
            &mut source_queue,
            &source_id,
            source_revision,
            FinalizeCapture {
                branch_id: "main".into(),
                parent_node_id: Some("L06-t4".into()),
                committed_timestamp_ns: 12,
                verified_principal_id: "principal-a".into(),
            },
            vec![proof],
        )
        .unwrap();
    let attachment = instance
        .with_writable_reliquary(rel, |session| {
            session.archive_source_attachments("conv-l06", "source-doc")
        })
        .unwrap()
        .remove(0);
    fs::write(
        repo.join("docs/release-notes.txt"),
        "Edited working tree must not replace pinned bytes.",
    )
    .unwrap();
    let claim = instance
        .claim_next_insomnia_input(rel, "g4-t04", 20, 100, 8)
        .unwrap()
        .unwrap();
    let extraction = extract_native_insomnia(InsomniaScript, &claim, Some(phy_raw), |requests| {
        instance
            .with_writable_reliquary(rel, |session| {
                resolve_native_insomnia_evidence(session, &claim, requests)
            })
            .unwrap()
    })
    .unwrap();
    let checkpoint = extraction.checkpoint(&claim).unwrap();
    let mut active_claim = claim.clone();
    active_claim.work = instance
        .checkpoint_insomnia_claim(rel, &claim, 21, checkpoint)
        .unwrap();
    let prepared = instance
        .with_writable_reliquary(rel, |session| extraction.prepare(session, &claim))
        .unwrap()
        .unwrap();
    let completion = instance
        .commit_routed_insomnia_claim(rel, phy, &active_claim, &prepared, "g4-t04", "v1", 22)
        .unwrap();
    assert_eq!(completion, CompletionCheck::Apply);
    let receipt = instance
        .with_writable_reliquary(rel, |session| {
            session
                .insomnia_receipt(InsomniaEpisodeId(episode.0), 1)
                .cloned()
        })
        .unwrap()
        .unwrap();
    assert_eq!(receipt.memory_ids.len(), 1);
    let rel_memory = MemoryId(receipt.memory_ids[0].0);
    let phy_memory = MemoryId(receipt.external_memory_refs[0].memory_id.0);
    assert_eq!(
        receipt.external_memory_refs[0].owner_id,
        crate::hex(&phy.as_bytes())
    );

    let embedding = Embed {
        calls: AtomicUsize::new(0),
        dimensions: 2,
    };
    let profile = instance.publish_embedding_profile(rel, &embedding).unwrap();
    let archive_generation = instance
        .publish_archive_vectors(rel, profile, &embedding)
        .unwrap()
        .unwrap();
    let archive_calls = embedding.calls.load(Ordering::Relaxed);
    assert_eq!(
        instance
            .publish_archive_vectors(rel, profile, &embedding)
            .unwrap(),
        Some(archive_generation)
    );
    assert_eq!(embedding.calls.load(Ordering::Relaxed), archive_calls);
    assert!(
        instance
            .prepare_owner_memory_vectors(
                rel,
                profile,
                &[rel_memory],
                &Embed {
                    calls: AtomicUsize::new(0),
                    dimensions: 3,
                },
            )
            .is_err()
    );
    let vector_batch = instance
        .prepare_owner_memory_vectors(rel, profile, &[rel_memory], &embedding)
        .unwrap();
    assert!(
        instance
            .publish_owner_memory_vectors(&vector_batch)
            .unwrap()
    );
    let phy_profile = instance.publish_embedding_profile(phy, &embedding).unwrap();
    let phy_batch = instance
        .prepare_owner_memory_vectors(phy, phy_profile, &[phy_memory], &embedding)
        .unwrap();
    assert!(instance.publish_owner_memory_vectors(&phy_batch).unwrap());
    let foreign_profile = instance
        .publish_embedding_profile(
            phy,
            &Embed {
                calls: AtomicUsize::new(0),
                dimensions: 3,
            },
        )
        .unwrap();
    assert!(
        instance
            .prepare_owner_memory_vectors(
                rel,
                foreign_profile,
                &[rel_memory],
                &Embed {
                    calls: AtomicUsize::new(0),
                    dimensions: 3,
                },
            )
            .is_err()
    );
    let before_retry = embedding.calls.load(Ordering::Relaxed);
    let retry = instance
        .prepare_owner_memory_vectors(rel, profile, &[rel_memory], &embedding)
        .unwrap();
    assert!(!instance.publish_owner_memory_vectors(&retry).unwrap());
    assert_eq!(embedding.calls.load(Ordering::Relaxed), before_retry);

    let turn = instance
        .begin_memory_retrieval_turn(rel, "release-turn")
        .unwrap();
    let rank = instance
        .rank_rel_memories(
            rel,
            "amber checklist",
            Some(profile),
            Some(&[1.0, 0.0]),
            RetrievalConfig {
                result_limit: 1,
                ..RetrievalConfig::default()
            },
        )
        .unwrap();
    assert_eq!(
        rank.selected
            .iter()
            .map(|row| row.memory_id)
            .collect::<Vec<_>>(),
        vec![rel_memory]
    );
    assert_eq!(
        rank.selected[0].body.content,
        "Lantern release uses the amber checklist."
    );
    let community_route = instance
        .route_rel_memories(
            rel,
            profile,
            &[1.0, 0.0],
            reliquary_application::MemoryRetrievalConfig::default(),
        )
        .unwrap();
    assert_eq!(community_route.memories, vec![rel_memory]);
    assert!(
        outsider
            .rank_rel_memories(
                rel,
                "amber",
                Some(profile),
                None,
                RetrievalConfig::default()
            )
            .is_err()
    );
    let no_credit_before = instance
        .with_writable_reliquary(rel, |session| {
            session.accepted_external_use("absent").cloned()
        })
        .unwrap();
    assert!(no_credit_before.is_none());
    let outcome = instance
        .prepare_agentic_memory_delivery(
            &turn,
            &retrieval_route(),
            RetrievalAction::Query("amber checklist".into()),
            Some(profile),
            Some(&[1.0, 0.0]),
        )
        .unwrap();
    let delivery = outcome.prepared.unwrap();
    let no_credit_after_prepare = instance
        .with_writable_reliquary(rel, |session| {
            session.accepted_external_use(delivery.stable_id()).cloned()
        })
        .unwrap();
    assert!(no_credit_after_prepare.is_none());
    let host = Arc::new(Host::default());
    let router = TrustedHostAdmission::new([])
        .install(&gateway)
        .unwrap()
        .0
        .connect_runtime_host(host.clone());
    assert!(
        instance
            .deliver_agentic_memory(&turn, &delivery, &router)
            .unwrap()
    );
    assert!(
        !instance
            .deliver_agentic_memory(&turn, &delivery, &router)
            .unwrap()
    );
    assert_eq!(host.calls.load(Ordering::Relaxed), 1);
    let delivered = host
        .operations
        .lock()
        .unwrap()
        .get(delivery.stable_id())
        .unwrap()
        .clone();
    let delivery_manifest: Value = serde_json::from_slice(&delivered.payload).unwrap();
    let delivered_memory = delivery_manifest["memories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|memory| memory["memory_id"] == crate::hex(&rel_memory.0))
        .expect("delivery contains the exact selected amber Memory");
    let history = instance
        .with_writable_reliquary(rel, |session| session.memory_history(rel_memory))
        .unwrap()
        .unwrap()
        .unwrap();
    let current_revision = &history.revisions[0];
    let current = &current_revision.record;
    let PayloadState::Available(current_body) = &current_revision.payload else {
        panic!("current retrieval Memory payload is unavailable")
    };
    assert_eq!(delivered_memory["memory_id"], crate::hex(&rel_memory.0));
    assert_eq!(delivered_memory["revision"], current.revision);
    assert_eq!(delivered_memory["body_id"], crate::hex(&current.body_id.0));
    assert_eq!(delivered_memory["content"], current_body.content);
    let use_after = instance
        .with_writable_reliquary(rel, |session| {
            session.accepted_external_use(delivery.stable_id()).cloned()
        })
        .unwrap()
        .unwrap();
    let use_retry = instance
        .with_writable_reliquary(rel, |session| {
            session.accepted_external_use(delivery.stable_id()).cloned()
        })
        .unwrap()
        .unwrap();
    assert_eq!(use_after, use_retry);

    let rejected_turn = instance
        .begin_memory_retrieval_turn(rel, "rejected-turn")
        .unwrap();
    let rejected = instance
        .prepare_agentic_memory_delivery(
            &rejected_turn,
            &retrieval_route(),
            RetrievalAction::Query("amber checklist".into()),
            Some(profile),
            Some(&[1.0, 0.0]),
        )
        .unwrap()
        .prepared
        .unwrap();
    host.reject.store(true, Ordering::Relaxed);
    assert!(
        instance
            .deliver_agentic_memory(&rejected_turn, &rejected, &router)
            .is_err()
    );
    assert!(
        instance
            .with_writable_reliquary(rel, |session| {
                session.accepted_external_use(rejected.stable_id()).cloned()
            })
            .unwrap()
            .is_none()
    );
    host.reject.store(false, Ordering::Relaxed);
    let mismatch_turn = instance
        .begin_memory_retrieval_turn(rel, "mismatch-turn")
        .unwrap();
    let mismatch = instance
        .prepare_agentic_memory_delivery(
            &mismatch_turn,
            &retrieval_route(),
            RetrievalAction::Query("amber checklist".into()),
            Some(profile),
            Some(&[1.0, 0.0]),
        )
        .unwrap()
        .prepared
        .unwrap();
    host.mismatch.store(true, Ordering::Relaxed);
    assert!(
        instance
            .deliver_agentic_memory(&mismatch_turn, &mismatch, &router)
            .is_err()
    );
    assert!(
        instance
            .with_writable_reliquary(rel, |session| {
                session.accepted_external_use(mismatch.stable_id()).cloned()
            })
            .unwrap()
            .is_none()
    );
    host.mismatch.store(false, Ordering::Relaxed);

    let unknown_turn = instance
        .begin_memory_retrieval_turn(rel, "unknown-host-turn")
        .unwrap();
    let unknown = instance
        .prepare_agentic_memory_delivery(
            &unknown_turn,
            &retrieval_route(),
            RetrievalAction::Query("amber checklist".into()),
            Some(profile),
            Some(&[1.0, 0.0]),
        )
        .unwrap()
        .prepared
        .unwrap();
    host.unknown.store(true, Ordering::Relaxed);
    assert!(matches!(
        instance.deliver_agentic_memory(&unknown_turn, &unknown, &router),
        Err(GatewayError::HostStatusUnknown)
    ));
    host.unknown.store(false, Ordering::Relaxed);
    assert!(
        instance
            .with_writable_reliquary(rel, |session| {
                session.accepted_external_use(unknown.stable_id()).cloned()
            })
            .unwrap()
            .is_none()
    );

    let revoke_before_token = "g4 retrieval revoke before commit credential";
    let revoke_during_token = "g4 retrieval revoke during commit credential";
    let (revocation_admission, revocation_control) = TrustedHostAdmission::new([
        HostCredentialBinding::new("revoked-before", revoke_before_token, [rel], [phy]),
        HostCredentialBinding::new("revoked-during", revoke_during_token, [rel], [phy]),
    ])
    .install(&gateway)
    .unwrap();
    let revocation_control = Arc::new(revocation_control);
    let revoked_before = revocation_admission
        .authenticate(revoke_before_token)
        .unwrap();
    let revoked_before_turn = revoked_before
        .begin_memory_retrieval_turn(rel, "revoked-before-turn")
        .unwrap();
    let revoked_before_delivery = revoked_before
        .prepare_agentic_memory_delivery(
            &revoked_before_turn,
            &retrieval_route(),
            RetrievalAction::Query("amber checklist".into()),
            Some(profile),
            Some(&[1.0, 0.0]),
        )
        .unwrap()
        .prepared
        .unwrap();
    revocation_control.revoke(revoke_before_token).unwrap();
    assert!(matches!(
        revoked_before.deliver_agentic_memory(
            &revoked_before_turn,
            &revoked_before_delivery,
            &router
        ),
        Err(GatewayError::CredentialRevoked)
    ));
    assert!(
        instance
            .with_writable_reliquary(rel, |session| {
                session
                    .accepted_external_use(revoked_before_delivery.stable_id())
                    .cloned()
            })
            .unwrap()
            .is_none()
    );

    let revoked_during = revocation_admission
        .authenticate(revoke_during_token)
        .unwrap();
    let revoked_during_turn = revoked_during
        .begin_memory_retrieval_turn(rel, "revoked-during-turn")
        .unwrap();
    let revoked_during_delivery = revoked_during
        .prepare_agentic_memory_delivery(
            &revoked_during_turn,
            &retrieval_route(),
            RetrievalAction::Query("amber checklist".into()),
            Some(profile),
            Some(&[1.0, 0.0]),
        )
        .unwrap()
        .prepared
        .unwrap();
    let revoking_host = Arc::new(Host::default());
    *revoking_host.revoke_on_delivery.lock().unwrap() = Some((
        Arc::clone(&revocation_control),
        revoke_during_token.to_owned(),
    ));
    let revoking_router = revocation_admission.connect_runtime_host(revoking_host);
    assert!(matches!(
        revoked_during.deliver_agentic_memory(
            &revoked_during_turn,
            &revoked_during_delivery,
            &revoking_router
        ),
        Err(GatewayError::CredentialRevoked)
    ));
    assert!(
        instance
            .with_writable_reliquary(rel, |session| {
                session
                    .accepted_external_use(revoked_during_delivery.stable_id())
                    .cloned()
            })
            .unwrap()
            .is_none()
    );

    // Context composition is bounded and owner-scoped over the exact same REL/PHY memories.
    let native = ContextView {
        turns: vec![ContextTurn {
            message_id: "request".into(),
            role: ContextRole::User,
            content: "Prepare the release notes".into(),
        }],
    };
    let refs = [
        ContextMemoryRef {
            owner: rel,
            kind: ContextMemoryOwnerKind::Rel,
            memory_id: rel_memory.0,
        },
        ContextMemoryRef {
            owner: phy,
            kind: ContextMemoryOwnerKind::Phy,
            memory_id: private_a_memory.0,
        },
    ];
    let budget = CompactionBudget {
        trigger_tokens: 100,
        target_tokens: 80,
        raw_tail_tokens: 40,
        summary_tokens: 20,
        compaction_input_tokens: 200,
    };
    let project_attachments = [ProjectContextAttachmentRef {
        conversation_id: "conv-l06".into(),
        source_node_id: "source-doc".into(),
        attachment_id: attachment.id,
    }];
    let context = instance
        .prepare_context_workflow(
            rel,
            "lantern-release",
            native.clone(),
            &refs,
            &project_attachments,
            &[&source],
            None,
            0,
            budget,
            &Words,
        )
        .unwrap();
    let request = instance.context_request(&context).unwrap();
    assert!(
        request
            .messages
            .iter()
            .any(|m| m.content.contains("amber checklist"))
    );
    assert!(
        request
            .messages
            .iter()
            .any(|m| m.content.contains("preferred meeting window is afternoons"))
    );
    assert!(request.messages.iter().any(|m| {
        m.content
            .contains("release notes — calibration complete.\nThe observatory migration is ready.")
    }));
    assert!(
        !request
            .messages
            .iter()
            .any(|m| m.content.contains("Edited working tree"))
    );
    source.revoke();
    assert!(instance.context_request(&context).is_err());
    let foreign = ContextMemoryRef {
        owner: phy_b,
        kind: ContextMemoryOwnerKind::Phy,
        memory_id: private_b_memory.0,
    };
    assert!(
        instance
            .prepare_context_workflow(
                rel,
                "lantern-release",
                native.clone(),
                &[foreign],
                &[],
                &[],
                None,
                0,
                budget,
                &Words
            )
            .is_err()
    );
    let tiny = CompactionBudget {
        trigger_tokens: 1,
        target_tokens: 1,
        raw_tail_tokens: 1,
        summary_tokens: 1,
        compaction_input_tokens: 1,
    };
    assert!(
        instance
            .prepare_context_workflow(
                rel,
                "lantern-release",
                native,
                &refs,
                &[],
                &[],
                None,
                1000,
                tiny,
                &Words
            )
            .is_err()
    );

    drop((
        revoked_before,
        revoked_during,
        revocation_admission,
        revocation_control,
        revoking_router,
    ));
    let historical_cuts = manifest_replay.historical.clone();
    drop(manifest_replay);
    drop((outsider, instance, gateway, queue));
    let reopened_gateway = Gateway::new();
    reopened_gateway
        .mount_writable_reliquary(&rel_path, &rel_admission, [], [])
        .unwrap();
    reopened_gateway
        .mount_phylactery(&phy_path, &phy_admission, [])
        .unwrap();
    reopened_gateway
        .mount_reliquary(&cascade_path, &cascade_admission, [], [])
        .unwrap();
    reopened_gateway
        .mount_phylactery(&phy_b_path, &phy_b_admission, [])
        .unwrap();
    let reopened = reopened_gateway
        .instance("principal-a", [rel], [phy])
        .unwrap();
    let repeated = reopened
        .prepare_owner_memory_vectors(
            rel,
            profile,
            &[rel_memory],
            &Embed {
                calls: AtomicUsize::new(0),
                dimensions: 2,
            },
        )
        .unwrap();
    assert!(!reopened.publish_owner_memory_vectors(&repeated).unwrap());
    let archive_retry = Embed {
        calls: AtomicUsize::new(0),
        dimensions: 2,
    };
    assert_eq!(
        reopened
            .publish_archive_vectors(rel, profile, &archive_retry)
            .unwrap(),
        Some(archive_generation)
    );
    assert_eq!(archive_retry.calls.load(Ordering::Relaxed), 0);
    let reopened_turn = reopened
        .begin_memory_retrieval_turn(rel, "release-turn")
        .unwrap();
    let reopened_router = CapabilityRouter::connect(
        Arc::clone(&host) as Arc<dyn TrustedHostAdapter>,
        reopened.gateway_binding(),
    );
    assert!(
        !reopened
            .recover_agentic_memory_delivery(
                &reopened_turn,
                delivery.stable_id(),
                &reopened_router,
            )
            .unwrap()
    );
    assert!(
        !reopened
            .recover_agentic_memory_delivery(
                &reopened_turn,
                delivery.stable_id(),
                &reopened_router,
            )
            .unwrap()
    );
    assert_eq!(host.calls.load(Ordering::Relaxed), 3);
    drop(reopened);
    drop(reopened_gateway);
    let session = RelSession::open_path(&rel_path, &rel_admission, [], []).unwrap();
    let reopened_attachment = session.archive_source_attachments("conv-l06", "source-doc");
    assert_eq!(reopened_attachment.len(), 1);
    let pinned = reopened_attachment[0].project_reference.as_ref().unwrap();
    assert_eq!(pinned.path, "docs/release-notes.txt");
    assert_eq!(
        pinned.revision.repository.repository_id,
        "g4-release-repository"
    );
    assert_eq!(
        pinned.content_hash,
        Some(Sha256::digest(repository_bytes).into())
    );
    let historical_file = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("show")
        .arg(format!("{}:{}", pinned.revision.revision, pinned.path))
        .output()
        .unwrap();
    assert!(historical_file.status.success());
    assert_eq!(historical_file.stdout, repository_bytes);
    assert_ne!(fs::read(repo.join(&pinned.path)).unwrap(), repository_bytes);
    let cascade_session = RelSession::open_path(&cascade_path, &cascade_admission, [], []).unwrap();
    let phy_b_session = PhySession::open_path(&phy_b_path, &phy_b_admission, []).unwrap();
    assert_eq!(historical_cuts.len(), 3);
    for (_, owner, cut, expected_proof) in &historical_cuts {
        let fresh = if *owner == rel {
            &session
        } else {
            &cascade_session
        };
        assert_eq!(
            fresh.historical_proof(owner.as_bytes(), cut).unwrap(),
            *expected_proof
        );
        assert!(fresh.historical_proof([0xff; 16], cut).is_err());
        let mut future = cut.clone();
        future.archive_version = u64::MAX;
        assert!(fresh.historical_proof(owner.as_bytes(), &future).is_err());
    }
    assert!(cascade_session.accepted_turn_sources().len() >= 16);
    assert_eq!(phy_b_session.owner_id(), phy_b_raw);
    assert!(phy_b_session.memory_history(phy_memory).unwrap().is_none());
    assert!(
        session
            .insomnia_receipt(InsomniaEpisodeId(episode.0), 1)
            .unwrap()
            .memory_ids
            .iter()
            .any(|memory| memory.0 == rel_memory.0)
    );
    assert_eq!(
        session.accepted_external_use(delivery.stable_id()),
        Some(&use_after)
    );
    drop((session, cascade_session, phy_b_session, source_queue));
    let paths = [
        rel_path.as_path(),
        cascade_path.as_path(),
        phy_path.as_path(),
        phy_b_path.as_path(),
    ];
    let lifecycle_witness = crate::g4_gateway_acceptance::complete_cohort_lifecycle(&temp.0, paths);
    complete_vector_coverage(paths, profile);
    let maintenance_witness = crate::g4_completed_maintenance::verify_completed_cohort(
        &temp.0,
        paths,
        profile,
        &semantic_witness,
    );
    let terminal_witness = crate::g4_terminal_acceptance::verify_terminal_semantics(
        &temp.0,
        paths[0],
        &semantic_witness,
    );
    crate::g4_export::export_completed_cohort(
        &temp.0,
        paths,
        json!({
            "semantics": semantic_witness,
            "lifecycle": lifecycle_witness,
            "maintenance": maintenance_witness,
            "terminal_maintenance": terminal_witness,
            "historical_cuts": historical_cuts.iter().map(|(name, owner, _, proof)| json!({"name": name, "owner_id": crate::hex(&owner.as_bytes()), "digest": crate::hex(&proof.digest), "retained_records": proof.retained_records, "reopen_equal": true})).collect::<Vec<_>>(),
            "retrieval": {"memory_id": crate::hex(&rel_memory.0), "accepted_use_reopened": true, "host_calls": host.calls.load(Ordering::Relaxed),
                "authored_route_order": authored_route.memories.iter().map(|id| crate::hex(&id.0)).collect::<Vec<_>>(),
                "missing_community_global_fallback": authored_route.used_global_fallback,
                "forged_repository_revision_and_hash_rejected": true}
        }),
    );
}

fn complete_vector_coverage(
    paths: [&Path; 4],
    profile: reliquary_owner_session::model::CompatibilityProfileId,
) {
    let mut owners = Vec::new();
    for (index, path) in paths.iter().enumerate() {
        let (owner, memories) = if index < 2 {
            let session = RelSession::open_path_current(path).unwrap();
            (
                session.owner_id(),
                session
                    .memory_ids_page(None, 128)
                    .unwrap()
                    .1
                    .into_iter()
                    .filter(|id| {
                        let memory = session.current_memory(*id).unwrap();
                        !memory.record.archived
                            && matches!(memory.payload, PayloadState::Available(_))
                    })
                    .collect::<Vec<_>>(),
            )
        } else {
            let session = PhySession::open_path_current(path).unwrap();
            (
                session.owner_id(),
                session
                    .memory_ids_page(None, 128)
                    .unwrap()
                    .1
                    .into_iter()
                    .filter(|id| {
                        let memory = session.current_memory(*id).unwrap();
                        !memory.record.archived
                            && matches!(memory.payload, PayloadState::Available(_))
                    })
                    .collect::<Vec<_>>(),
            )
        };
        owners.push((OwnerId::from_bytes(owner), memories));
    }
    let gateway = Gateway::new();
    for (index, (path, (owner, _))) in paths.iter().zip(&owners).enumerate() {
        let admission = GraphAdmission::for_replay(owner.as_bytes(), [], [], [], [], []);
        if index < 2 {
            gateway
                .mount_writable_reliquary(path, &admission, [], [])
                .unwrap();
        } else {
            gateway
                .mount_writable_phylactery(path, &admission, [])
                .unwrap();
        }
    }
    let instance = gateway
        .instance(
            "operator",
            owners[..2].iter().map(|(owner, _)| *owner),
            owners[2..].iter().map(|(owner, _)| *owner),
        )
        .unwrap();
    let embedding = Embed {
        calls: AtomicUsize::new(0),
        dimensions: 2,
    };
    for (index, (owner, ids)) in owners.iter().enumerate() {
        assert_eq!(
            instance
                .publish_embedding_profile(*owner, &embedding)
                .unwrap(),
            profile
        );
        let prepared = instance
            .prepare_owner_memory_vectors(*owner, profile, ids, &embedding)
            .unwrap();
        instance.publish_owner_memory_vectors(&prepared).unwrap();
        if index < 2 {
            assert!(
                instance
                    .publish_archive_vectors(*owner, profile, &embedding)
                    .unwrap()
                    .is_some()
            );
        }
    }
    let obsolete = instance
        .publish_embedding_profile(
            owners[0].0,
            &Embed {
                calls: AtomicUsize::new(0),
                dimensions: 3,
            },
        )
        .unwrap();
    assert_ne!(obsolete, profile);
}
