use super::*;
use crate::ego_pending::EgoBatchKey;
use reliquary_ego::EgoCandidate;
use reliquary_memory_audit::OwnerCut;
use reliquary_switchboard::EmbeddingNormalization;
use serde_json::json;
use std::{
    collections::VecDeque,
    fs,
    path::Path,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

struct General {
    replies: Mutex<VecDeque<Result<Value, GeneralEndpointError>>>,
    calls: Mutex<Vec<String>>,
}

impl GeneralEndpoint for General {
    fn model(&self) -> &str {
        "scripted"
    }

    fn complete_json(
        &self,
        _system_prompt: &str,
        _payload: &str,
        name: &str,
        _schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        self.calls.lock().unwrap().push(name.into());
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .expect("planned inference call")
    }
}

#[derive(Default)]
struct Embeddings(Mutex<usize>);

impl EmbeddingEndpoint for Embeddings {
    fn dimensions(&self) -> u32 {
        2
    }

    fn normalization(&self) -> EmbeddingNormalization {
        EmbeddingNormalization::None
    }

    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        assert_eq!(mode, EmbeddingMode::Document);
        *self.0.lock().unwrap() += 1;
        Ok(vec![vec![1.0, 0.5]; inputs.len()])
    }
}

#[test]
fn rel_ego_reuses_accepted_stages_on_reopen_and_publishes_exact_quote() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    fs::create_dir_all(&directory).unwrap();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = directory.join(format!(
        "ego-rel-stage-{}-{nonce}.sqlite",
        std::process::id()
    ));
    let run = EgoRun {
        phy: [1; 16],
        source: OwnerCut {
            owner_id: [2; 16],
            history_generation: [3; 32],
            frontier: 8,
        },
        kind: EgoRunKind::RelRecurring,
        memory_version: 17,
        route: [4; 32],
        contract: 1,
    };
    let key = EgoBatchKey {
        capture: [6; 32],
        ordinal: 0,
    };
    let text = "Prefer a simple example.";
    let cite = SourceCitation {
        owner_id: run.source.owner_id,
        history_generation: run.source.history_generation,
        source: SourceId::Memory([7; 32]),
        revision: 2,
        body_id: [8; 32],
        field: TextField::Content,
        start_byte: 5,
        end_byte: 5 + text.len() as u32,
    };
    let mut pending = EgoPendingWork::open(&path).unwrap();
    let batches = [pending
        .accept_batch(
            run,
            key,
            &[EgoCandidate {
                subject: "Examples".into(),
                sources: vec![cite],
            }],
        )
        .unwrap()];
    pending.seal(run).unwrap();
    let evidence = RelEgoEvidence {
        run,
        current: BTreeMap::from([(cite, evidence(cite, 1, text.into()))]),
        historical: BTreeMap::new(),
    };
    let general = General {
        replies: Mutex::new(VecDeque::from([
            Ok(json!({"points": [{
                "statement": "Prefer a simple example.",
                "status": "requested",
                "time_scope": "current",
                "support": [{"slot": 0, "quote": "simple example"}]
            }]})),
            Err(GeneralEndpointError::HttpStatus(503)),
            Ok(json!({"checks": [{
                "point_id": 0,
                "meaning": "supported",
                "status": "supported",
                "time_scope": "supported",
                "attribution": "supported",
                "hold_reason": ""
            }]})),
        ])),
        calls: Mutex::default(),
    };
    let embedding = Embeddings::default();
    let routes = RelEgoRoutes {
        general: &general,
        embedding: &embedding,
        identity: run.route,
        embedding_profile: [5; 32],
        max_request_bytes: 32_768,
    };
    let work = || RelEgoWork {
        run,
        batches: &batches,
        prior: None,
        evidence: &evidence,
        as_of: "2026-11-27",
        verified_at_ns: 12,
    };
    assert!(matches!(
        prepare_rel_ego_publication(&routes, work(), &mut pending),
        Err(RelEgoError::General(GeneralEndpointError::HttpStatus(503)))
    ));
    drop(pending);

    let mut pending = EgoPendingWork::open(&path).unwrap();
    let publication = prepare_rel_ego_publication(&routes, work(), &mut pending).unwrap();
    assert_eq!(
        publication,
        prepare_rel_ego_publication(&routes, work(), &mut pending).unwrap()
    );
    assert_eq!(
        general.calls.lock().unwrap().as_slice(),
        &[
            "reliquary_ego_rel_synthesize",
            "reliquary_ego_rel_verify",
            "reliquary_ego_rel_verify",
        ]
    );
    assert_eq!(*embedding.0.lock().unwrap(), 2);
    assert_eq!(publication.completion.unwrap().batch_ids, vec![key.id(run)]);
    let [
        EgoRelChange::Topic { id, .. },
        EgoRelChange::Point {
            change: EgoItemChange::Put(point),
            ..
        },
    ] = publication.changes.as_slice()
    else {
        panic!("expected one topic and verified point");
    };
    assert_eq!(point.topic_id, *id);
    assert_eq!(
        point.citations,
        vec![EgoCitation {
            start_byte: 14,
            end_byte: 28,
            ..owner_citation(cite).unwrap()
        }]
    );
    pending.clear_completed(run).unwrap();
    assert!(
        pending
            .rel_stage(
                run,
                stage_key(
                    routes.embedding_profile,
                    "embed",
                    &[],
                    "",
                    "[\"Examples\"]",
                    ""
                )
            )
            .unwrap()
            .is_none()
    );
    drop(pending);
    fs::remove_file(path).unwrap();
}
