use super::*;
use crate::EgoRoutes;
#[cfg(test)]
use reliquary_memory_core::MemoryStore;
use reliquary_owner_session::{EgoCitation, EgoCompletion, EgoPhySummary, EgoTextField};

const PROFILE: &str = "Meetings use video.";
const PROFILE_EVIDENCE: &str = "I use video for meetings.";
const PROJECT_EVIDENCE: &str = "Atlas reviews purchase requests before releasing payment.";
const IDENTITY: &str = "Respond with practical precision.";
const PIN: &str = "Always use metric units.";

struct EgoScript;

impl GeneralEndpoint for EgoScript {
    fn model(&self) -> &str {
        "ego-mounted-fixture"
    }

    fn complete_json(
        &self,
        _: &str,
        payload: &str,
        schema: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        let payload: Value = serde_json::from_str(payload).unwrap();
        let supported = json!({"judgment": "supported", "hold_reason": ""});
        Ok(match schema {
            "memory_web_audit_v3" => {
                let source =
                    payload["sources"].as_array().unwrap().iter().find(|row| {
                        row["text"] == PROJECT_EVIDENCE || row["text"] == PROFILE_EVIDENCE
                    });
                let candidates = source
                    .into_iter()
                    .map(|source| {
                        let subject = if source["text"] == PROJECT_EVIDENCE {
                            "Atlas payments"
                        } else {
                            "Meetings"
                        };
                        json!({"subject": subject, "source_slots": [source["slot"]]})
                    })
                    .collect::<Vec<_>>();
                json!({
                    "ego": {"candidates": candidates, "lookups": []},
                    "perception": {"candidates": [], "lookups": []},
                })
            }
            "reliquary_ego_rel_synthesize" => json!({"points": [{
                "statement": BASE, "status": "observed", "time_scope": "current",
                "support": [{"slot": 0, "quote": PROJECT_EVIDENCE}],
            }]}),
            "reliquary_ego_rel_verify" => json!({"checks": [{
                "point_id": 0, "meaning": "supported", "status": "supported",
                "time_scope": "supported", "attribution": "supported", "hold_reason": "",
            }]}),
            "ego_phy_draft" => {
                let paragraphs = payload["memories"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|row| row["text"] == PROFILE_EVIDENCE)
                    .into_iter()
                    .map(|source| {
                        json!({
                            "text": PROFILE,
                            "support": [{"slot": source["slot"], "quote": PROFILE_EVIDENCE}],
                        })
                    })
                    .collect::<Vec<_>>();
                json!({"paragraphs": paragraphs})
            }
            "ego_phy_verify" => {
                let checks = payload["final_summary"]["paragraphs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|paragraph| {
                        json!({
                            "paragraph_id": paragraph["paragraph_id"], "meaning": "supported",
                            "time": "supported", "scope": "supported", "attribution": "supported",
                            "hold_reason": "",
                        })
                    })
                    .collect::<Vec<_>>();
                json!({"checks": checks, "continuity": supported})
            }
            _ => {
                return Err(GeneralEndpointError::InvalidResponse(
                    "unexpected Ego fixture request",
                ));
            }
        })
    }
}

#[test]
fn managed_ego_publishes_and_reopens_both_summary_forms() {
    let fixture = Fixture::new();
    let project_source = fixture
        .append(&[("Atlas".into(), PROJECT_EVIDENCE)], 0)
        .remove(0);
    let phy_path = fixture.path.with_extension("phy");
    drop(PhyWriter::create_path(&phy_path, AUDIT_PHY).unwrap());
    let personal_source = {
        let mut container = Container::open_writer(&phy_path).unwrap();
        publish_accepted_phy_memory(&mut container, 0)
    };
    let profile_dir = fixture.path.with_extension("sqlite");
    fs::create_dir(&profile_dir).unwrap();
    let gateway = Gateway::new();
    let rel = gateway
        .mount_writable_reliquary_current(&fixture.path)
        .unwrap();
    let phy = gateway
        .mount_writable_phylactery_current(&phy_path)
        .unwrap();
    let instance = Arc::new(gateway.instance("alice", [rel], [phy]).unwrap());
    instance
        .create_ego_pin(EgoScope::Phy(phy), PIN.into(), EgoAnchorPriority::High)
        .unwrap();
    let identity = instance
        .create_ego_identity(phy, "Fixture Identity".into(), IDENTITY.into())
        .unwrap();
    instance
        .select_ego_identity(phy, 0, Some((identity.id, identity.revision)))
        .unwrap();
    let mut worker = gateway
        .start_ego_worker(
            instance.clone(),
            phy,
            vec![rel],
            &profile_dir,
            EgoRoutes {
                general: Arc::new(EgoScript),
                embedding: Some(Arc::new(Script {
                    phase: 0,
                    calls: Arc::new(Mutex::new(Vec::new())),
                })),
                identity: [4; 32],
                embedding_profile: [5; 32],
                count_tokens: Arc::new(|s| s.len() as u64),
            },
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match instance.ego_context(Some(rel), phy, "Atlas payment", None, 500, str::len) {
            Ok(assembled) => {
                if assembled.standing_context.contains(BASE)
                    && assembled.standing_context.contains(PROFILE)
                {
                    assert_eq!(assembled.identity.as_deref(), Some(IDENTITY));
                    assert!(assembled.standing_context.contains(PIN));
                    break;
                }
            }
            Err(GatewayError::StaleRelContext) => {}
            Err(error) => panic!("Ego context assembly failed: {error}"),
        }
        assert!(
            Instant::now() < deadline,
            "the mounted summaries did not finish"
        );
        thread::sleep(Duration::from_millis(40));
    }
    worker.stop().unwrap();
    drop(instance);
    drop(gateway);
    let reopened = PhySession::open_path_current(&phy_path).unwrap();
    let personal = reopened.ego_phy_summary().unwrap();
    let project = reopened.ego_rel_summary(rel.0).unwrap();
    let personal_text = personal.text.as_ref().unwrap();
    let point = &project.points.values().next().unwrap().value;
    assert_eq!(personal_text.text, PROFILE);
    assert_eq!(point.statement, BASE);
    assert_reopened_proof(
        &reopened,
        &personal.completion,
        phy,
        &personal_text.paragraphs[0].citations,
        &personal_source,
        PROFILE_EVIDENCE,
    );
    assert_reopened_proof(
        &reopened,
        project.completion.as_ref().unwrap(),
        rel,
        &point.citations,
        &project_source,
        PROJECT_EVIDENCE,
    );
    let file = profile_dir.join("ego-pending.sqlite3");
    let pending = EgoPendingWork::open(&file).unwrap();
    for owner in [rel, phy] {
        assert!(pending.ready_run(phy.0, owner.0).unwrap().is_none());
        assert!(pending.pending_runs(phy.0, owner.0).unwrap().is_empty());
    }
    drop(pending);
    fs::remove_file(file).unwrap();
    if let Some(destination) = std::env::var_os("RELIQUARY_EGO_CAPTURE_FIXTURE") {
        let destination = PathBuf::from(destination);
        fs::create_dir(&destination).unwrap();
        fs::copy(&fixture.path, destination.join("atlas.rel")).unwrap();
        fs::copy(&phy_path, destination.join("personal.phy")).unwrap();
    }
}

#[test]
fn mounted_ego_phy_refresh_waits_for_five_distinct_accepted_memories() {
    let fixture = Fixture::new();
    let phy_path = fixture.path.with_extension("phy");
    drop(PhyWriter::create_path(&phy_path, AUDIT_PHY).unwrap());
    {
        let mut container = Container::open_writer(&phy_path).unwrap();
        publish_accepted_phy_memory(&mut container, 0);
    }
    let profile_dir = fixture.path.with_extension("sqlite");
    fs::create_dir(&profile_dir).unwrap();
    let (gateway, instance, phy, mut worker) = start_mounted_phy_worker(&phy_path, &profile_dir);
    wait_for_phy_revision(&instance, phy, 1);
    worker.stop().unwrap();
    drop(instance);
    drop(gateway);

    {
        let mut container = Container::open_writer(&phy_path).unwrap();
        for index in 1..=4 {
            publish_accepted_phy_memory(&mut container, index);
        }
    }
    let (gateway, instance, phy, mut worker) = start_mounted_phy_worker(&phy_path, &profile_dir);
    wait_for_phy_waiting_checkpoint(&profile_dir, phy);
    assert_eq!(
        instance
            .inspect_ego_phy_summary(phy, None)
            .unwrap()
            .unwrap()
            .revision,
        1
    );
    worker.stop().unwrap();
    drop(instance);
    drop(gateway);

    {
        let mut container = Container::open_writer(&phy_path).unwrap();
        publish_accepted_phy_memory(&mut container, 5);
    }
    let (gateway, instance, phy, mut worker) = start_mounted_phy_worker(&phy_path, &profile_dir);
    let summary = wait_for_phy_revision(&instance, phy, 2);
    assert_eq!(summary.completion.cut.memory_version, 6);
    assert_eq!(summary.text.unwrap().text, PROFILE);
    worker.stop().unwrap();
    drop(instance);
    drop(gateway);
    fs::remove_file(profile_dir.join("ego-pending.sqlite3")).unwrap();
}

fn publish_accepted_phy_memory(container: &mut Container, index: u64) -> MemoryRecord {
    publish_phy_evidence(container, index, PROFILE_EVIDENCE)
}

fn publish_phy_evidence(container: &mut Container, index: u64, evidence: &str) -> MemoryRecord {
    MemoryStore::open(container)
        .unwrap()
        .publish_versioned_creation(
            container,
            MemoryDraft {
                title: format!("Meetings evidence {index}"),
                content: evidence.into(),
                category: "preference".into(),
                memory_type: "communication".into(),
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
                source_ref: None,
                source_time_ns: Some(index as i64 + 5),
                temporal_inference: None,
                mutation_id: format!("mounted-ego-accepted-phy-{index}"),
                created_at_ns: index as i64 + 5,
                updated_at_ns: index as i64 + 5,
            },
            index as i64 + 5,
        )
        .unwrap()
        .revision
        .record
}

struct SharedPhyScript {
    perception: Arc<Script>,
    requested: Mutex<Vec<(bool, bool)>>,
}

impl GeneralEndpoint for SharedPhyScript {
    fn model(&self) -> &str {
        "shared-phy-audit-fixture"
    }

    fn complete_json(
        &self,
        instruction: &str,
        payload: &str,
        schema: &str,
        output: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        if schema != "memory_web_audit_v3" {
            return if schema.starts_with("ego_phy_") {
                EgoScript.complete_json(instruction, payload, schema, output)
            } else {
                self.perception
                    .complete_json(instruction, payload, schema, output)
            };
        }
        let input: Value = serde_json::from_str(payload).unwrap();
        let active = (
            input["active"]["ego"].as_bool().unwrap(),
            input["active"]["perception"].as_bool().unwrap(),
        );
        let mut requested = self.requested.lock().unwrap();
        let first = requested.is_empty();
        requested.push(active);
        let mut reply = self
            .perception
            .complete_json(instruction, payload, schema, output)?;
        if first {
            assert_eq!(active, (true, true));
            reply["ego"] = json!({"candidates":"malformed","lookups":[]});
        } else if active.0 {
            reply["ego"] =
                EgoScript.complete_json(instruction, payload, schema, output)?["ego"].clone();
        }
        reply["perception"] = if active.1 {
            reply["perception"].clone()
        } else {
            json!({"candidates":[],"lookups":[]})
        };
        Ok(reply)
    }
}

#[test]
fn shared_phy_audit_retries_only_the_unaccepted_list_then_reopens_both_results() {
    let fixture = Fixture::new();
    let phy_path = fixture.path.with_extension("phy");
    drop(PhyWriter::create_path(&phy_path, AUDIT_PHY).unwrap());
    {
        let mut writer = Container::open_writer(&phy_path).unwrap();
        publish_accepted_phy_memory(&mut writer, 0);
        publish_phy_evidence(&mut writer, 1, BASE);
    }
    let profile = fixture.path.with_extension("sqlite");
    fs::create_dir(&profile).unwrap();
    let pending_path = profile.join("ego-pending.sqlite3");
    let gateway = Gateway::new();
    let phy = gateway
        .mount_writable_phylactery_current(&phy_path)
        .unwrap();
    let instance = Arc::new(gateway.instance("alice", [], [phy]).unwrap());
    let lifecycle = gateway.perception_lifecycle();
    let endpoint = Arc::new(Script {
        phase: 1,
        calls: Arc::new(Mutex::new(Vec::new())),
    });
    let discovery = Arc::new(SharedPhyScript {
        perception: endpoint.clone(),
        requested: Mutex::new(Vec::new()),
    });
    let route = [33; 32];
    lifecycle
        .set_routes(Some(PerceptionRoutes {
            general: discovery.clone(),
            contribution: endpoint.clone(),
            verification: endpoint.clone(),
            embedding: endpoint.clone(),
            identity: route,
            embedding_identity: [34; 32],
        }))
        .unwrap();
    // Register the receiver before starting the scan to make its startup deterministic.
    let registration = gateway
        .ego_audit_registry
        .admit(&instance, phy, &[], &pending_path, route)
        .unwrap();
    let mut perception = gateway
        .start_phy_perception(&lifecycle, instance.clone(), phy)
        .unwrap();
    let mut ego = gateway
        .start_ego_worker_with_perception(
            instance.clone(),
            phy,
            vec![],
            &profile,
            EgoRoutes {
                general: discovery.clone(),
                embedding: None,
                identity: route,
                embedding_profile: [34; 32],
                count_tokens: Arc::new(|text| text.len() as u64),
            },
            Some(&lifecycle),
        )
        .unwrap();
    let summary = wait_for_phy_revision(&instance, phy, 1);
    assert_eq!(summary.text.unwrap().text, PROFILE);
    assert!(
        discovery
            .requested
            .lock()
            .unwrap()
            .starts_with(&[(true, true), (true, false)])
    );
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        let ready = instance
            .with_ego_phy_writer(phy, |writer| {
                Ok(writer.observations_after(None, 16).iter().any(|head| {
                    head.standing == ObservationStanding::Current
                        && head.revision.draft.proposition == A
                }))
            })
            .unwrap();
        if ready {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "shared discovery did not publish the accepted PHY Observation"
        );
        thread::sleep(Duration::from_millis(40));
    }
    assert_eq!(
        endpoint
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| matches!(call, Call::Synthesized(text) if text == A))
            .count(),
        1
    );
    ego.stop().unwrap();
    perception.stop().unwrap();
    drop(registration);
    drop(instance);
    drop(gateway);
    let reopened = PhySession::open_path_current(&phy_path).unwrap();
    assert_eq!(
        reopened
            .ego_phy_summary()
            .unwrap()
            .text
            .as_ref()
            .unwrap()
            .text,
        PROFILE
    );
    assert_eq!(
        reopened
            .observations_after(None, 16)
            .iter()
            .filter(|head| head.standing == ObservationStanding::Current
                && head.revision.draft.proposition == A)
            .count(),
        1
    );
}

fn wait_for_phy_revision(instance: &Instance, phy: OwnerId, revision: u64) -> EgoPhySummary {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(summary) = instance
            .inspect_ego_phy_summary(phy, None)
            .unwrap()
            .filter(|summary| summary.revision == revision)
        {
            return summary;
        }
        assert!(
            Instant::now() < deadline,
            "Ego PHY revision {revision} did not publish"
        );
        thread::sleep(Duration::from_millis(40));
    }
}

fn wait_for_phy_waiting_checkpoint(profile_dir: &Path, phy: OwnerId) {
    let deadline = Instant::now() + Duration::from_secs(20);
    let pending = EgoPendingWork::open(&profile_dir.join("ego-pending.sqlite3")).unwrap();
    loop {
        let waiting = pending
            .open_checkpoint_run(phy.0, phy.0)
            .unwrap()
            .and_then(|run| pending.checkpoint::<Value>(run).unwrap())
            .is_some_and(|checkpoint| checkpoint["phase"] == "Waiting");
        if waiting {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Ego PHY refresh did not reach its stored Waiting checkpoint"
        );
        thread::sleep(Duration::from_millis(40));
    }
}

fn start_mounted_phy_worker(
    phy_path: &Path,
    profile_dir: &Path,
) -> (Gateway, Arc<Instance>, OwnerId, EgoWorkerHandle) {
    let gateway = Gateway::new();
    let phy = gateway.mount_writable_phylactery_current(phy_path).unwrap();
    let instance = Arc::new(gateway.instance("alice", [], [phy]).unwrap());
    let worker = gateway
        .start_ego_worker(
            instance.clone(),
            phy,
            vec![],
            profile_dir,
            EgoRoutes {
                general: Arc::new(EgoScript),
                embedding: None,
                identity: [4; 32],
                embedding_profile: [5; 32],
                count_tokens: Arc::new(|text| text.len() as u64),
            },
        )
        .unwrap();
    (gateway, instance, phy, worker)
}

fn assert_reopened_proof(
    reopened: &PhySession,
    completion: &EgoCompletion,
    owner: OwnerId,
    citations: &[EgoCitation],
    source: &MemoryRecord,
    text: &str,
) {
    assert_eq!(completion.cut.owner_id, owner.0);
    assert_eq!(
        citations,
        [EgoCitation {
            owner_id: owner.0,
            history_generation: completion.cut.history_generation,
            memory_id: source.id.0,
            revision: source.revision,
            body_id: source.body_id.0,
            field: EgoTextField::Content,
            start_byte: 0,
            end_byte: text.len() as u32,
        }]
    );
    assert_eq!(completion.batch_ids.len(), 1);
    assert_eq!(
        reopened.ego_completion_for_batch(completion.batch_ids[0]),
        Some(completion)
    );
}

#[test]
fn managed_ego_refreshes_read_only_rel_without_perception_routes() {
    use reliquary_owner_session::{AUDIT_CADENCE, EgoCompletion, EgoRelPublication, EgoSourceCut};
    let fixture = Fixture::new();
    fixture.append(&[("First Atlas evidence".into(), PROJECT_EVIDENCE)], 0);
    let prior = RelSession::open_path_current(&fixture.path)
        .unwrap()
        .perception_source_cut();
    let phy_path = fixture.path.with_extension("phy");
    {
        let mut phy = PhyWriter::create_path(&phy_path, AUDIT_PHY).unwrap();
        phy.publish_ego_rel_summary(EgoRelPublication {
            source_owner_id: prior.owner_id,
            expected_revision: 0,
            completion: Some(EgoCompletion {
                id: [14; 32],
                cut: EgoSourceCut {
                    owner_id: prior.owner_id,
                    history_generation: prior.history_generation,
                    memory_version: prior.memory,
                    audit_frontier: prior.settlements,
                },
                batch_ids: vec![],
            }),
            changes: vec![],
        })
        .unwrap();
    }
    let additional: Vec<_> = (0..AUDIT_CADENCE)
        .map(|index| (format!("Later Atlas evidence {index:02}"), PROJECT_EVIDENCE))
        .collect();
    fixture.append(&additional, 0);
    {
        let source = RelSession::open_path_current(&fixture.path).unwrap();
        let cut = source.perception_source_cut();
        let mut after_memory = prior.memory;
        while after_memory < cut.memory {
            let (next, ids) = source
                .perception_recurring_changed_ids_page(&cut, after_memory, prior.settlements, 1)
                .unwrap();
            assert!(ids.is_empty(), "newly settled sources were already paged");
            assert!(
                next > after_memory,
                "skipped changes must still advance the cursor"
            );
            after_memory = next;
        }
    }

    let profile_dir = fixture.path.with_extension("sqlite");
    fs::create_dir(&profile_dir).unwrap();
    let gateway = Gateway::new();
    let rel = gateway.mount_reliquary_current(&fixture.path).unwrap();
    let phy = gateway
        .mount_writable_phylactery_current(&phy_path)
        .unwrap();
    let instance = Arc::new(gateway.instance("alice", [rel], [phy]).unwrap());
    let perception = gateway.perception_lifecycle();
    perception.admit(&instance, vec![rel]).unwrap();
    let mut worker = gateway
        .start_ego_worker_with_perception(
            instance.clone(),
            phy,
            vec![rel],
            &profile_dir,
            EgoRoutes {
                general: Arc::new(EgoScript),
                embedding: Some(Arc::new(Script {
                    phase: 0,
                    calls: Arc::new(Mutex::new(Vec::new())),
                })),
                identity: [4; 32],
                embedding_profile: [5; 32],
                count_tokens: Arc::new(|text| text.len() as u64),
            },
            Some(&perception),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let summary = instance
            .inspect_ego_rel_summary(phy, rel, None)
            .unwrap()
            .unwrap();
        if summary.revision == 2 {
            assert_eq!(summary.points[0].statement.as_deref(), Some(BASE));
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Ego-only recurring REL audit did not publish"
        );
        thread::sleep(Duration::from_millis(40));
    }
    worker.stop().unwrap();
    drop(perception);
    drop(instance);
    drop(gateway);
    let reopened = PhySession::open_path_current(&phy_path).unwrap();
    let done = reopened
        .ego_rel_summary(rel.0)
        .unwrap()
        .completion
        .as_ref()
        .unwrap();
    assert_eq!(
        done.cut.audit_frontier,
        prior.settlements + AUDIT_CADENCE as u64
    );
    assert!(!done.batch_ids.is_empty());
    let source = RelSession::open_path_current(&fixture.path).unwrap();
    assert!(source.observation_progress().is_none());
    fs::remove_file(profile_dir.join("ego-pending.sqlite3")).unwrap();
}
