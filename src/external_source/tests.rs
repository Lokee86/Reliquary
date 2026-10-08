use super::*;
use crate::{Cva, EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingMode, VectorNormalization};
use std::sync::Mutex;

pub(crate) struct Endpoint;
impl EmbeddingEndpoint for Endpoint {
    fn dimensions(&self) -> u32 {
        2
    }
    fn normalization(&self) -> VectorNormalization {
        VectorNormalization::L2
    }
    fn embed(
        &self,
        _: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        Ok(inputs.iter().map(|_| vec![0.6, 0.8]).collect())
    }
}
#[derive(Default)]
struct HostState {
    watermark: u64,
    floor: u64,
    messages: Vec<HydratedExternalMessage>,
    changes: Vec<ExternalConversationChange>,
    denied: bool,
}
pub(crate) struct Host {
    state: Mutex<HostState>,
}
impl Host {
    pub(crate) fn new(text: &str) -> Self {
        Self {
            state: Mutex::new(HostState {
                watermark: 1,
                floor: 1,
                messages: vec![message(text)],
                ..Default::default()
            }),
        }
    }
    fn change(&self, kind: ExternalChangeKind) {
        let mut s = self.state.lock().unwrap();
        s.watermark += 1;
        let sequence = s.watermark;
        s.changes
            .push(ExternalConversationChange { sequence, kind });
    }
}
fn message(text: &str) -> HydratedExternalMessage {
    HydratedExternalMessage {
        descriptor: ExternalMessageDescriptor {
            reference: ExternalMessageRef {
                source_namespace: "host:test".into(),
                conversation_id: "c".into(),
                message_id: "m".into(),
                content_hash: external_content_hash(text),
            },
            role: "user".into(),
            principal_id: Some("alice".into()),
            timestamp_ns: 42,
            active: true,
        },
        content: text.into(),
    }
}
impl ExternalConversationSource for Host {
    fn source_namespace(&self) -> &str {
        "host:test"
    }
    fn authorize(&self, p: &str) -> Result<(), ExternalSourceError> {
        if p != "alice" || self.state.lock().unwrap().denied {
            Err(ExternalSourceError::Unauthorized)
        } else {
            Ok(())
        }
    }
    fn changes_after(
        &self,
        p: &str,
        cursor: u64,
        limit: usize,
    ) -> Result<ExternalChangeBatch, ExternalSourceError> {
        self.authorize(p)?;
        let s = self.state.lock().unwrap();
        Ok(ExternalChangeBatch {
            feed_floor: s.floor,
            feed_watermark: s.watermark,
            changes: s
                .changes
                .iter()
                .filter(|c| c.sequence > cursor)
                .take(limit)
                .cloned()
                .collect(),
        })
    }
    fn snapshot_manifest(&self, p: &str) -> Result<ExternalSnapshotManifest, ExternalSourceError> {
        self.authorize(p)?;
        let s = self.state.lock().unwrap();
        Ok(ExternalSnapshotManifest {
            source_namespace: self.source_namespace().into(),
            contract_version: EXTERNAL_SOURCE_CONTRACT,
            watermark: s.watermark,
            messages: s.messages.iter().map(|m| m.descriptor.clone()).collect(),
        })
    }
    fn hydrate(
        &self,
        p: &str,
        refs: &[ExternalMessageDescriptor],
    ) -> Result<Vec<HydratedExternalMessage>, ExternalSourceError> {
        self.authorize(p)?;
        let s = self.state.lock().unwrap();
        refs.iter()
            .map(|r| {
                s.messages
                    .iter()
                    .find(|m| m.descriptor == *r)
                    .cloned()
                    .ok_or(ExternalSourceError::StaleEvidence)
            })
            .collect()
    }
}
fn path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("external-{}.cva", uuid::Uuid::new_v4()))
}
fn rebuild(cva: &mut Cva, host: &Host) -> ExternalIndexStatus {
    let profile = crate::compatibility_profile_probe::profile_from_endpoint(&Endpoint).unwrap();
    let old = cva
        .external_index
        .sources
        .get(host.source_namespace())
        .cloned();
    let prepared = index::prepare_rebuild(host, "alice", &Endpoint, profile, old.as_ref()).unwrap();
    cva.external_install(prepared).unwrap()
}
fn update(cva: &mut Cva, host: &Host) -> Result<ExternalIndexStatus, ExternalSourceError> {
    let old = cva.external_index_state(host.source_namespace())?.unwrap();
    let profile = cva.external_profile(&old)?;
    cva.external_install(index::prepare_incremental(
        host, "alice", &Endpoint, profile, old,
    )?)
}
fn hits(cva: &mut Cva) -> Vec<ExternalSearchHit> {
    let state = cva.external_index_state("host:test").unwrap().unwrap();
    cva.external_search(&state, None, "secret", 10).unwrap()
}

#[test]
fn reference_index_reopens_replays_and_never_persists_host_body() {
    let body = "secret canonical transcript fixture 98f147c5 never locally archived";
    let host = Host::new(body);
    let path = path();
    let mut cva = Cva::create(&path).unwrap();
    let status = rebuild(&mut cva, &host);
    assert_eq!(status.cursor, 1);
    assert_eq!(hits(&mut cva).len(), 1);
    let before = cva.container.latest_version();
    assert_eq!(update(&mut cva, &host).unwrap(), status);
    assert_eq!(before, cva.container.latest_version());
    drop(cva);
    assert!(
        !std::fs::read(&path)
            .unwrap()
            .windows(body.len())
            .any(|w| w == body.as_bytes())
    );
    let mut cva = Cva::open(&path).unwrap();
    assert_eq!(
        cva.external_index_state("host:test")
            .unwrap()
            .unwrap()
            .status(),
        status
    );
    assert_eq!(hits(&mut cva).len(), 1);
}

#[test]
fn edit_visibility_reactivation_and_delete_replace_only_source_rows() {
    let host = Host::new("secret old");
    let mut cva = Cva::create(path()).unwrap();
    rebuild(&mut cva, &host);
    let old = hits(&mut cva)[0].reference.message.clone();
    let edited = message("secret replacement");
    host.state.lock().unwrap().messages = vec![edited.clone()];
    host.change(ExternalChangeKind::Upsert(edited.descriptor.clone()));
    update(&mut cva, &host).unwrap();
    assert_ne!(hits(&mut cva)[0].reference.message, old);
    host.state.lock().unwrap().messages[0].descriptor.active = false;
    host.change(ExternalChangeKind::Visibility {
        conversation_id: "c".into(),
        message_ids: vec!["m".into()],
        active: false,
    });
    update(&mut cva, &host).unwrap();
    assert!(hits(&mut cva).is_empty());
    host.state.lock().unwrap().messages[0].descriptor.active = true;
    host.change(ExternalChangeKind::Visibility {
        conversation_id: "c".into(),
        message_ids: vec!["m".into()],
        active: true,
    });
    update(&mut cva, &host).unwrap();
    assert_eq!(hits(&mut cva).len(), 1);
    host.state.lock().unwrap().messages.clear();
    host.change(ExternalChangeKind::DeleteConversation {
        conversation_id: "c".into(),
    });
    update(&mut cva, &host).unwrap();
    assert!(hits(&mut cva).is_empty());
}

#[test]
fn lost_metadata_rebuilds_at_manifest_cut_and_feed_gap_stops_incremental() {
    let host = Host::new("secret");
    let mut cva = Cva::create(path()).unwrap();
    rebuild(&mut cva, &host);
    host.state.lock().unwrap().floor = 3;
    assert_eq!(
        update(&mut cva, &host),
        Err(ExternalSourceError::RebuildRequired)
    );
    assert_eq!(hits(&mut cva).len(), 1);
    cva.external_index.sources.clear();
    cva.external_index.damaged = true;
    rebuild(&mut cva, &host);
    assert_eq!(hits(&mut cva).len(), 1);
}

#[test]
fn forged_hash_bad_namespace_and_noncontiguous_feed_leave_cursor_unchanged() {
    let host = Host::new("secret");
    let mut cva = Cva::create(path()).unwrap();
    rebuild(&mut cva, &host);
    host.state.lock().unwrap().messages[0].content = "tampered".into();
    assert!(
        index::prepare_rebuild(
            &host,
            "alice",
            &Endpoint,
            cva.external_profile(&cva.external_index.sources["host:test"])
                .unwrap(),
            None
        )
        .is_err()
    );
    host.state
        .lock()
        .unwrap()
        .changes
        .push(ExternalConversationChange {
            sequence: 3,
            kind: ExternalChangeKind::DeleteConversation {
                conversation_id: "c".into(),
            },
        });
    host.state.lock().unwrap().watermark = 3;
    assert_eq!(
        update(&mut cva, &host),
        Err(ExternalSourceError::InvalidContract)
    );
    assert_eq!(cva.external_index.sources["host:test"].cursor, 1);
    let mut reference = message("secret").descriptor.reference;
    reference.source_namespace.clear();
    assert_eq!(
        reference.validate(),
        Err(ExternalSourceError::InvalidContract)
    );
}

#[test]
fn snapshot_corruption_is_rebuild_required_and_stale_preparation_cannot_overwrite() {
    let host = Host::new("secret");
    let mut cva = Cva::create(path()).unwrap();
    rebuild(&mut cva, &host);
    let old = cva.external_index.sources["host:test"].clone();
    let profile = cva.external_profile(&old).unwrap();
    let prepared = index::prepare_rebuild(&host, "alice", &Endpoint, profile, Some(&old)).unwrap();
    rebuild(&mut cva, &host);
    assert_eq!(
        cva.external_install(prepared),
        Err(ExternalSourceError::Conflict)
    );
    let mut bytes = store::encode(&old).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    let mut reopened = store::ExternalIndexStore::default();
    reopened.ingest(&bytes);
    assert!(reopened.damaged);
    assert!(reopened.sources.is_empty());
}

fn prepared_memory(window: [u8; 32], body: &str) -> processing::PreparedExternalMemory {
    processing::PreparedExternalMemory {
        draft: crate::MemoryDraft {
            category: "knowledge".into(),
            memory_type: "fact".into(),
            authority_kind: "direct".into(),
            temporal_status: "current".into(),
            title: "Derived decision".into(),
            content: body.into(),
            scope: "project".into(),
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
            source_time_ns: Some(42),
            mutation_id: format!("external-test-{}", window[0]),
            created_at_ns: 42,
            updated_at_ns: 43,
        },
        temporal: None,
        routing: None,
        evidence: ExternalMemoryEvidence {
            source_owner_id: "rel-source".into(),
            principal_id: "alice".into(),
            processing_window: window,
            authority: ExternalChunkRef {
                message: message("secret").descriptor.reference,
                start: 0,
                end: 6,
            },
            content: None,
            grounding: None,
        },
    }
}
#[test]
fn external_publication_receipt_provenance_and_global_clock_reopen_atomically() {
    let path = path();
    let mut cva = Cva::create(&path).unwrap();
    let window = [7; 32];
    let prepared = vec![prepared_memory(
        window,
        "A distinct derived semantic proposition.",
    )];
    let memories = cva.publish_external_window(window, &prepared).unwrap();
    let id = memories[0].id;
    assert!(memories[0].source_episode_id.is_none());
    assert!(memories[0].source_node_id.is_none());
    assert!(memories[0].source_ref.is_none());
    let before = cva.container.latest_version();
    let evidence = cva.external_memory_evidence(id).unwrap().unwrap();
    assert_eq!(evidence.processing_window, window);
    let different = vec![prepared_memory(
        window,
        "A retried model produced different wording.",
    )];
    assert_eq!(
        cva.publish_external_window(window, &different).unwrap()[0].content,
        memories[0].content
    );
    assert_eq!(before, cva.container.latest_version());
    drop(cva);
    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.container.latest_version(), before);
    assert_eq!(reopened.memory(id).unwrap().content, memories[0].content);
    assert_eq!(
        reopened.external_memory_evidence(id).unwrap(),
        Some(evidence)
    );
    assert_eq!(
        reopened
            .publish_external_window(window, &different)
            .unwrap()
            .len(),
        1
    );
    assert!(reopened.episodes().is_empty());
}

#[test]
fn corrupt_publication_does_not_claim_embedded_versions_and_empty_receipt_reopens() {
    let path = path();
    let mut cva = Cva::create(&path).unwrap();
    let before = cva.container.latest_version();
    cva.publish_external_window([9; 32], &[]).unwrap();
    assert_eq!(cva.container.latest_version(), before);
    drop(cva);
    let mut cva = Cva::open(&path).unwrap();
    assert!(
        cva.memories
            .external_window(&mut cva.container, [9; 32])
            .unwrap()
            .is_some()
    );
    let wire = publication::Publication {
        window: [1; 32],
        global_start: 1,
        transaction_ns: 42,
        birth_turn: None,
        records: vec![],
        bodies: vec![],
        bindings: vec![],
        routing: vec![],
    };
    let mut bytes = publication::encode(&wire).unwrap();
    bytes[16] ^= 1;
    assert!(publication::decode_embedded_version_range(&bytes).is_err());
}

#[test]
fn hydration_hashes_utf8_bytes_and_rejects_missing_or_inactive_evidence() {
    let mut hydrated = message("secret café 🦀");
    assert!(hydrated.validate(&hydrated.descriptor).is_ok());
    hydrated.content.push('!');
    assert_eq!(
        hydrated.validate(&hydrated.descriptor),
        Err(ExternalSourceError::StaleEvidence)
    );
    let mut inactive = message("secret");
    inactive.descriptor.active = false;
    assert!(inactive.validate(&inactive.descriptor).is_err());
}

pub(crate) struct SemanticEndpoint {
    pub(crate) calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}
impl crate::GeneralEndpoint for SemanticEndpoint {
    fn model(&self) -> &str {
        "external-shared-insomnia"
    }
    fn complete_json(
        &self,
        _: &str,
        payload: &str,
        schema: &str,
        _: &serde_json::Value,
    ) -> Result<serde_json::Value, crate::GeneralEndpointError> {
        use serde_json::json;
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let p: serde_json::Value = serde_json::from_str(payload).unwrap();
        match schema {
            "insomnia_authority_disposition_ledger" => {
                assert_eq!(p["source_mode"], "external-derived");
                assert!(p.get("episode_id").is_none());
                Ok(json!({"turns":{"m":[
                    {"disposition":"retain","authority_kind":"direct","category":"fact","type":"project",
                     "lifecycle":"current","proposition":"The project uses Rust.","authority_source_node_id":"",
                     "grounding_source_node_id":"","reason":"Durable project decision."},
                    {"disposition":"retain","authority_kind":"direct","category":"preference","type":"communication",
                     "lifecycle":"current","proposition":"Alice prefers concise replies.","authority_source_node_id":"",
                     "grounding_source_node_id":"","reason":"Durable personal preference."}
                ]},"evidence_requests":[]}))
            }
            "insomnia_memory_ownership" => {
                Ok(json!({"groups":{"g000":{"ownership":"project"},"g001":{"ownership":"user"}}}))
            }
            "insomnia_memory_wording" => Ok(json!({"groups":{
                "g000":{"title":"Implementation language","content":"The project uses Rust."},
                "g001":{"title":"Reply preference","content":"Alice prefers concise replies."}
            }})),
            other => Err(crate::GeneralEndpointError::Failure(format!(
                "unexpected schema {other}"
            ))),
        }
    }
}
#[test]
fn shared_insomnia_derives_project_and_user_memories_from_transient_evidence() {
    let host = Host::new("The project uses Rust. Alice prefers concise replies.");
    let mut cva = Cva::create(path()).unwrap();
    rebuild(&mut cva, &host);
    let state = cva.external_index.sources["host:test"].clone();
    let refs = vec![state.messages[0].descriptor.reference.clone()];
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let endpoint = || SemanticEndpoint {
        calls: calls.clone(),
    };
    let extractor = crate::InsomniaExtractor::new(endpoint()).with_ownership_endpoint(endpoint());
    let prepared = processing::prepare(
        "rel-source",
        "alice",
        "project",
        state,
        &refs,
        &host,
        &extractor,
        100,
    )
    .unwrap();
    assert_eq!(prepared.project.len(), 1);
    assert_eq!(prepared.user.len(), 1);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
    let project = cva
        .publish_external_window(prepared.window, &prepared.project)
        .unwrap();
    let phy_path = std::env::temp_dir().join(format!("external-{}.phy", uuid::Uuid::new_v4()));
    let mut phy = crate::Phylactery::create(&phy_path).unwrap();
    let user = publication::publish(
        &mut phy.container,
        &mut phy.memories,
        prepared.window,
        &prepared.user,
        None,
    )
    .unwrap();
    assert_eq!(project[0].source_time_ns, Some(42));
    assert_eq!(user[0].source_time_ns, Some(42));
    assert!(project[0].source_episode_id.is_none());
    assert!(user[0].source_ref.is_none());
    assert_eq!(
        cva.external_memory_evidence(project[0].id)
            .unwrap()
            .unwrap()
            .authority
            .message,
        refs[0]
    );
    assert_eq!(
        phy.external_memory_evidence(user[0].id)
            .unwrap()
            .unwrap()
            .authority
            .message,
        refs[0]
    );
    assert!(cva.episodes().is_empty());
    drop(phy);
    let phy = crate::Phylactery::open(&phy_path).unwrap();
    assert!(phy.external_memory_evidence(user[0].id).unwrap().is_some());
}

impl Host {
    pub(crate) fn refs(&self) -> Vec<ExternalMessageRef> {
        self.state
            .lock()
            .unwrap()
            .messages
            .iter()
            .map(|m| m.descriptor.reference.clone())
            .collect()
    }
    pub(crate) fn deny(&self) {
        self.state.lock().unwrap().denied = true;
    }
}

#[test]
fn corrupt_logical_index_reopens_without_losing_semantics_and_rebuild_persists() {
    let path = path();
    let host = Host::new("secret canonical evidence");
    let mut cva = Cva::create(&path).unwrap();
    rebuild(&mut cva, &host);
    let window = [19; 32];
    let mut prepared = prepared_memory(window, "An independent semantic decision.");
    prepared.evidence.authority.message = host.refs()[0].clone();
    let memory = cva
        .publish_external_window(window, &[prepared])
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let old = cva.external_index.sources["host:test"].clone();
    let mut bytes = store::encode(&old).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    // Keep physical Container framing valid; only the rebuildable index envelope is damaged.
    cva.container.append(&bytes).unwrap();
    cva.container.sync().unwrap();
    drop(cva);
    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(
        reopened.external_index_state("host:test"),
        Err(ExternalSourceError::RebuildRequired)
    );
    assert_eq!(reopened.memory(memory.id).unwrap().content, memory.content);
    assert!(
        reopened
            .external_memory_evidence(memory.id)
            .unwrap()
            .is_some()
    );
    let rebuilt = rebuild(&mut reopened, &host);
    assert_eq!(hits(&mut reopened).len(), 1);
    drop(reopened);
    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(
        reopened
            .external_index_state("host:test")
            .unwrap()
            .unwrap()
            .status(),
        rebuilt
    );
    assert_eq!(hits(&mut reopened).len(), 1);
    assert_eq!(reopened.memory(memory.id).unwrap().content, memory.content);
}
#[test]
fn interrupted_preparation_reopens_old_cursor_and_retries_exactly_once() {
    let path = path();
    let host = Host::new("secret original");
    let mut cva = Cva::create(&path).unwrap();
    let original = rebuild(&mut cva, &host);
    let old_ref = hits(&mut cva)[0].reference.clone();
    let edited = message("secret replacement");
    host.state.lock().unwrap().messages = vec![edited.clone()];
    host.change(ExternalChangeKind::Upsert(edited.descriptor));
    let old = cva.external_index.sources["host:test"].clone();
    let profile = cva.external_profile(&old).unwrap();
    let prepared = index::prepare_incremental(&host, "alice", &Endpoint, profile, old).unwrap();
    assert_eq!(prepared.state.cursor, 2);
    assert_eq!(cva.external_index.sources["host:test"].status(), original);
    drop(prepared);
    drop(cva);
    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(
        reopened.external_index.sources["host:test"].status(),
        original
    );
    assert_eq!(hits(&mut reopened)[0].reference, old_ref);
    let updated = update(&mut reopened, &host).unwrap();
    assert_eq!(updated.cursor, 2);
    assert_ne!(hits(&mut reopened)[0].reference, old_ref);
    assert_eq!(update(&mut reopened, &host).unwrap(), updated);
    drop(reopened);
    let reopened = Cva::open(&path).unwrap();
    assert_eq!(
        reopened.external_index.sources["host:test"].status(),
        updated
    );
}
