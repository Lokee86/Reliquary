use super::*;
use reliquary_freshness_store::FreshnessReplayEntry;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const SCHEMA: &str = "reliquary-owner-history-proof-v2";

pub(super) fn rel(
    owner_id: [u8; 16],
    history: &CommonHistory,
    archive: &reliquary_archive::ArchiveHistorySnapshot,
    freshness: &BTreeMap<FreshnessMemoryId, FreshnessRecord>,
    freshness_evidence: &[FreshnessReplayEntry],
    cut: &RelHistoricalCut,
) -> [u8; 32] {
    canonical_hash(json!({
        "schema": SCHEMA, "kind": "REL", "owner": owner_id,
        "cut": rel_cut_value(cut), "common": common_value(history),
        "archive": archive_value(archive),
        "freshness": freshness_value(freshness, freshness_evidence),
    }))
}

pub(super) fn phy(
    owner_id: [u8; 16],
    history: &CommonHistory,
    cut: HistoricalOwnerCut,
) -> [u8; 32] {
    canonical_hash(json!({
        "schema": SCHEMA, "kind": "PHY", "owner": owner_id,
        "cut": owner_cut_value(cut), "common": common_value(history),
    }))
}

fn canonical_hash(value: Value) -> [u8; 32] {
    // serde_json's default Map is key-ordered; arrays below have explicit
    // semantic order and this schema contains no floating-point values.
    let bytes = serde_json::to_vec(&value).expect("historical proof value is serializable");
    let mut hash = Sha256::new();
    hash.update(SCHEMA.as_bytes());
    hash.update([0]);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

fn owner_cut_value(cut: HistoricalOwnerCut) -> Value {
    json!({
        "memory_version": cut.memory_version,
        "perception_version": cut.perception_version,
        "graph_version": cut.graph_version,
        "ego_version": cut.ego_version,
        "vector_version": cut.vector_version,
    })
}

fn rel_cut_value(cut: &RelHistoricalCut) -> Value {
    json!({
        "owner": owner_cut_value(cut.owner),
        "archive_version": cut.archive_version,
        "freshness": {
            "rel_turn": cut.freshness.rel_turn,
            "event_id_inclusive": cut.freshness.event_id_inclusive,
        },
    })
}

fn common_value(history: &CommonHistory) -> Value {
    let mut value = json!({
        "memories": history.memories.iter().map(memory_history_value).collect::<Vec<_>>(),
        "perception": perception_value(&history.perception),
        "graph_publications": history.graph_history.iter().map(relation_value).collect::<Vec<_>>(),
        "communities": history.community_history.iter().map(community_value).collect::<Vec<_>>(),
        "ego_publications": history.ego.records.iter().map(ego_record_value).collect::<Vec<_>>(),
        "vector_generations": history.generations.iter().map(generation_value).collect::<Vec<_>>(),
    });
    if !history.community_renames.is_empty() {
        value["community_renames"] = json!(
            history
                .community_renames
                .iter()
                .map(|receipt| {
                    let request = &receipt.request;
                    json!({
                        "request_key": request.request_key, "cut_digest": request.cut_digest,
                        "id": request.id.0, "expected_generation": request.expected_generation,
                        "graph_version": request.graph_version, "name": request.name,
                        "generation": receipt.generation, "member_count": receipt.member_count,
                    })
                })
                .collect::<Vec<_>>()
        );
    }
    value
}

fn memory_history_value(value: &(MemoryId, MemoryHistory)) -> Value {
    let (id, history) = value;
    json!({
        "id": id.0,
        "revisions": history.revisions.iter().map(memory_revision_value).collect::<Vec<_>>(),
        "tombstone": history.tombstone.map(|tombstone| json!({
            "revision": tombstone.revision, "memory_version": tombstone.memory_version,
        })),
    })
}

fn memory_revision_value(revision: &MemoryRevision) -> Value {
    let record = &revision.record;
    json!({
        "record": {
            "id": record.id.0, "revision": record.revision, "body_id": record.body_id.0,
            "category": record.category, "memory_type": record.memory_type,
            "authority_kind": record.authority_kind, "temporal_status": record.temporal_status,
            "scope": record.scope, "lifecycle_state": record.lifecycle_state,
            "archived": record.archived, "parent_id": record.parent_id.map(|id| id.0),
            "superseded_by": record.superseded_by.map(|id| id.0),
            "source_node_id": record.source_node_id,
            "content_source_conversation_id": record.content_source_conversation_id,
            "content_source_node_id": record.content_source_node_id,
            "grounding_source_conversation_id": record.grounding_source_conversation_id,
            "grounding_source_node_id": record.grounding_source_node_id,
            "source_episode_id": record.source_episode_id,
            "source_ref": record.source_ref.as_ref().map(memory_source_value),
            "source_time_ns": record.source_time_ns,
            "temporal_inference": record.temporal_inference.as_ref().map(memory_temporal_value),
            "mutation_id": record.mutation_id, "created_at_ns": record.created_at_ns,
            "updated_at_ns": record.updated_at_ns, "memory_version": record.memory_version,
        },
        "payload": match &revision.payload {
            reliquary_memory_core::PayloadState::Available(body) => json!({
                "state": "available", "title": body.title, "content": body.content,
            }),
            reliquary_memory_core::PayloadState::Absent => json!({"state": "absent"}),
        },
    })
}

fn memory_source_value(value: &reliquary_memory_core::MemorySourceRef) -> Value {
    json!({
        "owner_id": value.owner_id, "principal_id": value.principal_id,
        "source_episode_id": value.source_episode_id, "source_node_id": value.source_node_id,
        "content_source_conversation_id": value.content_source_conversation_id,
        "content_source_node_id": value.content_source_node_id,
        "grounding_source_conversation_id": value.grounding_source_conversation_id,
        "grounding_source_node_id": value.grounding_source_node_id,
    })
}

fn memory_temporal_value(value: &reliquary_memory_core::MemoryTemporalInference) -> Value {
    json!({
        "body_id": value.body_id.0, "source_time_ns": value.source_time_ns,
        "model": value.inference.model, "contract_version": value.inference.contract_version,
        "resolutions": value.inference.resolutions.iter().map(|resolution| json!({
            "start_byte": resolution.start_byte, "end_byte": resolution.end_byte,
            "kind": temporal_kind(resolution.kind), "evidence": resolution.evidence,
            "canonical_expression": resolution.canonical_expression,
        })).collect::<Vec<_>>(),
    })
}

fn temporal_kind(value: reliquary_memory_core::TemporalIndicationKind) -> &'static str {
    use reliquary_memory_core::TemporalIndicationKind::*;
    match value {
        Explicit => "explicit",
        Calendar => "calendar",
        Relative => "relative",
        Boundary => "boundary",
        Recurrence => "recurrence",
        Duration => "duration",
    }
}

fn perception_value(value: &reliquary_perception_store::PerceptionHistorySnapshot) -> Value {
    json!({
        "perception_version": value.perception_version,
        "entity_revisions": value.entity_revisions.iter().map(|revision| json!({
            "perception_version": revision.perception_version,
            "value": entity_value(&revision.value),
        })).collect::<Vec<_>>(),
        "entity_tombstones": value.entity_tombstones.iter().map(|terminal| json!({
            "id": terminal.id.0, "revision": terminal.revision,
            "perception_version": terminal.perception_version,
        })).collect::<Vec<_>>(),
        "entity_description_prunes": value.entity_description_prunes.iter().map(|proof| json!({
            "id": proof.id.0, "revision": proof.revision,
            "summary_digest": proof.summary_digest, "manifest_digest": proof.manifest_digest,
        })).collect::<Vec<_>>(),
        "mention_revisions": value.mention_revisions.iter().map(|revision| json!({
            "perception_version": revision.perception_version,
            "value": mention_value(&revision.value),
        })).collect::<Vec<_>>(),
        "relationship_revisions": value.relationship_revisions.iter().map(|revision| json!({
            "perception_version": revision.perception_version,
            "value": relationship_value(&revision.value),
        })).collect::<Vec<_>>(),
        "relationship_tombstones": value.relationship_tombstones.iter().map(|terminal| json!({
            "id": terminal.id.0, "revision": terminal.revision,
            "relationship_version": terminal.relationship_version,
        })).collect::<Vec<_>>(),
        "relationship_description_prunes": value.relationship_description_prunes.iter().map(|proof| json!({
            "id": proof.id.0, "revision": proof.revision,
            "summary_digest": proof.summary_digest, "manifest_digest": proof.manifest_digest,
        })).collect::<Vec<_>>(),
    })
}

fn entity_value(value: &reliquary_perception_store::Entity) -> Value {
    json!({
        "id": value.id.0, "revision": value.revision,
        "canonical_name": value.canonical_name, "aliases": value.aliases,
        "kind": value.kind, "summary": value.summary, "mutation_id": value.mutation_id,
        "created_at_ns": value.created_at_ns, "updated_at_ns": value.updated_at_ns,
    })
}

fn mention_value(value: &MemoryEntityResolution) -> Value {
    json!({
        "key": {"memory_id": value.key.memory_id, "field": match value.key.field {
            reliquary_perception_store::MemoryTextField::Title => "title",
            reliquary_perception_store::MemoryTextField::Content => "content",
        }, "start_byte": value.key.start_byte, "end_byte": value.key.end_byte},
        "revision": value.revision, "updated_at_ns": value.updated_at_ns,
        "status": mention_status_value(&value.status),
    })
}

fn mention_status_value(value: &reliquary_perception_store::MemoryEntityResolutionStatus) -> Value {
    use reliquary_perception_store::MemoryEntityResolutionStatus::*;
    match value {
        Resolved { entity_id, reason } => json!({
            "state": "resolved", "entity_id": entity_id.0, "reason": reason_value(*reason),
        }),
        Ambiguous { candidates } => json!({
            "state": "ambiguous", "candidates": candidates.iter().map(|id| id.0).collect::<Vec<_>>(),
        }),
        Unresolved { reason } => json!({"state": "unresolved", "reason": reason_value(*reason)}),
    }
}

fn reason_value(value: reliquary_perception_store::EntityResolutionReason) -> &'static str {
    use reliquary_perception_store::EntityResolutionReason::*;
    match value {
        ContextMatch => "context_match",
        ContextConflictNewIdentity => "context_conflict_new_identity",
        NamedReferent => "named_referent",
        PersistentArtifact => "persistent_artifact",
        DescriptiveIdentity => "descriptive_identity",
        FirstSeenIdentity => "first_seen_identity",
        InsufficientEvidence => "insufficient_evidence",
        GenericRole => "generic_role",
        AbstractProcess => "abstract_process",
        TransientValue => "transient_value",
        SentenceLocal => "sentence_local",
        WrapperCategory => "wrapper_category",
        Ambiguous => "ambiguous",
        RecurrenceRequired => "recurrence_required",
        PrincipalIdentity => "principal_identity",
        UserWithdrawal => "user_withdrawal",
    }
}

fn relationship_value(value: &reliquary_perception_store::Relationship) -> Value {
    json!({
        "id": value.id.0, "revision": value.revision, "kind": value.kind,
        "participants": value.participants.iter().map(|participant| json!({
            "owner_id": participant.entity.owner_id, "entity_id": participant.entity.entity_id.0,
            "role": participant.role,
        })).collect::<Vec<_>>(),
        "evidence": value.evidence.iter().map(|evidence| json!({
            "owner_id": evidence.owner_id, "memory_id": evidence.memory_id,
        })).collect::<Vec<_>>(),
        "summary": value.summary, "mutation_id": value.mutation_id,
        "created_at_ns": value.created_at_ns, "updated_at_ns": value.updated_at_ns,
        "relationship_version": value.relationship_version,
    })
}

fn relation_value(value: &Relation) -> Value {
    json!({
        "source": node_ref_value(value.source), "target": node_ref_value(value.target),
        "kind": relation_kind_value(value.kind), "active": value.active,
        "origin": match value.origin {
            reliquary_graph::RelationOrigin::Dream => "dream",
            reliquary_graph::RelationOrigin::User => "user",
            reliquary_graph::RelationOrigin::Perception => "perception",
        },
        "graph_version": value.graph_version,
    })
}

fn node_ref_value(value: NodeRef) -> Value {
    match value {
        NodeRef::Memory(id) => json!({"kind": "memory", "id": id}),
        NodeRef::Entity(id) => json!({"kind": "entity", "id": id}),
    }
}

fn relation_kind_value(value: reliquary_graph::RelationKind) -> &'static str {
    use reliquary_graph::RelationKind::*;
    match value {
        Topical => "topical",
        Factual => "factual",
        Causal => "causal",
        Recurrent => "recurrent",
        References => "references",
        DuplicateOf => "duplicate_of",
        Supersedes => "supersedes",
        StructuralParent => "structural_parent",
        EntityAssociation => "entity_association",
        PrincipalAssociation => "principal_association",
    }
}

fn community_value(value: &CommunitySnapshot) -> Value {
    json!({
        "generation": value.generation, "graph_version": value.graph_version,
        "communities": value.communities.iter().map(|community| json!({
            "id": community.id.0, "members": community.members,
        })).collect::<Vec<_>>(),
    })
}

fn ego_record_value(value: &reliquary_ego_store::EgoHistoryRecord) -> Value {
    use reliquary_ego_store::EgoHistoryRecord::*;
    match value {
        Identity {
            ego_version,
            id,
            revision,
            deleted,
            name,
            text,
        } => json!({
            "kind": "identity", "ego_version": ego_version, "id": id.0,
            "revision": revision, "deleted": deleted, "name": name, "text": text,
        }),
        Personality {
            ego_version,
            revision,
            source_memory_version,
            text,
        } => json!({
            "kind": "personality", "ego_version": ego_version, "revision": revision,
            "source_memory_version": source_memory_version, "text": text,
        }),
        Anchor {
            ego_version,
            id,
            revision,
            deleted,
            priority,
            text,
        } => json!({
            "kind": "anchor", "ego_version": ego_version, "id": id.0,
            "revision": revision, "deleted": deleted,
            "priority": match priority {
                reliquary_ego_store::EgoAnchorPriority::High => "high",
                reliquary_ego_store::EgoAnchorPriority::Normal => "normal",
                reliquary_ego_store::EgoAnchorPriority::Low => "low",
            }, "text": text,
        }),
        Synthesis {
            ego_version,
            revision,
            source_memory_version,
            text,
        } => json!({
            "kind": "synthesis", "ego_version": ego_version, "revision": revision,
            "source_memory_version": source_memory_version, "text": text,
        }),
    }
}

fn generation_value(value: &VectorGeneration) -> Value {
    json!({
        "id": value.id.0, "compatibility_profile_id": value.compatibility_profile_id.0,
        "archive_vector_id": value.archive_vector_id.0,
        "source_archive_version": value.source_archive_version,
        "vector_version": value.vector_version,
    })
}

fn archive_value(value: &reliquary_archive::ArchiveHistorySnapshot) -> Value {
    json!({
        "archive_version": value.archive_version, "activity_cut": value.activity_cut,
        "turns": value.turns.iter().map(archive_turn_value).collect::<Vec<_>>(),
        "branch_revisions": value.branch_revisions.iter().map(|revision| json!({
            "archive_version": revision.archive_version, "branch": branch_value(&revision.branch),
        })).collect::<Vec<_>>(),
        "episodes": value.episodes.iter().map(episode_value).collect::<Vec<_>>(),
        "import_receipts": value.import_receipts.iter().map(import_receipt_value).collect::<Vec<_>>(),
        "fragment_definitions": value.fragment_definitions.iter().map(fragment_value).collect::<Vec<_>>(),
        "conversation_tombstones": value.conversation_tombstones.iter().map(|terminal| json!({
            "conversation_id": terminal.conversation_id, "archive_version": terminal.archive_version,
        })).collect::<Vec<_>>(),
    })
}

fn archive_turn_value(value: &reliquary_archive::HistoricalArchiveTurn) -> Value {
    let node = &value.node;
    json!({
        "node": {"id": node.id, "conversation_id": node.conversation_id,
            "parent_id": node.parent_id, "role": node.role, "principal_id": node.principal_id,
            "timestamp_ns": node.timestamp_ns, "content_id": node.content_id.0},
        "content": value.content,
        "attachments": value.attachments.iter().map(source_attachment_value).collect::<Vec<_>>(),
        "activity_position": value.activity_position,
    })
}

fn source_attachment_value(value: &reliquary_archive::SourceAttachment) -> Value {
    json!({
        "id": value.id, "content_id": value.content_id.0, "filename": value.filename,
        "mime_type": value.mime_type, "byte_length": value.byte_length, "bytes": value.bytes,
        "project_reference": value.project_reference.as_ref().map(project_file_value),
    })
}

fn project_file_value(value: &reliquary_archive::ProjectFileRef) -> Value {
    json!({
        "repository": {"kind": match value.revision.repository.kind {
            reliquary_archive::ProjectRepositoryKind::Lore => "lore",
            reliquary_archive::ProjectRepositoryKind::Git => "git",
        }, "repository_id": value.revision.repository.repository_id,
        "project_path": value.revision.repository.project_path},
        "revision": value.revision.revision, "path": value.path,
        "content_hash": value.content_hash,
    })
}

fn branch_value(value: &reliquary_archive::Branch) -> Value {
    json!({"id": value.id, "conversation_id": value.conversation_id,
        "leaf_node_id": value.leaf_node_id, "canonical": value.canonical})
}

fn episode_value(value: &reliquary_archive::Episode) -> Value {
    json!({
        "id": value.id.0, "conversation_id": value.conversation_id,
        "start_node_id": value.start_node_id, "end_node_id": value.end_node_id,
        "origin": match value.origin {
            reliquary_archive::EpisodeOrigin::Live => "live",
            reliquary_archive::EpisodeOrigin::Import => "import",
        },
        "boundary": match value.boundary {
            reliquary_archive::EpisodeBoundary::Size => "size",
            reliquary_archive::EpisodeBoundary::Inactivity => "inactivity",
            reliquary_archive::EpisodeBoundary::CreateMemory => "create_memory",
            reliquary_archive::EpisodeBoundary::ImportEnd => "import_end",
            reliquary_archive::EpisodeBoundary::Explicit => "explicit",
        },
        "source_through_ns": value.source_through_ns, "finalized_at_ns": value.finalized_at_ns,
    })
}

fn import_receipt_value(value: &reliquary_archive::ImportReceipt) -> Value {
    json!({
        "schema_version": value.schema_version, "owner_uuid": value.owner_uuid,
        "import_key": value.import_key, "source_namespace": value.source_namespace,
        "source_format": value.source_format, "source_digest": value.source_digest,
        "conversations": value.conversations.iter().map(|conversation| json!({
            "source_id": conversation.source_id, "conversation_id": conversation.conversation_id,
            "title": conversation.title,
            "messages": conversation.messages.iter().map(|message| json!({
                "source_id": message.source_id, "revision": message.revision,
                "source_parent_id": message.source_parent_id, "replaces": message.replaces,
                "deleted": message.deleted, "node_id": message.node_id,
                "fingerprint": message.fingerprint,
                "artifacts": message.artifacts.iter().map(|artifact| json!({
                    "source_id": artifact.source_id, "source_ref": artifact.source_ref,
                    "sha256": artifact.sha256, "file_id": artifact.file_id,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "branches": conversation.branches.iter().map(|branch| json!({
                "source_id": branch.source_id, "branch_id": branch.branch_id,
                "leaf_node_id": branch.leaf_node_id, "canonical": branch.canonical,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "receipt_digest": value.receipt_digest,
    })
}

fn fragment_value(value: &reliquary_archive::FragmentDefinition) -> Value {
    json!({
        "fragment": {"id": value.fragment.id.0,
            "conversation_id": value.fragment.conversation_id,
            "start_node_id": value.fragment.start_node_id,
            "end_node_id": value.fragment.end_node_id},
        "branch_id": value.branch_id, "archive_cut": value.archive_cut,
        "config": {"turns": value.config.turns, "overlap": value.config.overlap},
        "close_tail": value.close_tail,
    })
}

fn freshness_value(
    records: &BTreeMap<FreshnessMemoryId, FreshnessRecord>,
    evidence: &[FreshnessReplayEntry],
) -> Value {
    json!({
        "records": records.iter().map(|(id, record)| json!({
            "memory_id": id.0, "score": record.score,
            "admitted_at_turn": record.admitted_at_turn, "accounted_turn": record.accounted_turn,
        })).collect::<Vec<_>>(),
        "evidence": evidence.iter().map(freshness_evidence_value).collect::<Vec<_>>(),
    })
}

fn freshness_evidence_value(value: &FreshnessReplayEntry) -> Value {
    match value {
        FreshnessReplayEntry::Birth {
            memory_id,
            accepted_turn,
            admitted_at_birth,
        } => json!({
            "kind": "birth", "memory_id": memory_id.0, "accepted_turn": accepted_turn,
            "admitted_at_birth": admitted_at_birth,
        }),
        FreshnessReplayEntry::Admission {
            memory_id,
            accepted_turn,
        } => json!({
            "kind": "admission", "memory_id": memory_id.0, "accepted_turn": accepted_turn,
        }),
        FreshnessReplayEntry::Event(event) => json!({
            "kind": "event", "event_id": event.event_id, "accepted_turn": event.accepted_turn,
            "provenance": {"producer_kind": event.provenance.producer_kind,
                "source_id": event.provenance.source_id, "origin_owner": event.provenance.origin_owner,
                "accepted_turn": event.provenance.accepted_turn,
                "policy_version": event.provenance.policy_version},
            "proof": freshness_proof_value(event.proof),
            "effects": event.effects.iter().map(|(id, amount)| json!({
                "memory_id": id.0, "amount": amount,
            })).collect::<Vec<_>>(),
        }),
        FreshnessReplayEntry::AcceptedUse(receipt, proof) => json!({
            "kind": "accepted_use", "use_id": receipt.use_id,
            "accepted_turn": receipt.accepted_turn,
            "sources": receipt.sources.iter().map(|id| id.0).collect::<Vec<_>>(),
            "policy_version": receipt.policy_version, "proof": freshness_proof_value(*proof),
        }),
    }
}

fn freshness_proof_value(value: reliquary_freshness_store::FreshnessEventProof) -> Value {
    json!({"graph_version": value.graph_version,
        "community_generation": value.community_generation,
        "community_graph_version": value.community_graph_version})
}
