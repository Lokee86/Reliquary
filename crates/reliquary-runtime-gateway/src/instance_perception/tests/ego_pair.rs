use super::*;
use crate::EgoRoutes;
#[cfg(test)]
use reliquary_memory_core::MemoryStore;

const PROFILE: &str = "Meetings use video.";

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
                let source = payload["sources"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|row| row["text"] == BASE || row["text"] == PROFILE);
                let candidates = source
                    .into_iter()
                    .map(|source| {
                        let subject = if source["text"] == BASE {
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
                "support": [{"slot": 0, "quote": BASE}],
            }]}),
            "reliquary_ego_rel_verify" => json!({"checks": [{
                "point_id": 0, "meaning": "supported", "status": "supported",
                "time_scope": "supported", "attribution": "supported", "hold_reason": "",
            }]}),
            "ego_phy_draft" => json!({"paragraphs": [{
                "text": PROFILE, "support": [{"slot": 0, "quote": PROFILE}],
            }]}),
            "ego_phy_verify" => json!({
                "checks": [{"paragraph_id": 0, "meaning": "supported", "time": "supported",
                    "scope": "supported", "attribution": "supported", "hold_reason": ""}],
                "continuity": supported,
            }),
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
    fixture.append(&[("Atlas".into(), BASE)], 0);
    let phy_path = fixture.path.with_extension("phy");
    drop(PhyWriter::create_path(&phy_path, AUDIT_PHY).unwrap());
    {
        let mut container = Container::open_writer(&phy_path).unwrap();
        MemoryStore::open(&container)
            .unwrap()
            .publish_versioned_creation(
                &mut container,
                MemoryDraft {
                    title: "Meetings".into(),
                    content: PROFILE.into(),
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
                    source_time_ns: Some(5),
                    temporal_inference: None,
                    mutation_id: "mounted-ego-accepted-phy".into(),
                    created_at_ns: 5,
                    updated_at_ns: 5,
                },
                5,
            )
            .unwrap();
    }
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
        let assembled = instance
            .ego_context(Some(rel), phy, "Atlas payment", None, 500, str::len)
            .unwrap();
        if assembled.standing_context.contains(BASE) && assembled.standing_context.contains(PROFILE)
        {
            break;
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
    assert_eq!(personal.text.as_ref().unwrap().text, PROFILE);
    assert_eq!(
        project.points.values().next().unwrap().value.statement,
        BASE
    );
    let file = profile_dir.join("ego-pending.sqlite3");
    let pending = EgoPendingWork::open(&file).unwrap();
    for owner in [rel, phy] {
        assert!(pending.ready_run(phy.0, owner.0).unwrap().is_none());
        assert!(pending.pending_runs(phy.0, owner.0).unwrap().is_empty());
    }
    drop(pending);
    fs::remove_file(file).unwrap();
}
