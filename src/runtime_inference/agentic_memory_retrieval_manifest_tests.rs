use super::*;
use crate::MemoryId;
use std::collections::BTreeSet;
fn prepared() -> PreparedAgenticMemoryResult {
    let memory = MemoryRef {
        owner_id: "owner-a".into(),
        memory_id: MemoryId([7; 32]),
    };
    PreparedAgenticMemoryResult {
        delivery_id: "turn/tool-1".into(),
        payload: AgenticMemoryPayload {
            operation: AgenticMemoryOperation::Query,
            outcome: AgenticMemoryOutcome::Memories(vec![AgenticMemoryContent {
                memory: memory.clone(),
                title: "title".into(),
                content: "exact evidence".into(),
            }]),
            anchors: vec![],
        },
        delivered: BTreeSet::from([memory]),
        owner_cuts: BTreeMap::from([("owner-a".into(), 42)]),
    }
}
#[test]
fn agentic_manifest_is_deterministic_and_binds_every_delivery_input() {
    let original = prepared();
    let digest = original.manifest_digest();
    assert_eq!(digest.len(), 64);
    assert_eq!(original.clone().manifest_digest(), digest);
    assert_eq!(
        original.canonical_manifest()["payload"],
        original.payload().to_json()
    );
    assert_eq!(
        original.canonical_manifest()["delivered"][0]["memory_id"],
        "07".repeat(32)
    );
    let mut changed = original.clone();
    changed.delivery_id.push('x');
    assert_ne!(changed.manifest_digest(), digest);
    let mut changed = original.clone();
    changed.owner_cuts.insert("owner-a".into(), 43);
    assert_ne!(changed.manifest_digest(), digest);
    let mut changed = original.clone();
    changed.delivered.clear();
    assert_ne!(changed.manifest_digest(), digest);
    let mut changed = original;
    let AgenticMemoryOutcome::Memories(content) = &mut changed.payload.outcome else {
        panic!()
    };
    content[0].content.push('x');
    assert_ne!(changed.manifest_digest(), digest);
}
#[test]
fn agentic_manifest_fallback_contains_no_newly_delivered_content() {
    let mut value = prepared();
    value.payload.outcome = AgenticMemoryOutcome::AlreadyProvided(value.delivered.clone());
    value.delivered.clear();
    value.owner_cuts.clear();
    let payload = value.payload().to_json();
    assert_eq!(payload["outcome"]["status"], "already_provided");
    assert!(payload["outcome"]["memories"][0].get("content").is_none());
    assert_eq!(value.canonical_manifest()["delivered"], json!([]));
    value.payload.outcome = AgenticMemoryOutcome::NoMatches;
    assert_eq!(
        value.payload().to_json()["outcome"],
        json!({"status":"no_matches"})
    );
}

#[test]
fn agentic_manifest_restore_is_exact_and_rejects_mutation_and_extra_fields() {
    let original = prepared();
    let manifest = original.canonical_manifest();
    let digest = original.manifest_digest();
    assert_eq!(
        PreparedAgenticMemoryResult::restore_manifest(&manifest, &digest).unwrap(),
        original
    );
    let mut changed = manifest.clone();
    changed["payload"]["outcome"]["memories"][0]["content"] = json!("changed");
    assert!(PreparedAgenticMemoryResult::restore_manifest(&changed, &digest).is_err());
    let mut changed = manifest.clone();
    changed["unexpected"] = json!(true);
    assert!(PreparedAgenticMemoryResult::restore_manifest(&changed, &digest).is_err());
    let mut changed = manifest.clone();
    changed["delivered"][0]["memory_id"] = json!("é".repeat(32));
    assert!(PreparedAgenticMemoryResult::restore_manifest(&changed, &digest).is_err());
    let mut changed = manifest;
    changed["version"] = json!(2);
    assert!(PreparedAgenticMemoryResult::restore_manifest(&changed, &digest).is_err());
}

#[test]
fn agentic_manifest_restore_roundtrips_graph_reports_and_empty_outcomes() {
    let mut original = prepared();
    let anchor = original.delivered.iter().next().unwrap().clone();
    original.payload.operation = AgenticMemoryOperation::Traversal;
    original.payload.anchors.push(AgenticMemoryAnchorReport {
        anchor: anchor.clone(),
        unseen_neighbors: 12,
        declined_neighbors: 3,
        relations: vec![AgenticMemoryRelationEvidence {
            neighbor: anchor.clone(),
            kind: crate::GraphRelationKind::References,
            direction: crate::GraphDirection::Incoming,
        }],
        approved: original.delivered.clone(),
    });
    assert_eq!(
        PreparedAgenticMemoryResult::restore_manifest(
            &original.canonical_manifest(),
            &original.manifest_digest()
        )
        .unwrap(),
        original
    );
    original.payload.operation = AgenticMemoryOperation::Refinement;
    original.payload.outcome = AgenticMemoryOutcome::AlreadyProvided(original.delivered.clone());
    original.delivered.clear();
    original.owner_cuts.clear();
    assert_eq!(
        PreparedAgenticMemoryResult::restore_manifest(
            &original.canonical_manifest(),
            &original.manifest_digest()
        )
        .unwrap(),
        original
    );
    original.payload.outcome = AgenticMemoryOutcome::NoMatches;
    assert_eq!(
        PreparedAgenticMemoryResult::restore_manifest(
            &original.canonical_manifest(),
            &original.manifest_digest()
        )
        .unwrap(),
        original
    );
}
