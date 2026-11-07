//! End-to-end terminal Memory, Entity, and Relationship maintenance on
//! independent, verified copies of the completed Lantern REL.

#[cfg(test)]
use super::*;
use reliquary_application::{
    DreamClassifier, DreamVerificationPolicy, DreamVerifier, EntityMemoryMention,
    EntityOwnerSession, TextField, draft_dream_batch_organization, extract_native_insomnia,
};
use reliquary_container::Container;
use reliquary_owner_session::model::{
    EntityId, EntityResolutionReason, InsomniaEpisodeId, MemoryEntityResolutionStatus, MemoryId,
    MemoryTextField, RelationshipId,
};
use reliquary_owner_session::{
    CreateRelationshipRequest, DreamMemoryCut, EntityRetirementRequest, GraphAdmission,
    MemoryRetirementRequest, RelSession, RelationshipParticipantEvidence, RelationshipPruneReport,
    RelationshipRetirementRequest, VerifiedOwnerCopyPlan, WithdrawEntityMentionRequest,
    prune_terminal_entity_descriptions_copy, prune_terminal_memory_copy,
    prune_terminal_relationship_descriptions_copy,
};
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use serde_json::{Value, json};
use std::{fs, path::Path};

const G4_AUTHORED_MANIFEST: &str = include_str!("../../../tests/fixtures/g4-native-rel21-v1.json");

#[derive(Clone)]
struct L04MemoryEndpoint {
    turns: Vec<reliquary_owner_session::model::ResolvedTurn>,
    source_nodes: [String; 2],
}

impl GeneralEndpoint for L04MemoryEndpoint {
    fn model(&self) -> &str {
        "g4-terminal-l04"
    }

    fn complete_json(
        &self,
        _: &str,
        payload: &str,
        schema: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        match schema {
            "insomnia_authority_disposition_ledger" => {
                let turns = self
                    .turns
                    .iter()
                    .filter(|turn| turn.role == "user")
                    .map(|turn| {
                        let retained = self.source_nodes.contains(&turn.node_id);
                        let clauses = if retained {
                            let proposition = if turn.node_id == self.source_nodes[0] {
                                "The earlier deployment target was north lab."
                            } else {
                                "The deployment target is south lab."
                            };
                            json!([{
                                "disposition":"retain", "authority_kind":"direct",
                                "category":"fact", "type":"project",
                                "lifecycle":"current", "proposition":proposition,
                                "authority_source_node_id":"", "grounding_source_node_id":"",
                                "reason":"direct accepted user statement"
                            }])
                        } else {
                            omitted_clause()
                        };
                        (turn.node_id.clone(), clauses)
                    })
                    .collect::<serde_json::Map<String, Value>>();
                Ok(json!({"turns":turns,"evidence_requests":[]}))
            }
            "insomnia_memory_ownership" => {
                let input: Value = serde_json::from_str(payload).unwrap();
                let count = input["synthesis_groups"].as_array().unwrap().len();
                let ownership = (0..count)
                    .map(|i| (format!("g{i:03}"), json!("project")))
                    .collect::<serde_json::Map<_, _>>();
                Ok(json!({"ownership":ownership}))
            }
            "insomnia_memory_wording" => {
                let candidates = self
                    .turns
                    .iter()
                    .filter(|turn| self.source_nodes.contains(&turn.node_id))
                    .map(|turn| {
                        let (title, content) = if turn.node_id == self.source_nodes[0] {
                            (
                                "Earlier deployment target",
                                "The earlier deployment target was north lab.",
                            )
                        } else {
                            (
                                "Current deployment target",
                                "The deployment target is south lab.",
                            )
                        };
                        json!({
                            "authority_kind":"direct", "category":"fact",
                            "memory_type":"project", "temporal_status":"current",
                            "ownership":"project", "title":title, "content":content,
                            "source_node_id":turn.node_id,
                            "source_quote":turn.content,
                            "authority_source":null, "grounding_source":null,
                            "routing_mentions":[]
                        })
                    })
                    .collect::<Vec<_>>();
                Ok(json!({"candidates":candidates}))
            }
            other => panic!("unexpected L04 Insomnia schema: {other}"),
        }
    }
}

#[derive(Clone)]
struct EmptyLedgerEndpoint(Vec<String>);

impl GeneralEndpoint for EmptyLedgerEndpoint {
    fn model(&self) -> &str {
        "g4-terminal-empty-ledger"
    }

    fn complete_json(
        &self,
        _: &str,
        _: &str,
        schema: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        assert_eq!(schema, "insomnia_authority_disposition_ledger");
        let turns = self
            .0
            .iter()
            .map(|id| (id.clone(), omitted_clause()))
            .collect::<serde_json::Map<String, Value>>();
        Ok(json!({"turns":turns,"evidence_requests":[]}))
    }
}

struct TerminalEntityEndpoint;

fn omitted_clause() -> Value {
    json!([{"disposition":"omit", "authority_kind":"none", "category":"none",
        "type":"none", "lifecycle":"none", "proposition":"",
        "authority_source_node_id":"", "grounding_source_node_id":"",
        "reason":"No additional enduring fact in this context turn"}])
}

impl GeneralEndpoint for TerminalEntityEndpoint {
    fn model(&self) -> &str {
        "g4-terminal-entity"
    }

    fn complete_json(
        &self,
        _: &str,
        payload: &str,
        schema: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        let kind = if payload.contains("Lantern") {
            "application"
        } else {
            "person"
        };
        match schema {
            "entity_resolution" => Ok(json!({
                "decision":"create_new", "reason":"named_referent",
                "target_candidate_index":-1, "identity_relation":"related_distinct",
                "mention_kind":kind
            })),
            "entity_admission" => Ok(json!({
                "decision":"create_new", "reason":"first_seen_identity",
                "promotion_policy":"immediate", "entity_kind":kind,
                "entity_summary":format!("{kind} named by accepted G4 source evidence.")
            })),
            other => panic!("unexpected terminal Entity schema: {other}"),
        }
    }
}

struct TerminalDreamEndpoint {
    older_is_a: bool,
}

impl GeneralEndpoint for TerminalDreamEndpoint {
    fn model(&self) -> &str {
        "g4-terminal-dream-supersession"
    }

    fn complete_json(
        &self,
        _: &str,
        _: &str,
        schema: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        if schema == "dream_pair_verification" {
            Ok(json!({
                "relation_supported":"yes", "direction_supported":"yes",
                "evidence_supported":"yes"
            }))
        } else {
            assert_eq!(schema, "dream_pair_classification");
            let (direction, a, b) = if self.older_is_a {
                ("b_to_a", "north lab", "south lab")
            } else {
                ("a_to_b", "south lab", "north lab")
            };
            Ok(json!({
                "relation":"supersedes", "direction":direction,
                "evidence":[{"side":"a","quote":a},
                    {"side":"b","quote":b}]
            }))
        }
    }
}

/// Exercise source-bound retirement and three independently manifested
/// positive prunes, plus vector-reference refusal and Gateway authorization.
pub(crate) fn verify_terminal_semantics(
    root: &Path,
    source: &Path,
    semantic_witness: &Value,
) -> Value {
    fs::create_dir_all(root).expect("create terminal acceptance root");
    let work = root.join(format!("terminal-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&work).expect("create fresh terminal lineage directory");
    let completed = work.join("completed-lantern.rel");
    copy_verified(source, &completed);
    let completed_owner = owner_id(&completed);

    let vector_lineage = work.join("vector-negative-lineage.rel");
    copy_verified(&completed, &vector_lineage);
    let vector_refusal = retire_l08_then_refuse_vector_prune(
        &vector_lineage,
        &work.join("vector-refused.rel"),
        semantic_witness,
    );

    let entity_lineage = work.join("entity-lineage.rel");
    copy_verified(&completed, &entity_lineage);
    let (entity_id, entity_revision, entity_report, withdrawn_mentions) =
        withdraw_retire_and_prune_entity(
            &entity_lineage,
            &work.join("entity-pruned.rel"),
            semantic_witness,
        );
    let entity_pruned = RelSession::open_path_current(&work.join("entity-pruned.rel"))
        .expect("open pruned terminal Entity copy");
    assert!(entity_pruned.entity_tombstone(entity_id).is_some());
    assert!(
        entity_pruned
            .entity_description_prune(entity_id, entity_revision)
            .is_some()
    );
    assert!(
        entity_pruned
            .entity_revision(entity_id, entity_revision)
            .is_some_and(|revision| revision.summary.is_empty())
    );
    drop(entity_pruned);

    let relationship_lineage = work.join("relationship-lineage.rel");
    copy_verified(&completed, &relationship_lineage);
    let (relationship_id, relationship_revision, relationship_report) =
        create_retire_and_prune_relationship(
            &relationship_lineage,
            &work.join("relationship-pruned.rel"),
            semantic_witness,
        );
    let relationship_pruned = RelSession::open_path_current(&work.join("relationship-pruned.rel"))
        .expect("open pruned terminal Relationship copy");
    assert!(
        relationship_pruned
            .relationship_tombstone(relationship_id)
            .is_some()
    );
    assert!(
        relationship_pruned
            .relationship_description_prune(relationship_id, relationship_revision)
            .is_some()
    );
    assert!(
        relationship_pruned
            .relationship_revision(relationship_id, relationship_revision)
            .is_some_and(|revision| revision.summary.is_empty())
    );
    drop(relationship_pruned);

    let denied_lineage = work.join("authorization-negative-lineage.rel");
    copy_verified(&completed, &denied_lineage);
    let authorization = verify_lifecycle_denials(&denied_lineage, semantic_witness);
    assert_eq!(owner_id(&completed), completed_owner);

    let memory_lineage = work.join("memory-lineage.rel");
    copy_verified(&completed, &memory_lineage);
    let (positive_memory, retired_memory, memory_prune_report) =
        retire_and_prune_l04(&memory_lineage, &work.join("memory-pruned.rel"));
    let memory_pruned = RelSession::open_path_current(&work.join("memory-pruned.rel"))
        .expect("open pruned terminal Memory copy");
    let history = memory_pruned
        .memory_history(retired_memory)
        .unwrap()
        .expect("terminal Memory history remains present after body prune");
    assert!(history.tombstone.is_some());
    assert!(history.revisions.iter().all(|revision| matches!(
        revision.payload,
        reliquary_owner_session::model::PayloadState::Absent
    )));
    assert_eq!(
        memory_pruned
            .current_memory(positive_memory)
            .unwrap()
            .record
            .id,
        positive_memory
    );
    drop(memory_pruned);

    json!({
        "completed_copy_owner_id": hex(&completed_owner),
        "completed_copy_bytes_match_source": fs::read(&completed).unwrap() == fs::read(source).unwrap(),
        "positive_memory_prune": {
            "retired_memory_id": hex(&retired_memory.0),
            "successor_memory_id": hex(&positive_memory.0),
            "pruned_memories": memory_prune_report.pruned_memories,
            "terminal_history_reopened": true,
            "retired_and_pruned_memory_excluded_from_retrieval": true
        },
        "vector_reference_prune_refusal": vector_refusal,
        "positive_entity_prune": {
            "entity_id": hex(&entity_id.0),
            "entity_revision": entity_revision,
            "withdrawn_mentions": withdrawn_mentions,
            "pruned_entities": entity_report.pruned_entities,
            "terminal_description_reopened": true
        },
        "positive_relationship_prune": {
            "relationship_id": hex(&relationship_id.0),
            "relationship_revision": relationship_revision,
            "pruned_relationships": relationship_report.pruned_relationships,
            "terminal_description_reopened": true
        },
        "gateway_lifecycle_denials": authorization
    })
}

fn retire_and_prune_l04(
    path: &Path,
    pruned_path: &Path,
) -> (
    MemoryId,
    MemoryId,
    reliquary_owner_session::PrunedOwnerCopyReport,
) {
    let gateway = Gateway::new();
    let owner = gateway.mount_writable_reliquary_current(path).unwrap();
    let instance = gateway.instance("principal-a", [owner], []).unwrap();
    let episode = instance
        .finalize_conversation_episode(
            owner,
            "conv-l04",
            "L04-t1",
            "L04-t4",
            reliquary_owner_session::model::EpisodeBoundary::Explicit,
            300_000,
        )
        .unwrap();
    let mut target_claim = None;
    for _ in 0..64 {
        let Some(claim) = instance
            .claim_next_insomnia_input(owner, "g4-terminal-l04", 300_001, 100_000, 64)
            .unwrap()
        else {
            break;
        };
        if claim.episode.id == episode {
            target_claim = Some(claim);
            break;
        }
        complete_empty_claim(&instance, owner, &claim);
    }
    let claim = target_claim.expect("L04 accepted Episode becomes claimable");
    assert_eq!(claim.episode.conversation_id, "conv-l04");
    assert_eq!(claim.episode.start_node_id, "L04-t1");
    assert_eq!(claim.episode.end_node_id, "L04-t4");
    let turn_by_id = claim
        .turns
        .iter()
        .map(|turn| (turn.node_id.clone(), turn.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let selected = ["L04-t1".to_owned(), "L04-t4".to_owned()];
    for node in &selected {
        assert!(turn_by_id.contains_key(node));
    }
    let endpoint = L04MemoryEndpoint {
        turns: claim.turns.clone(),
        source_nodes: [selected[0].clone(), selected[1].clone()],
    };
    let extraction = extract_native_insomnia(endpoint, &claim, None, |_| {
        unreachable!("L04 authored authority requires no extra Archive evidence")
    })
    .expect("scripted L04 extraction completes");
    let prepared = instance
        .with_writable_reliquary(owner, |rel| extraction.prepare(rel, &claim))
        .unwrap()
        .unwrap();
    assert_eq!(prepared.project_memories().count(), 2);
    assert!(prepared.rejected().is_empty());
    let completion = instance
        .commit_prepared_insomnia_claim(owner, &claim, &prepared, "g4-terminal-l04", "v1", 300_002)
        .unwrap();
    assert_eq!(completion, reliquary_owner_session::CompletionCheck::Apply);
    let ids = instance
        .with_writable_reliquary(owner, |rel| {
            rel.insomnia_receipt(InsomniaEpisodeId(episode.0), claim.work.attempt_count)
                .unwrap()
                .memory_ids
                .iter()
                .map(|id| MemoryId(id.0))
                .collect::<Vec<_>>()
        })
        .unwrap();
    assert_eq!(ids.len(), 2);

    let cuts = instance
        .with_writable_reliquary(owner, |rel| {
            ids.iter()
                .map(|id| {
                    let record = rel.current_memory(*id).unwrap().record;
                    DreamMemoryCut {
                        id: *id,
                        revision: record.revision,
                        body_id: record.body_id,
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap();
    let source_memory = ids_for_source(&instance, owner, &ids, "L04-t1");
    let successor_memory = ids_for_source(&instance, owner, &ids, "L04-t4");
    let source_cut = cuts.iter().find(|cut| cut.id == source_memory).unwrap();
    let successor_cut = cuts.iter().find(|cut| cut.id == successor_memory).unwrap();
    instance
        .with_rel_writer(owner, |writer| {
            writer.begin_initial_dream_frontier(
                "g4-terminal-l04-supersession".into(),
                *source_cut,
                vec![*successor_cut],
                300_003,
            )
        })
        .unwrap()
        .unwrap();
    let dream = instance
        .with_writable_reliquary(owner, |rel| {
            draft_dream_batch_organization(
                &EntityOwnerSession::Rel(rel),
                owner.as_bytes(),
                source_memory,
                &[successor_memory],
                &DreamClassifier::new(TerminalDreamEndpoint {
                    older_is_a: source_memory < successor_memory,
                }),
                Some(&DreamVerifier::new(TerminalDreamEndpoint {
                    older_is_a: source_memory < successor_memory,
                })),
                DreamVerificationPolicy::default(),
            )
        })
        .unwrap()
        .unwrap();
    assert!(
        instance
            .publish_dream_batch(owner, [0x71; 32], &dream, 300_004)
            .unwrap()
    );

    let (old_record, new_record) = instance
        .with_writable_reliquary(owner, |rel| {
            (
                rel.current_memory(source_memory).unwrap().record,
                rel.current_memory(successor_memory).unwrap().record,
            )
        })
        .unwrap();
    assert_eq!(old_record.superseded_by, Some(successor_memory));
    assert!(old_record.archived);
    assert!(!new_record.archived);
    instance
        .retire_semantic_memory(
            owner,
            MemoryRetirementRequest {
                owner_id: owner.as_bytes(),
                memory_id: source_memory,
                memory_revision: old_record.revision,
                body_id: old_record.body_id,
                superseded_by: successor_memory,
                successor_revision: new_record.revision,
                successor_body_id: new_record.body_id,
            },
        )
        .expect("Dream-superseded L04 Memory retires through authorized Gateway");
    drop(instance);
    drop(gateway);

    assert_terminal_retrieval(path, successor_memory, source_memory);
    let admission = GraphAdmission::for_replay(owner.as_bytes(), [], [], [], [], []);
    let plan = VerifiedOwnerCopyPlan::prepare(path).unwrap();
    let report = prune_terminal_memory_copy(&plan, path, pruned_path, &[source_memory], &admission)
        .expect("independent terminal L04 Memory with no vector references prunes");
    assert_terminal_retrieval(pruned_path, successor_memory, source_memory);
    (successor_memory, source_memory, report)
}

fn assert_terminal_retrieval(path: &Path, successor: MemoryId, retired: MemoryId) {
    let gateway = Gateway::new();
    let owner = gateway.mount_reliquary_current(path).unwrap();
    let instance = gateway.instance("principal-a", [owner], []).unwrap();
    let selected = instance
        .rank_rel_memories(
            owner,
            "deployment target",
            None,
            None,
            reliquary_application::RetrievalConfig::default(),
        )
        .unwrap();
    assert_eq!(
        selected
            .selected
            .iter()
            .map(|hit| hit.memory_id)
            .collect::<Vec<_>>(),
        vec![successor]
    );
    assert!(selected.selected.iter().all(|hit| hit.memory_id != retired));
}

fn complete_empty_claim(
    instance: &Instance,
    owner: OwnerId,
    claim: &reliquary_owner_session::RelInsomniaClaimInput,
) {
    let endpoint = EmptyLedgerEndpoint(
        claim
            .turns
            .iter()
            .filter(|turn| turn.role == "user")
            .map(|turn| turn.node_id.clone())
            .collect(),
    );
    let extraction = extract_native_insomnia(endpoint, claim, None, |_| unreachable!())
        .expect("empty ledger work is structurally valid");
    let prepared = instance
        .with_writable_reliquary(owner, |rel| extraction.prepare(rel, claim))
        .unwrap()
        .unwrap();
    assert!(prepared.candidates().is_empty());
    instance
        .commit_prepared_insomnia_claim(owner, claim, &prepared, "g4-terminal-empty", "v1", 300_002)
        .expect("unrelated claim is closed without Memory publication");
}

fn ids_for_source(instance: &Instance, owner: OwnerId, ids: &[MemoryId], source: &str) -> MemoryId {
    instance
        .with_writable_reliquary(owner, |rel| {
            ids.iter()
                .copied()
                .find(|id| {
                    rel.current_memory(*id)
                        .unwrap()
                        .record
                        .source_node_id
                        .as_deref()
                        == Some(source)
                })
                .expect("Insomnia Memory retains its authored source node")
        })
        .unwrap()
}

fn retire_l08_then_refuse_vector_prune(path: &Path, refused_path: &Path, witness: &Value) -> Value {
    let pending = source_memory_id(witness, "src-rehearsal-pending");
    let complete = source_memory_id(witness, "src-rehearsal-complete");
    let gateway = Gateway::new();
    let owner = gateway.mount_writable_reliquary_current(path).unwrap();
    let instance = gateway.instance("principal-a", [owner], []).unwrap();
    let (old, new) = instance
        .with_writable_reliquary(owner, |rel| {
            (
                rel.current_memory(pending).unwrap().record,
                rel.current_memory(complete).unwrap().record,
            )
        })
        .unwrap();
    assert_eq!(old.superseded_by, Some(complete));
    instance
        .retire_semantic_memory(
            owner,
            MemoryRetirementRequest {
                owner_id: owner.as_bytes(),
                memory_id: pending,
                memory_revision: old.revision,
                body_id: old.body_id,
                superseded_by: complete,
                successor_revision: new.revision,
                successor_body_id: new.body_id,
            },
        )
        .expect("authored L08 superseded Memory retires before vector check");
    drop(instance);
    drop(gateway);
    let plan = VerifiedOwnerCopyPlan::prepare(path).unwrap();
    let admission = GraphAdmission::for_replay(owner.as_bytes(), [], [], [], [], []);
    let error = prune_terminal_memory_copy(&plan, path, refused_path, &[pending], &admission)
        .expect_err("L08 Memory body remains referenced by published vectors");
    assert!(error.contains("referenced by vectors"), "{error}");
    assert!(!refused_path.exists());
    json!({
        "memory_id": hex(&pending.0),
        "vector_reference_refusal": error,
        "destination_absent": true
    })
}

fn withdraw_retire_and_prune_entity(
    path: &Path,
    pruned_path: &Path,
    witness: &Value,
) -> (
    EntityId,
    u64,
    reliquary_owner_session::EntityPruneReport,
    usize,
) {
    let entity_id = entity_id(&witness["entity"]["mira_entity_id"]);
    let gateway = Gateway::new();
    let owner = gateway.mount_writable_reliquary_current(path).unwrap();
    let instance = gateway.instance("principal-a", [owner], []).unwrap();
    let requests = instance
        .with_writable_reliquary(owner, |rel| {
            let entity = rel
                .entity(entity_id)
                .expect("authored Mira Entity is active");
            let targets = rel
                .entity_resolutions()
                .into_iter()
                .filter_map(|mention| {
                    let ids = match &mention.status {
                        MemoryEntityResolutionStatus::Resolved { entity_id, .. } => {
                            vec![*entity_id]
                        }
                        MemoryEntityResolutionStatus::Ambiguous { candidates } => {
                            candidates.clone()
                        }
                        MemoryEntityResolutionStatus::Unresolved { .. } => return None,
                    };
                    if !ids.contains(&entity_id) {
                        return None;
                    }
                    let memory = rel.current_memory(MemoryId(mention.key.memory_id)).unwrap();
                    let body = available_body(&memory.payload);
                    let text = field_text(&body, mention.key.field)
                        .get(mention.key.start_byte as usize..mention.key.end_byte as usize)
                        .expect("active mention retains its exact source surface")
                        .to_owned();
                    let target_revisions = ids
                        .iter()
                        .map(|id| (*id, rel.entity(*id).unwrap().revision))
                        .collect();
                    Some(WithdrawEntityMentionRequest {
                        owner_id: owner.as_bytes(),
                        memory_id: MemoryId(mention.key.memory_id),
                        memory_revision: memory.record.revision,
                        body_id: memory.record.body_id,
                        key: mention.key,
                        source_surface: text,
                        expected_mention_revision: mention.revision,
                        target_revisions,
                        withdrawn_at_ns: 600_000.max(mention.updated_at_ns),
                    })
                })
                .collect::<Vec<_>>();
            assert_eq!(entity.canonical_name, "Mira");
            targets
        })
        .unwrap();
    assert!(
        !requests.is_empty(),
        "the accepted Mira source has an active mention"
    );
    for request in &requests {
        let withdrawn = instance
            .withdraw_semantic_entity_mention(owner, request.clone())
            .expect("owner withdraws exact source-bound mention");
        assert!(matches!(
            withdrawn.status,
            MemoryEntityResolutionStatus::Unresolved {
                reason: EntityResolutionReason::UserWithdrawal
            }
        ));
    }
    let source_memory = source_memory_id(witness, "src-mira");
    let (entity_revision, memory_record, start, end, quote) = instance
        .with_writable_reliquary(owner, |rel| {
            let entity = rel.entity(entity_id).unwrap();
            let memory = rel.current_memory(source_memory).unwrap();
            let body = available_body(&memory.payload);
            let start = body.content.find("Mira").unwrap();
            (
                entity.revision,
                memory.record,
                start,
                start + "Mira".len(),
                "Mira".to_owned(),
            )
        })
        .unwrap();
    instance
        .retire_semantic_entity(
            owner,
            EntityRetirementRequest {
                owner_id: owner.as_bytes(),
                entity_id,
                entity_revision,
                source_memory_id: source_memory,
                source_memory_revision: memory_record.revision,
                source_body_id: memory_record.body_id,
                source_field: MemoryTextField::Content,
                start_byte: start,
                end_byte: end,
                source_quote: quote,
            },
        )
        .expect("Entity retires only after its active mentions were withdrawn");
    drop(instance);
    drop(gateway);
    let admission = GraphAdmission::for_replay(owner.as_bytes(), [], [], [], [], []);
    let plan = VerifiedOwnerCopyPlan::prepare(path).unwrap();
    let report =
        prune_terminal_entity_descriptions_copy(&plan, path, pruned_path, &[entity_id], &admission)
            .expect("terminal unreferenced Entity description prunes");
    (entity_id, entity_revision, report, requests.len())
}

fn create_retire_and_prune_relationship(
    path: &Path,
    pruned_path: &Path,
    witness: &Value,
) -> (RelationshipId, u64, RelationshipPruneReport) {
    let mira_id = entity_id(&witness["entity"]["mira_entity_id"]);
    let gateway = Gateway::new();
    let owner = gateway.mount_writable_reliquary_current(path).unwrap();
    let instance = gateway.instance("principal-a", [owner], []).unwrap();
    let memory_id = source_memory_id(witness, "src-mira-known");
    let (mira_revision, memory_record, span, start, lantern_start) = instance
        .with_writable_reliquary(owner, |rel| {
            let memory = rel.current_memory(memory_id).unwrap();
            let body = available_body(&memory.payload);
            let span = manifest_turn_content("L05-t2");
            let start = body
                .content
                .find(&span)
                .expect("Memory preserves accepted Mira source span");
            let lantern_start = body.content[start..]
                .find("Lantern")
                .map(|offset| start + offset)
                .expect("accepted Mira source span names Lantern");
            (
                rel.entity(mira_id)
                    .expect("authored Mira Entity remains active")
                    .revision,
                memory.record,
                span,
                start,
                lantern_start,
            )
        })
        .unwrap();
    let lantern_mention = instance
        .resolve_and_publish_entity_mention(
            EntityMemoryMention {
                owner_id: owner.as_bytes(),
                memory_id,
                field: TextField::Content,
                start_byte: lantern_start,
                end_byte: lantern_start + "Lantern".len(),
                text: "Lantern".into(),
            },
            TerminalEntityEndpoint,
            700_000,
        )
        .unwrap();
    let lantern_id = match lantern_mention.status {
        MemoryEntityResolutionStatus::Resolved { entity_id, .. } => entity_id,
        status => panic!("Lantern Entity was not resolved: {status:?}"),
    };
    let lantern_revision = instance
        .with_writable_reliquary(owner, |rel| rel.entity(lantern_id).unwrap().revision)
        .unwrap();
    let relationship = instance
        .publish_semantic_relationship(
            owner,
            CreateRelationshipRequest {
                owner_id: owner.as_bytes(),
                kind: "reviewed".into(),
                participants: vec![
                    RelationshipParticipantEvidence {
                        entity_id: mira_id,
                        entity_revision: mira_revision,
                        role: Some("reviewer".into()),
                    },
                    RelationshipParticipantEvidence {
                        entity_id: lantern_id,
                        entity_revision: lantern_revision,
                        role: Some("project".into()),
                    },
                ],
                source_memory_id: memory_id,
                source_memory_revision: memory_record.revision,
                source_body_id: memory_record.body_id,
                source_field: MemoryTextField::Content,
                start_byte: start,
                end_byte: start + span.len(),
                source_quote: span.clone(),
                mutation_id: "g4-terminal-mira-reviewed-lantern".into(),
                updated_at_ns: 700_001,
            },
        )
        .expect("Relationship names each Entity in accepted Memory evidence");
    assert_eq!(relationship.participants.len(), 2);
    instance
        .retire_semantic_relationship(
            owner,
            RelationshipRetirementRequest {
                owner_id: owner.as_bytes(),
                relationship_id: relationship.id,
                relationship_revision: relationship.revision,
                source_memory_id: memory_id,
                source_memory_revision: memory_record.revision,
                source_body_id: memory_record.body_id,
                source_field: MemoryTextField::Content,
                start_byte: start,
                end_byte: start + span.len(),
                source_quote: span,
            },
        )
        .expect("Relationship retires against its exact accepted source");
    drop(instance);
    drop(gateway);
    let admission = GraphAdmission::for_replay(owner.as_bytes(), [], [], [], [], []);
    let plan = VerifiedOwnerCopyPlan::prepare(path).unwrap();
    let report = prune_terminal_relationship_descriptions_copy(
        &plan,
        path,
        pruned_path,
        &[relationship.id],
        &admission,
    )
    .expect("terminal Relationship description prunes");
    (relationship.id, relationship.revision, report)
}

fn verify_lifecycle_denials(path: &Path, witness: &Value) -> Value {
    let before = fs::read(path).unwrap();
    let gateway = Gateway::new();
    let owner = gateway.mount_writable_reliquary_current(path).unwrap();
    let token = "g4 semantic lifecycle authorized credential";
    let (admission, control) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "principal-a",
        token,
        [owner],
        [],
    )])
    .install(&gateway)
    .unwrap();
    let authorized = admission.authenticate(token).unwrap();
    let outsider = gateway.instance("outsider", [], []).unwrap();
    let pending = source_memory_id(witness, "src-rehearsal-pending");
    let complete = source_memory_id(witness, "src-rehearsal-complete");
    let mira = source_memory_id(witness, "src-mira");
    let (old, successor, mira_record, mira_text) = authorized
        .with_writable_reliquary(owner, |rel| {
            let old = rel.current_memory(pending).unwrap().record;
            let successor = rel.current_memory(complete).unwrap().record;
            let memory = rel.current_memory(mira).unwrap();
            let body = available_body(&memory.payload);
            let start = body.content.find("Mira").unwrap();
            (old, successor, memory.record, (start, start + 4))
        })
        .unwrap();
    let retire_request = MemoryRetirementRequest {
        owner_id: owner.as_bytes(),
        memory_id: pending,
        memory_revision: old.revision,
        body_id: old.body_id,
        superseded_by: complete,
        successor_revision: successor.revision,
        successor_body_id: successor.body_id,
    };
    let (withdraw_request, mira_entity) = authorized
        .with_writable_reliquary(owner, |rel| {
            let (mention, entity) = rel
                .entity_resolutions()
                .into_iter()
                .find_map(|mention| match &mention.status {
                    MemoryEntityResolutionStatus::Resolved { entity_id, .. }
                        if mention.key.memory_id == mira.0 =>
                    {
                        let entity_id = *entity_id;
                        Some((mention, entity_id))
                    }
                    _ => None,
                })
                .expect("completed owner has a resolved Mira source mention");
            let entity = rel.entity(entity).unwrap();
            let memory = rel.current_memory(mira).unwrap();
            let body = available_body(&memory.payload);
            let surface = field_text(&body, mention.key.field)
                .get(mention.key.start_byte as usize..mention.key.end_byte as usize)
                .unwrap()
                .to_owned();
            (
                WithdrawEntityMentionRequest {
                    owner_id: owner.as_bytes(),
                    memory_id: mira,
                    memory_revision: memory.record.revision,
                    body_id: memory.record.body_id,
                    key: mention.key,
                    source_surface: surface,
                    expected_mention_revision: mention.revision,
                    target_revisions: vec![(entity.id, entity.revision)],
                    withdrawn_at_ns: 800_000.max(mention.updated_at_ns),
                },
                entity.id,
            )
        })
        .unwrap();
    let entity_request = EntityRetirementRequest {
        owner_id: owner.as_bytes(),
        entity_id: mira_entity,
        entity_revision: authorized
            .with_writable_reliquary(owner, |rel| rel.entity(mira_entity).unwrap().revision)
            .unwrap(),
        source_memory_id: mira,
        source_memory_revision: mira_record.revision,
        source_body_id: mira_record.body_id,
        source_field: MemoryTextField::Content,
        start_byte: mira_text.0,
        end_byte: mira_text.1,
        source_quote: "Mira".into(),
    };
    let relationship_request = CreateRelationshipRequest {
        owner_id: owner.as_bytes(),
        kind: "auth-denial-probe".into(),
        participants: vec![RelationshipParticipantEvidence {
            entity_id: mira_entity,
            entity_revision: entity_request.entity_revision,
            role: None,
        }],
        source_memory_id: mira,
        source_memory_revision: mira_record.revision,
        source_body_id: mira_record.body_id,
        source_field: MemoryTextField::Content,
        start_byte: mira_text.0,
        end_byte: mira_text.1,
        source_quote: "Mira".into(),
        mutation_id: "g4-terminal-auth-denial".into(),
        updated_at_ns: 800_001,
    };
    let cut_before = authorized
        .with_writable_reliquary(owner, |rel| rel.current_historical_cut())
        .unwrap();
    assert!(matches!(
        outsider.retire_semantic_memory(owner, retire_request.clone()),
        Err(GatewayError::UnauthorizedRel { .. } | GatewayError::UnauthorizedPhy { .. })
    ));
    assert!(matches!(
        outsider.withdraw_semantic_entity_mention(owner, withdraw_request.clone()),
        Err(GatewayError::UnauthorizedRel { .. } | GatewayError::UnauthorizedPhy { .. })
    ));
    assert!(matches!(
        outsider.publish_semantic_relationship(owner, relationship_request.clone()),
        Err(GatewayError::UnauthorizedRel { .. } | GatewayError::UnauthorizedPhy { .. })
    ));
    assert!(matches!(
        outsider.retire_semantic_entity(owner, entity_request.clone()),
        Err(GatewayError::UnauthorizedRel { .. } | GatewayError::UnauthorizedPhy { .. })
    ));
    let mut wrong_owner = retire_request.clone();
    wrong_owner.owner_id = [0xFF; 16];
    assert!(
        authorized
            .retire_semantic_memory(owner, wrong_owner)
            .is_err()
    );
    assert_eq!(
        authorized
            .with_writable_reliquary(owner, |rel| rel.current_historical_cut())
            .unwrap(),
        cut_before
    );
    let memory_version = authorized
        .with_writable_reliquary(owner, |rel| rel.memory_version())
        .unwrap();
    assert!(
        authorized
            .with_writable_reliquary(owner, |rel| rel.mention(withdraw_request.key).cloned())
            .unwrap()
            .is_some_and(|mention| matches!(
                mention.status,
                MemoryEntityResolutionStatus::Resolved { .. }
            ))
    );
    assert_eq!(
        authorized
            .with_writable_reliquary(owner, |rel| rel.memory_version())
            .unwrap(),
        memory_version
    );
    drop(outsider);
    control.revoke(token).unwrap();
    assert!(matches!(
        authorized.retire_semantic_memory(owner, retire_request),
        Err(GatewayError::CredentialRevoked)
    ));
    assert!(matches!(
        authorized.retire_semantic_entity(owner, entity_request),
        Err(GatewayError::CredentialRevoked)
    ));
    assert!(matches!(
        authorized.withdraw_semantic_entity_mention(owner, withdraw_request),
        Err(GatewayError::CredentialRevoked)
    ));
    assert!(matches!(
        authorized.publish_semantic_relationship(owner, relationship_request),
        Err(GatewayError::CredentialRevoked)
    ));
    drop(authorized);
    drop(admission);
    drop(control);
    drop(gateway);
    assert_eq!(
        fs::read(path).unwrap(),
        before,
        "denied writes preserve owner bytes"
    );
    json!({
        "outsider_retire_denied": true,
        "outsider_withdraw_denied": true,
        "outsider_create_relationship_denied": true,
        "wrong_request_owner_denied": true,
        "revoked_lifecycle_writes_denied":true,
        "owner_bytes_unchanged": true,
        "mention_remains_resolved": true
    })
}

fn source_memory_id(witness: &Value, source_id: &str) -> MemoryId {
    let source = witness["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["source_id"] == source_id)
        .unwrap_or_else(|| panic!("missing G4 source {source_id}"));
    MemoryId(parse_hex::<32>(source["memory_id"].as_str().unwrap()))
}

fn entity_id(value: &Value) -> EntityId {
    let bytes = value
        .as_array()
        .expect("semantic Entity witness is a serialized 32-byte array")
        .iter()
        .map(|value| value.as_u64().unwrap().try_into().unwrap())
        .collect::<Vec<u8>>();
    EntityId(bytes.try_into().expect("32-byte Entity identity"))
}

fn manifest_turn_content(turn_id: &str) -> String {
    let manifest: Value = serde_json::from_str(G4_AUTHORED_MANIFEST).unwrap();
    manifest["conversation_scripts"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|conversation| conversation["turns"].as_array().unwrap())
        .find(|turn| turn["id"] == turn_id)
        .unwrap_or_else(|| panic!("manifest turn missing {turn_id}"))["content"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn available_body(
    payload: &reliquary_owner_session::model::PayloadState,
) -> reliquary_owner_session::model::MemoryBody {
    match payload {
        reliquary_owner_session::model::PayloadState::Available(body) => body.clone(),
        reliquary_owner_session::model::PayloadState::Absent => {
            panic!("source Memory payload is present")
        }
    }
}

fn field_text<'a>(
    body: &'a reliquary_owner_session::model::MemoryBody,
    field: MemoryTextField,
) -> &'a str {
    match field {
        MemoryTextField::Title => &body.title,
        MemoryTextField::Content => &body.content,
    }
}

fn copy_verified(source: &Path, destination: &Path) {
    let plan = VerifiedOwnerCopyPlan::prepare(source).expect("prepare source owner copy proof");
    let report = plan
        .copy_to(source, destination)
        .expect("publish byte-identical completed owner copy");
    assert_eq!(report.byte_len, fs::metadata(source).unwrap().len());
    assert_eq!(fs::read(source).unwrap(), fs::read(destination).unwrap());
}

fn owner_id(path: &Path) -> [u8; 16] {
    Container::open_read_only(path)
        .expect("read copied REL identity")
        .owner_id()
}

fn parse_hex<const N: usize>(value: &str) -> [u8; N] {
    assert_eq!(value.len(), N * 2);
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
