//! G4 semantic denial and provenance cases over the completed cohort owners.

#[cfg(test)]
use super::*;
use reliquary_application::{EntityMemoryMention, TextField, extract_native_insomnia};
use reliquary_owner_session::model::{
    EpisodeBoundary, InsomniaEpisodeId, MemoryEntityResolutionStatus, MemoryId,
};
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use serde_json::{Value, json};

const G4_AUTHORED_MANIFEST: &str = include_str!("../../../tests/fixtures/g4-native-rel21-v1.json");

#[derive(Clone)]
struct ForgedQuoteEndpoint {
    source_node_id: String,
    user_turn_ids: Vec<String>,
}

impl GeneralEndpoint for ForgedQuoteEndpoint {
    fn model(&self) -> &str {
        "g4-forged-quote"
    }

    fn complete_json(
        &self,
        _: &str,
        _: &str,
        schema_name: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        match schema_name {
            "insomnia_authority_disposition_ledger" => {
                let clause = json!({
                    "disposition": "retain",
                    "authority_kind": "direct",
                    "category": "fact",
                    "type": "project",
                    "lifecycle": "current",
                    "proposition": "The authored source supports this test proposal.",
                    "authority_source_node_id": "",
                    "grounding_source_node_id": "",
                    "reason": "direct source assertion"
                });
                let turns = self
                    .user_turn_ids
                    .iter()
                    .map(|id| {
                        let clauses = if id == &self.source_node_id {
                            json!([clause.clone()])
                        } else {
                            json!([])
                        };
                        (id.clone(), clauses)
                    })
                    .collect::<serde_json::Map<_, _>>();
                Ok(json!({"turns":turns,"evidence_requests":[]}))
            }
            "insomnia_memory_wording" => Ok(json!({
                "candidates": [{
                    "authority_kind": "direct",
                    "category": "fact",
                    "memory_type": "project",
                    "temporal_status": "current",
                    "ownership": "project",
                    "title": "Forged source quote",
                    "content": "This proposal has no source authority.",
                    "source_node_id": self.source_node_id.as_str(),
                    "source_quote": "this quote does not occur in the accepted source",
                    "authority_source": null,
                    "grounding_source": null,
                    "routing_mentions": []
                }]
            })),
            other => panic!("unexpected forged-quote endpoint call: {other}"),
        }
    }
}

struct PrivateEntityEndpoint;

impl GeneralEndpoint for PrivateEntityEndpoint {
    fn model(&self) -> &str {
        "g4-private-entity"
    }

    fn complete_json(
        &self,
        _: &str,
        _: &str,
        schema_name: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        match schema_name {
            "entity_resolution" => Ok(json!({
                "decision": "create_new",
                "reason": "named_referent",
                "target_candidate_index": -1,
                "identity_relation": "related_distinct",
                "mention_kind": "application"
            })),
            "entity_admission" => Ok(json!({
                "decision": "create_new",
                "reason": "first_seen_identity",
                "promotion_policy": "immediate",
                "entity_kind": "application",
                "entity_summary": "A private preference mention from the accepted source."
            })),
            other => panic!("unexpected private-Entity endpoint call: {other}"),
        }
    }
}

pub(crate) fn verify_semantic_negatives(
    operator: &Instance,
    principal_a: &Instance,
    principal_b: &Instance,
    lantern: OwnerId,
    cascade: OwnerId,
    phy_a: OwnerId,
    phy_b: OwnerId,
    witness: &Value,
) -> Value {
    let manifest: Value = serde_json::from_str(G4_AUTHORED_MANIFEST).unwrap();
    let memory_id = |item: &Value| MemoryId(parse_hex::<32>(item["memory_id"].as_str().unwrap()));

    let memory_version_before = principal_a
        .with_writable_reliquary(lantern, |rel| rel.memory_version())
        .unwrap();
    let owner_cut_before = principal_a
        .with_writable_reliquary(lantern, |rel| rel.current_historical_cut())
        .unwrap();
    assert!(
        principal_a
            .finalize_conversation_episode(
                lantern,
                "conv-l02",
                "g4-missing-source-start",
                "g4-missing-source-end",
                EpisodeBoundary::Explicit,
                1_000_000,
            )
            .is_err()
    );
    assert_eq!(
        principal_a
            .with_writable_reliquary(lantern, |rel| rel.memory_version())
            .unwrap(),
        memory_version_before,
        "an invalid Episode boundary must not mutate Memory"
    );
    assert_eq!(
        principal_a
            .with_writable_reliquary(lantern, |rel| rel.current_historical_cut())
            .unwrap(),
        owner_cut_before
    );

    let mut claim = principal_a
        .claim_next_insomnia_input(lantern, "g4-semantic-negative", 2_000_000, 1_000_000, 8)
        .unwrap();
    if claim.is_none() {
        // L02-t1 is an accepted authored turn but is not one of the semantic
        // source references. It gives the rejection case fresh durable work.
        principal_a
            .finalize_conversation_episode(
                lantern,
                "conv-l02",
                "L02-t1",
                "L02-t1",
                EpisodeBoundary::Explicit,
                1_000_000,
            )
            .unwrap();
        claim = principal_a
            .claim_next_insomnia_input(lantern, "g4-semantic-negative", 2_000_000, 1_000_000, 8)
            .unwrap();
    }
    let claim = claim.expect("completed cohort must yield one Insomnia claim");
    let user_turn_ids = claim
        .turns
        .iter()
        .filter(|turn| turn.role == "user")
        .map(|turn| turn.node_id.clone())
        .collect::<Vec<_>>();
    let source_node_id = user_turn_ids
        .first()
        .expect("claimed Episode has a user source turn")
        .clone();
    let empty_before = principal_a
        .with_writable_reliquary(lantern, |rel| rel.memory_version())
        .unwrap();
    let extracted = extract_native_insomnia(
        ForgedQuoteEndpoint {
            source_node_id,
            user_turn_ids,
        },
        &claim,
        None,
        |_| unreachable!("the forged-quote ledger requests no evidence"),
    )
    .expect("the forged proposal is structurally valid for rejection");
    let prepared = principal_a
        .with_writable_reliquary(lantern, |rel| extracted.prepare(rel, &claim))
        .unwrap()
        .expect("source validation produces a rejected proposal");
    assert!(prepared.candidates().is_empty());
    assert!(!prepared.rejected().is_empty());
    principal_a
        .commit_prepared_insomnia_claim(
            lantern,
            &claim,
            &prepared,
            "g4-forged-quote",
            "v1",
            2_000_001,
        )
        .expect("rejected proposal still completes its durable claim");
    let (memory_version_after, receipt_memory_ids, rejected_count, work_completed) = principal_a
        .with_writable_reliquary(lantern, |rel| {
            let receipt = rel
                .insomnia_receipt(
                    InsomniaEpisodeId(claim.episode.id.0),
                    claim.work.attempt_count,
                )
                .expect("rejected claim has a terminal completion receipt");
            (
                rel.memory_version(),
                receipt.memory_ids.len(),
                receipt.rejected_count,
                rel.insomnia_work(InsomniaEpisodeId(claim.episode.id.0))
                    .is_some_and(|work| {
                        work.lease_token.is_none()
                            && work.retry_after_ns.is_none()
                            && work.held_payload.is_none()
                            && work.attempt_count == receipt.attempt
                    }),
            )
        })
        .unwrap();
    assert_eq!(memory_version_after, empty_before);
    assert_eq!(receipt_memory_ids, 0);
    assert!(rejected_count > 0);
    assert!(
        work_completed,
        "forged-quote work is no longer pending or leased"
    );

    assert!(matches!(
        principal_a.authorize_routed_owners(lantern, phy_b),
        Err(GatewayError::UnauthorizedPhy { .. })
    ));
    let cascade_memory = memory_id(find_source(witness, "src-cascade-plan"));
    assert!(matches!(
        principal_a.resolve_and_publish_entity_mention(
            EntityMemoryMention {
                owner_id: cascade.as_bytes(),
                memory_id: cascade_memory,
                field: TextField::Content,
                start_byte: 0,
                end_byte: 1,
                text: "C".into(),
            },
            PrivateEntityEndpoint,
            2_000_002,
        ),
        Err(GatewayError::UnauthorizedRel { owner, .. }
            | GatewayError::UnauthorizedPhy { owner, .. }) if owner == cascade
    ));

    let private_source = find_source(witness, "src-private-afternoons");
    assert_eq!(private_source["owner_id"], json!(hex(&phy_a.as_bytes())));
    let private_memory = memory_id(private_source);
    let authored_span = manifest["source_references"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "src-private-afternoons")
        .unwrap()["exact_span"]
        .as_str()
        .unwrap();
    let exact_mention = "afternoons";
    assert!(authored_span.contains(exact_mention));
    let start_byte = authored_span
        .find(exact_mention)
        .expect("the tested Entity span is in the exact authored source");
    let resolved = principal_a
        .resolve_and_publish_entity_mention(
            EntityMemoryMention {
                owner_id: phy_a.as_bytes(),
                memory_id: private_memory,
                field: TextField::Content,
                start_byte,
                end_byte: start_byte + exact_mention.len(),
                text: exact_mention.into(),
            },
            PrivateEntityEndpoint,
            2_000_003,
        )
        .unwrap();
    assert!(matches!(
        &resolved.status,
        MemoryEntityResolutionStatus::Resolved { .. }
    ));
    let private_entity_id = match &resolved.status {
        MemoryEntityResolutionStatus::Resolved { entity_id, .. } => hex(&entity_id.0),
        _ => unreachable!("resolution status was checked above"),
    };
    // A valid source cut cannot authorize a target Entity from another owner.
    let MemoryEntityResolutionStatus::Resolved {
        entity_id: foreign_entity,
        reason,
    } = resolved.status.clone()
    else {
        unreachable!()
    };
    let foreign_revision = principal_a
        .with_entity_phy_writer(phy_a, |writer| {
            writer
                .entity_mention_inputs(resolved.key)
                .unwrap()
                .entities
                .into_iter()
                .find(|entity| entity.id == foreign_entity)
                .unwrap()
                .revision
        })
        .unwrap();
    let mira = memory_id(find_source(witness, "src-mira"));
    let (foreign_target, cut_before) = principal_a
        .with_writable_reliquary(lantern, |rel| {
            let mention = rel
                .entity_resolutions()
                .into_iter()
                .find(|mention| mention.key.memory_id == mira.0)
                .unwrap();
            let memory = rel.current_memory(mira).unwrap();
            assert!(rel.entity(foreign_entity).is_none());
            let reliquary_owner_session::model::PayloadState::Available(body) = memory.payload
            else {
                panic!("retained source")
            };
            let text = match mention.key.field {
                reliquary_owner_session::model::MemoryTextField::Title => body.title,
                reliquary_owner_session::model::MemoryTextField::Content => body.content,
            };
            (
                reliquary_owner_session::EntityMentionCommit {
                    owner_id: lantern.as_bytes(),
                    memory_revision: memory.record.revision,
                    body_id: memory.record.body_id,
                    key: mention.key,
                    surface: text[mention.key.start_byte as usize..mention.key.end_byte as usize]
                        .to_owned(),
                    expected_revision: mention.revision,
                    updated_at_ns: 2_000_005,
                    status: MemoryEntityResolutionStatus::Resolved {
                        entity_id: foreign_entity,
                        reason,
                    },
                    target_revisions: vec![(foreign_entity, foreign_revision)],
                    new_entity: None,
                },
                rel.current_historical_cut(),
            )
        })
        .unwrap();
    let foreign_error = principal_a
        .with_rel_writer(lantern, |writer| {
            writer.commit_entity_mention(foreign_target)
        })
        .unwrap()
        .unwrap_err();
    assert!(
        matches!(foreign_error, reliquary_owner_session::OwnerSessionError::Record(message)
        if message == "mention target is unavailable to generic Entity resolution")
    );
    assert_eq!(
        principal_a
            .with_writable_reliquary(lantern, |rel| rel.current_historical_cut())
            .unwrap(),
        cut_before
    );
    let private_b_memory = memory_id(find_source(witness, "src-private-morning"));
    assert!(matches!(
        principal_a.resolve_and_publish_entity_mention(
            EntityMemoryMention {
                owner_id: phy_b.as_bytes(),
                memory_id: private_b_memory,
                field: TextField::Content,
                start_byte: 0,
                end_byte: 1,
                text: "M".into(),
            },
            PrivateEntityEndpoint,
            2_000_004,
        ),
        Err(GatewayError::UnauthorizedPhy { .. })
    ));

    let _ = operator;
    let _ = principal_b;
    json!({
        "invalid_episode_rejected": true,
        "invalid_episode_memory_version_unchanged": true,
        "forged_quote_rejected": prepared.rejected().len(),
        "forged_quote_candidates": prepared.candidates().len(),
        "forged_claim_completed": true,
        "forged_claim_receipt_memory_ids": receipt_memory_ids,
        "principal_a_phy_b_route_denied": true,
        "principal_a_cascade_entity_denied": true,
        "private_phy_entity_resolved": true,
        "private_phy_memory_id": hex(&private_memory.0),
        "private_phy_entity_id": private_entity_id,
        "private_phy_mention_key": {
            "memory_id": hex(&resolved.key.memory_id),
            "field": "content",
            "start_byte": resolved.key.start_byte,
            "end_byte": resolved.key.end_byte
        },
        "principal_a_phy_b_entity_denied": true
        ,"foreign_entity_target_rejected_without_mutation": true
    })
}

fn find_source<'a>(witness: &'a Value, id: &str) -> &'a Value {
    witness["sources"]
        .as_array()
        .expect("semantic witness sources are an array")
        .iter()
        .find(|item| item["source_id"] == id)
        .unwrap_or_else(|| panic!("missing semantic source witness {id}"))
}

fn parse_hex<const N: usize>(value: &str) -> [u8; N] {
    assert_eq!(value.len(), N * 2, "expected {N}-byte lowercase hex");
    let mut bytes = [0; N];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let digit = |byte: u8| match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => panic!("invalid lowercase hex"),
        };
        bytes[index] = (digit(pair[0]) << 4) | digit(pair[1]);
    }
    bytes
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
