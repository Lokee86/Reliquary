use super::*;
use crate::{
    FinalizeCapture, GraphAdmission, MountedOwner, PhySession, PhyWriter, RelSession, RelWriter,
    SubmissionId,
};
use reliquary_application::{
    NativeInsomniaExtraction, extract_native_insomnia, prepare_routed_user_proofs,
    resolve_native_insomnia_evidence,
};
use reliquary_container::ReliquaryScope;
use reliquary_owner_session::model::{EpisodeBoundary, InsomniaEpisodeId, MemoryId};
use serde_json::json;

struct Endpoint;

impl GeneralEndpoint for Endpoint {
    fn model(&self) -> &str {
        "v01-fixture"
    }

    fn complete_json(
        &self,
        _prompt: &str,
        payload: &str,
        schema_name: &str,
        _schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        match schema_name {
            "insomnia_authority_disposition_ledger" => {
                let requests = if payload.contains("read_only_archive_evidence") {
                    assert!(payload.contains("Private project notes are the designated records."));
                    json!([])
                } else {
                    json!([{"kind":"turn","conversation_id":"reference","node_id":"prior",
                        "start_node_id":"","end_node_id":"","query":"","limit":1}])
                };
                Ok(json!({"turns":{"accepted":[
                    {"disposition":"retain","authority_kind":"direct","category":"preference","type":"communication",
                    "lifecycle":"current","proposition":"The user prefers concise answers.",
                    "authority_source_node_id":"","grounding_source_node_id":"","reason":"direct assertion"},
                    {"disposition":"retain","authority_kind":"direct","category":"instruction","type":"project",
                    "lifecycle":"current","proposition":"Keep the project notes private.",
                    "authority_source_node_id":"","grounding_source_node_id":"prior","reason":"durable project instruction"}
                ]},"evidence_requests":requests}))
            }
            "insomnia_memory_ownership" => {
                Ok(json!({"ownership":{"g000":"user","g001":"project"}}))
            }
            "insomnia_memory_wording" => Ok(json!({"candidates":[
                {"authority_kind":"direct","category":"preference","memory_type":"communication","temporal_status":"current",
                "ownership":"user","title":"Concise answers","content":"The user prefers concise answers.",
                "source_node_id":"accepted","source_quote":"I prefer concise answers.","authority_source":null,
                "grounding_source":null,"routing_mentions":[]},
                {"authority_kind":"direct","category":"instruction","memory_type":"project","temporal_status":"current",
                "ownership":"project","title":"Private project notes","content":"Keep the project notes private.",
                "source_node_id":"accepted","source_quote":"Keep those project notes private.","authority_source":null,
                "grounding_source":{"conversation_id":"reference","node_id":"prior","quote":"Private project notes are the designated records."},"routing_mentions":[]}
            ]})),
            _ => panic!("unexpected endpoint call {schema_name}"),
        }
    }
}

struct NoInference;

impl GeneralEndpoint for NoInference {
    fn model(&self) -> &str {
        panic!("retry must reuse the synced extraction checkpoint")
    }

    fn complete_json(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        panic!("retry must reuse the synced extraction checkpoint")
    }
}

#[test]
fn native_mixed_route_reopens_and_reuses_phy_write_after_interruption() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).unwrap();
    let rel_path = root.join("owner.rel");
    let phy_path = root.join("owner.phy");
    let rel_id = [81; 16];
    let phy_id = [82; 16];
    let rel_admission = GraphAdmission::for_replay(rel_id, [], [], [], [], []);
    let phy_admission = GraphAdmission::for_replay(phy_id, [], [], [], [], []);
    drop(RelWriter::create_path(&rel_path, ReliquaryScope::Project, rel_id).unwrap());
    drop(PhyWriter::create_path(&phy_path, phy_id).unwrap());

    let gateway = Gateway::new();
    let owner = gateway
        .mount_writable_reliquary(&rel_path, &rel_admission, [], [])
        .unwrap();
    let phy_owner = gateway
        .mount_writable_phylactery(&phy_path, &phy_admission, [])
        .unwrap();
    let instance = gateway.instance("alice", [owner], [phy_owner]).unwrap();
    let mut queue = instance
        .open_conversation_queue(&root.join("queue"), owner)
        .unwrap();
    for (conversation, node, text, timestamp) in [
        (
            "reference",
            "prior",
            "Private project notes are the designated records.",
            10,
        ),
        (
            "native",
            "accepted",
            "I prefer concise answers. Keep those project notes private.",
            20,
        ),
    ] {
        let submission = SubmissionId::new(node).unwrap();
        let id = crate::ConversationSubmissionId::new(conversation, submission.clone()).unwrap();
        let revision = queue.conversation_revision(conversation);
        instance
            .submit_conversation(&mut queue, conversation, submission, text, revision)
            .unwrap();
        let revision = queue.conversation_revision(conversation);
        instance
            .finalize_conversation_submission(
                &mut queue,
                &id,
                revision,
                FinalizeCapture {
                    branch_id: "main".into(),
                    parent_node_id: None,
                    committed_timestamp_ns: i64::try_from(timestamp).unwrap(),
                    verified_principal_id: "alice".into(),
                },
            )
            .unwrap();
    }
    let episode = instance
        .finalize_conversation_episode(
            owner,
            "native",
            "accepted",
            "accepted",
            EpisodeBoundary::Explicit,
            21,
        )
        .unwrap();
    let claim = instance
        .claim_next_insomnia_input(owner, "interrupted", 30, 100, 8)
        .unwrap()
        .unwrap();
    let extracted = extract_native_insomnia(
        SharedEndpoint(Arc::new(Endpoint)),
        &claim,
        Some(phy_id),
        |requests| {
            instance
                .with_writable_reliquary(owner, |rel| {
                    resolve_native_insomnia_evidence(rel, &claim, requests)
                })
                .unwrap()
        },
    )
    .unwrap();
    let checkpoint = extracted.checkpoint(&claim).unwrap();
    assert!(
        NativeInsomniaExtraction::reopen_checkpoint(&claim, Some([99; 16]), &checkpoint).is_err()
    );
    let mut active_claim = claim.clone();
    active_claim.work = instance
        .checkpoint_insomnia_claim(owner, &claim, 31, checkpoint)
        .unwrap();
    let prepared = instance
        .with_writable_reliquary(owner, |rel| extracted.prepare(rel, &claim))
        .unwrap()
        .unwrap();
    assert_eq!(prepared.user_memories().count(), 1);
    assert_eq!(prepared.project_memories().count(), 1);
    let untouched_phy = std::fs::metadata(&phy_path).unwrap().len();
    let no_phy_grant = gateway.instance("alice", [owner], []).unwrap();
    assert!(matches!(
        no_phy_grant.commit_routed_insomnia_claim(
            owner,
            phy_owner,
            &active_claim,
            &prepared,
            "v01-fixture",
            "v3-4",
            32
        ),
        Err(GatewayError::UnauthorizedPhy { .. })
    ));
    let wrong_principal = gateway.instance("bob", [owner], [phy_owner]).unwrap();
    assert!(matches!(
        wrong_principal.commit_routed_insomnia_claim(
            owner,
            phy_owner,
            &active_claim,
            &prepared,
            "v01-fixture",
            "v3-4",
            32
        ),
        Err(GatewayError::RoutedMemory(_))
    ));
    assert!(matches!(
        instance.commit_routed_insomnia_claim(
            owner,
            phy_owner,
            &active_claim,
            &prepared,
            "v01-fixture",
            "v3-4",
            130
        ),
        Err(GatewayError::OwnerSession(_))
    ));
    assert_eq!(std::fs::metadata(&phy_path).unwrap().len(), untouched_phy);
    let phy_memory = {
        let owners = gateway.owners.read().unwrap();
        let MountedOwner::PhyWriter(mount) = owners.get(&phy_owner).unwrap() else {
            panic!("missing PHY writer")
        };
        let mut writer = mount.writer.lock().unwrap();
        let mut proofs = instance
            .with_writable_reliquary(owner, |rel| {
                prepare_routed_user_proofs(rel, &writer, &prepared, "alice")
            })
            .unwrap()
            .unwrap();
        writer
            .publish_routed_user_memory(proofs.pop().unwrap())
            .unwrap()
            .0
            .record
            .id
    };

    // Simulate process interruption immediately after the durable PHY write.
    drop(no_phy_grant);
    drop(wrong_principal);
    drop(queue);
    drop(instance);
    drop(gateway);
    let rel = RelSession::open_path(&rel_path, &rel_admission, [], []).unwrap();
    assert!(
        rel.insomnia_receipt(InsomniaEpisodeId(episode.0), 1)
            .is_none()
    );
    drop(rel);

    let gateway = Gateway::new();
    let owner = gateway
        .mount_writable_reliquary(&rel_path, &rel_admission, [], [])
        .unwrap();
    let phy_owner = gateway
        .mount_writable_phylactery(&phy_path, &phy_admission, [])
        .unwrap();
    let instance = Arc::new(gateway.instance("alice", [owner], [phy_owner]).unwrap());
    let mut worker = gateway
        .start_insomnia_workers(
            instance.clone(),
            owner,
            Arc::new(NoInference),
            InsomniaWorkerConfig {
                destination_phy_owner: Some(phy_owner),
                idle_poll: Duration::from_millis(5),
                ..Default::default()
            },
        )
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !instance
        .with_writable_reliquary(owner, |rel| {
            rel.insomnia_receipt(InsomniaEpisodeId(episode.0), 2)
                .is_some()
        })
        .unwrap()
        && std::time::Instant::now() < deadline
    {
        thread::sleep(Duration::from_millis(5));
    }
    worker.stop().unwrap();
    drop(worker);
    drop(instance);
    drop(gateway);

    let rel = RelSession::open_path(&rel_path, &rel_admission, [], []).unwrap();
    let receipt = rel
        .insomnia_receipt(InsomniaEpisodeId(episode.0), 2)
        .unwrap();
    assert_eq!(receipt.memory_ids.len(), 1);
    assert_eq!(receipt.external_memory_refs.len(), 1);
    assert_eq!(receipt.external_memory_refs[0].memory_id.0, phy_memory.0);
    assert_eq!(
        receipt.external_memory_refs[0].owner_id,
        crate::hex(&phy_id)
    );
    let local = rel
        .memory_history(MemoryId(receipt.memory_ids[0].0))
        .unwrap()
        .unwrap();
    assert_eq!(
        local.revisions[0]
            .record
            .grounding_source_node_id
            .as_deref(),
        Some("prior")
    );
    let phy = PhySession::open_path(&phy_path, &phy_admission, []).unwrap();
    let history = phy.memory_history(phy_memory).unwrap().unwrap();
    assert_eq!(history.revisions.len(), 1);
    let record = &history.revisions[0].record;
    assert!(record.source_episode_id.is_none() && record.source_node_id.is_none());
    assert_eq!(
        record.source_ref.as_ref().unwrap().source_episode_id,
        episode.0
    );
    assert_eq!(record.source_time_ns, Some(20));
    drop(phy);
    drop(rel);
    std::fs::remove_file(rel_path).unwrap();
    std::fs::remove_file(phy_path).unwrap();
}
