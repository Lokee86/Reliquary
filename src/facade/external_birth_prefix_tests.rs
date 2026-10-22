use crate::external_source::processing::PreparedExternalMemory;
use crate::external_source::{ExternalChunkRef, ExternalMemoryEvidence, ExternalMessageRef};
use crate::memory_external_publication::{self as publication, Publication};
use crate::{Cva, MemoryDraft};
use serde_json::json;
use sha2::Digest;

fn path(name: &str) -> std::path::PathBuf {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    std::fs::create_dir_all(&root).unwrap();
    root.join(format!("{name}-{}.rel", uuid::Uuid::new_v4()))
}

pub(super) fn prepared(window: [u8; 32], ordinal: u8) -> PreparedExternalMemory {
    let message = ExternalMessageRef {
        source_namespace: "external:test".into(),
        conversation_id: "conversation".into(),
        message_id: format!("message-{ordinal}"),
        content_hash: [ordinal; 32],
    };
    PreparedExternalMemory {
        draft: MemoryDraft {
            category: "knowledge".into(),
            memory_type: "fact".into(),
            authority_kind: "direct".into(),
            temporal_status: "current".into(),
            title: format!("External {ordinal}"),
            content: format!("Accepted external proposition {ordinal}."),
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
            source_time_ns: Some(100 + i64::from(ordinal)),
            mutation_id: format!("external-birth-{ordinal}"),
            created_at_ns: 100 + i64::from(ordinal),
            updated_at_ns: 100 + i64::from(ordinal),
        },
        temporal: None,
        routing: None,
        evidence: ExternalMemoryEvidence {
            source_owner_id: "source-owner".into(),
            principal_id: "principal".into(),
            processing_window: window,
            authority: ExternalChunkRef {
                message,
                start: 0,
                end: 1,
            },
            content: None,
            grounding: None,
        },
    }
}

fn cvaxmem1(publication: &Publication) -> Vec<u8> {
    let records = publication
        .records
        .iter()
        .map(|record| {
            json!({
                "global": record.global_version,
                "memory": record.memory_version,
                "bytes": crate::memory_codec::encode_record(record).unwrap(),
            })
        })
        .collect::<Vec<_>>();
    let bodies = publication
        .bodies
        .iter()
        .map(|body| json!({ "id": body.id.0, "bytes": body.bytes }))
        .collect::<Vec<_>>();
    let routing = publication
        .routing
        .iter()
        .map(|metadata| crate::memory_routing_codec::encode(metadata).unwrap())
        .collect::<Vec<_>>();
    let wire = json!({
        "window": publication.window,
        "global_start": publication.global_start,
        "transaction_ns": publication.transaction_ns,
        "birth_turn": publication.birth_turn,
        "records": records,
        "bodies": bodies,
        "bindings": publication.bindings,
        "routing": routing,
    });
    let json = serde_json::to_vec(&wire).unwrap();
    let mut frame = b"CVAXMEM1".to_vec();
    frame.extend_from_slice(&(json.len() as u64).to_le_bytes());
    frame.extend_from_slice(&publication.global_start.to_le_bytes());
    frame.extend_from_slice(&(publication.records.len() as u32).to_le_bytes());
    frame.extend_from_slice(&publication.transaction_ns.to_le_bytes());
    frame.extend_from_slice(&json);
    let digest = sha2::Sha256::digest(&frame);
    frame.extend_from_slice(&digest);
    frame
}

fn clone_publication(publication: &Publication) -> Publication {
    Publication {
        window: publication.window,
        global_start: publication.global_start,
        transaction_ns: publication.transaction_ns,
        birth_turn: publication.birth_turn,
        records: publication.records.clone(),
        bodies: publication.bodies.clone(),
        bindings: publication.bindings.clone(),
        routing: publication.routing.clone(),
    }
}

#[test]
fn external_birth_prefix_accepts_both_formats_and_filters_members_by_memory_cut() {
    let path = path("external-birth-prefix");
    let mut rel = Cva::create(&path).unwrap();
    let window = [31; 32];
    let memories = rel
        .publish_external_window(window, &[prepared(window, 1), prepared(window, 2)])
        .unwrap();
    assert_eq!(memories.len(), 2);
    let v2 = rel
        .container
        .chunks()
        .unwrap()
        .into_iter()
        .map(|chunk| rel.container.read(chunk).unwrap())
        .find(|payload| payload.starts_with(b"CVAXMEM2"))
        .unwrap();
    let decoded = publication::decode(&v2).unwrap().unwrap();

    let full = rel
        .external_birth_cohort_at_memory_prefix(&v2, rel.memory_version())
        .unwrap()
        .unwrap();
    assert_eq!(full.members.len(), 2);
    assert_eq!(
        full.members
            .iter()
            .map(|member| member.id)
            .collect::<Vec<_>>(),
        memories.iter().map(|memory| memory.id).collect::<Vec<_>>()
    );
    let partial = rel
        .external_birth_cohort_at_memory_prefix(&v2, 1)
        .unwrap()
        .unwrap();
    assert_eq!(partial.members.len(), 1);
    assert_eq!(partial.members[0].id, memories[0].id);
    assert!(
        rel.external_birth_cohort_at_memory_prefix(&v2, 0)
            .unwrap()
            .unwrap()
            .members
            .is_empty()
    );
    assert!(
        rel.external_birth_cohort_at_memory_prefix(&v2, rel.memory_version() + 1)
            .is_err()
    );

    let v1 = cvaxmem1(&decoded);
    assert!(v1.starts_with(b"CVAXMEM1"));
    assert_eq!(
        rel.external_birth_cohort_at_memory_prefix(&v1, rel.memory_version())
            .unwrap(),
        None
    );

    let mut no_proof = clone_publication(&decoded);
    no_proof.birth_turn = None;
    let no_proof = publication::encode(&no_proof).unwrap();
    assert!(
        rel.external_birth_cohort_at_memory_prefix(&no_proof, rel.memory_version())
            .unwrap()
            .is_none()
    );
    assert!(
        rel.external_birth_cohort_at_memory_prefix(&no_proof, rel.memory_version() + 1)
            .is_err()
    );

    let mut wrong_activity = clone_publication(&decoded);
    wrong_activity.birth_turn = Some(decoded.birth_turn.unwrap() + 1);
    let wrong_activity = publication::encode(&wrong_activity).unwrap();
    assert!(
        rel.external_birth_cohort_at_memory_prefix(&wrong_activity, rel.memory_version())
            .is_err()
    );

    let mut wrong_mutation = clone_publication(&decoded);
    wrong_mutation.records[0].mutation_id = "different-first-mutation".into();
    let wrong_mutation = publication::encode(&wrong_mutation).unwrap();
    assert!(
        rel.external_birth_cohort_at_memory_prefix(&wrong_mutation, rel.memory_version())
            .is_err()
    );

    let mut malformed = v2;
    let final_byte = malformed.len() - 1;
    malformed[final_byte] ^= 1;
    assert!(
        rel.external_birth_cohort_at_memory_prefix(&malformed, rel.memory_version())
            .is_err()
    );
}
