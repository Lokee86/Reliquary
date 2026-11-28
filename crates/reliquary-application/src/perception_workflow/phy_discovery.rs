//! Shared discovery over exact settled evidence from the active PHY.

use super::*;
use reliquary_ego::EgoContract;
use reliquary_memory_audit::{
    AuditError, AuditMode, Batch, BatchLimits, GraphLink, LookupPassage, MAX_MEMORIES,
    MAX_PASSAGES_PER_LOOKUP, Participant, WorkItem, plan_batches_in_mode, run_batch_for_features,
};
use reliquary_owner_session::observation::ObservationStanding;
use reliquary_owner_session::{
    AuditDiscoveryCursor, OwnerSessionError, PerceptionMemorySource, PhyAuditCut,
    PhyObservationCut, PhyWriter,
    model::{MemoryBody, MemoryRecord, NodeRef},
};
use reliquary_perception::{ObservationProfile, PerceptionContract};
use std::collections::BTreeSet;

pub struct PhyAuditDiscovery {
    pub key: [u8; 32],
    pub next: AuditDiscoveryCursor,
    batches: Vec<Batch>,
    contract: PerceptionContract,
}

impl PhyAuditDiscovery {
    pub fn batch_count(&self) -> usize {
        self.batches.len()
    }

    pub fn batch_features(&self, ordinal: usize) -> Option<Eligibility> {
        self.batches.get(ordinal).map(Batch::active_features)
    }
}

fn owner_cut(cut: PhyAuditCut) -> OwnerCut {
    OwnerCut {
        owner_id: cut.owner_id,
        history_generation: cut.history_generation,
        frontier: cut.memory,
    }
}

/// Capture bounded, exact slices from an ordered PHY page. Persist the cursor
/// only after every accepted batch in this capture has a durable handoff.
pub fn capture_phy_discovery(
    phy: &PhyWriter,
    cut: PhyAuditCut,
    sources: &[MemoryId],
    cursor: AuditDiscoveryCursor,
    route_identity: [u8; 32],
    limits: BatchLimits,
) -> Result<PhyAuditDiscovery, OwnerSessionError> {
    capture_phy_perception_discovery(
        phy,
        PhyObservationCut {
            audit: cut,
            ..phy.phy_observation_cut()
        },
        sources,
        &[],
        cursor,
        route_identity,
        limits,
        Eligibility::EGO,
    )
}

/// The durable source cursor runs through Memories, then derived Observations.
/// Optional context does not advance either list's source coverage.
pub fn capture_phy_perception_discovery(
    phy: &PhyWriter,
    cut: PhyObservationCut,
    sources: &[MemoryId],
    derived: &[ObservationId],
    mut cursor: AuditDiscoveryCursor,
    route_identity: [u8; 32],
    limits: BatchLimits,
    features: Eligibility,
) -> Result<PhyAuditDiscovery, OwnerSessionError> {
    let total = sources.len() + derived.len();
    if usize::from(cursor.source) > total
        || sources.len() > MAX_MEMORIES
        || derived.len() > MAX_MEMORIES
        || (!derived.is_empty() && !features.perception)
        || features == Eligibility::NONE
    {
        return Err(OwnerSessionError::ManagementLimit);
    }
    check_cut(phy, cut)?;
    let source_cut: PerceptionSourceCut = cut.into();
    let audit_cut = cut.audit;
    let mut work = Vec::new();
    let mut participant_ids = BTreeSet::new();
    let mut participants = Vec::new();
    let mut local_sources = BTreeSet::new();
    let mut bytes = 0;
    while usize::from(cursor.source) < total && bytes + 4 <= discovery::MAX_CAPTURE_TEXT_BYTES {
        let index = usize::from(cursor.source);
        let Some(&id) = sources.get(index) else {
            let id = derived[index - sources.len()];
            if let Some(head) = phy.observation(id).filter(|head| {
                head.standing == ObservationStanding::Current
                    && head.revision.publication_version <= cut.observation
            }) {
                let text = &head.revision.draft.proposition;
                if bytes + text.len() > discovery::MAX_CAPTURE_TEXT_BYTES {
                    break;
                }
                work.push(WorkItem {
                    passage: Passage {
                        citation: audit_citation(head.revision.full_citation()),
                        grade: Grade::Settled,
                        eligible: Eligibility::PERCEPTION,
                        text: text.clone(),
                    },
                    mandatory: Eligibility::PERCEPTION,
                });
                bytes += text.len();
            }
            cursor = discovery::next_source(cursor);
            continue;
        };
        let Some((record, body)) = phy.phy_audit_evidence(audit_cut, id)? else {
            cursor = discovery::next_source(cursor);
            continue;
        };
        let text = field_text(&body, cursor.field);
        let start = cursor.byte as usize;
        if !text.is_char_boundary(start) {
            return Err(OwnerSessionError::ManagementStale);
        }
        let end = text.floor_char_boundary(
            (start + discovery::MAX_CAPTURE_TEXT_BYTES - bytes).min(text.len()),
        );
        if start < end {
            if features.perception {
                take_participants(
                    phy,
                    &source_cut,
                    cut.retained_resolution,
                    &record,
                    &mut participant_ids,
                    &mut participants,
                )?;
                for (neighbor, _) in phy.phy_audit_graph_links(audit_cut, id)? {
                    if local_sources.len() == discovery::MAX_LOCAL_SOURCES {
                        break;
                    }
                    local_sources.insert(neighbor);
                }
            }
            work.push(WorkItem {
                passage: passage(
                    audit_cut,
                    &record,
                    cursor.field,
                    start,
                    &text[start..end],
                    features,
                ),
                mandatory: features,
            });
            bytes += end - start;
        }
        if end != text.len() {
            cursor.byte = end as u32;
            break;
        }
        cursor = match cursor.field {
            MemoryTextField::Title => AuditDiscoveryCursor {
                source: cursor.source,
                field: MemoryTextField::Content,
                byte: 0,
            },
            MemoryTextField::Content => discovery::next_source(cursor),
        };
    }
    for entity in &participants {
        for neighbor in phy.perception_relationship_sources(
            reliquary_owner_session::model::EntityId(entity.entity_id),
        ) {
            if local_sources.len() == discovery::MAX_LOCAL_SOURCES {
                break;
            }
            local_sources.insert(neighbor);
        }
    }
    let captured: BTreeSet<_> = work
        .iter()
        .map(|item| item.passage.citation.source)
        .collect();
    for id in local_sources {
        if captured.contains(&SourceId::Memory(id.0))
            || bytes + discovery::MAX_LOCAL_PASSAGE_BYTES > discovery::MAX_CAPTURE_TEXT_BYTES
        {
            continue;
        }
        let Some((record, body)) = phy.phy_audit_evidence(audit_cut, id)? else {
            continue;
        };
        let (field, text) = preferred_field(&body);
        let end = text.floor_char_boundary(discovery::MAX_LOCAL_PASSAGE_BYTES.min(text.len()));
        if end != 0 {
            take_participants(
                phy,
                &source_cut,
                cut.retained_resolution,
                &record,
                &mut participant_ids,
                &mut participants,
            )?;
            bytes += end;
            work.push(WorkItem {
                passage: passage(
                    audit_cut,
                    &record,
                    field,
                    0,
                    &text[..end],
                    Eligibility::PERCEPTION,
                ),
                mandatory: Eligibility::NONE,
            });
        }
    }
    let contract = PerceptionContract {
        profile: if participants.len() >= 2 {
            ObservationProfile::Relational
        } else {
            ObservationProfile::Ordinary
        },
    };
    let mut digest = Sha256::new();
    if features.perception {
        digest.update(b"reliquary-phy-perception-audit-capture-v1\0");
        digest.update(cut.entity.to_le_bytes());
        digest.update([u8::from(cut.retained_resolution.is_some())]);
        digest.update(cut.retained_resolution.unwrap_or_default().to_le_bytes());
        digest.update(cut.observation.to_le_bytes());
    } else {
        digest.update(b"reliquary-phy-audit-capture-v1\0");
    }
    digest.update(audit_cut.owner_id);
    digest.update(audit_cut.history_generation);
    digest.update(audit_cut.memory.to_le_bytes());
    digest.update(audit_cut.graph.to_le_bytes());
    digest.update(audit_cut.settlements.to_le_bytes());
    digest.update((limits.max_input_bytes as u64).to_le_bytes());
    digest.update((limits.max_response_bytes as u64).to_le_bytes());
    digest.update(discovery::capture_key(
        route_identity,
        source_cut.source_removal,
        &work,
        &participants,
    )?);
    let batches = plan_batches_in_mode(
        owner_cut(audit_cut),
        AuditMode::PhySettled,
        &work,
        &participants,
        limits,
        &EgoContract,
        &contract,
    )
    .map_err(|_| OwnerSessionError::ManagementLimit)?;
    Ok(PhyAuditDiscovery {
        key: digest.finalize().into(),
        next: cursor,
        batches,
        contract,
    })
}

pub(super) fn check_cut(phy: &PhyWriter, cut: PhyObservationCut) -> Result<(), OwnerSessionError> {
    let current = phy.phy_observation_cut();
    if !cut.audit.is_at_or_before(current.audit)
        || cut.entity > current.entity
        || cut.retained_resolution > current.retained_resolution
        || cut.observation > current.observation
    {
        return Err(OwnerSessionError::ManagementStale);
    }
    Ok(())
}

fn take_participants(
    phy: &PhyWriter,
    cut: &PerceptionSourceCut,
    retained_resolution: Option<u64>,
    record: &MemoryRecord,
    seen: &mut BTreeSet<[u8; 32]>,
    slots: &mut Vec<Participant>,
) -> Result<(), OwnerSessionError> {
    if slots.len() < discovery::MAX_PARTICIPANT_SLOTS {
        let source = PerceptionMemorySource {
            id: record.id,
            revision: record.revision,
            body_id: record.body_id,
        };
        for anchor in phy.perception_participant_evidence(cut, source)? {
            if anchor.retained_ordinal <= retained_resolution && seen.insert(anchor.entity_id.0) {
                slots.push(Participant {
                    owner_id: cut.owner_id,
                    entity_id: anchor.entity_id.0,
                    revision: anchor.entity_revision,
                    label: anchor.canonical_name,
                });
                if slots.len() == discovery::MAX_PARTICIPANT_SLOTS {
                    break;
                }
            }
        }
    }
    Ok(())
}

/// Run inference outside of the owner writer lease. The callback reacquires
/// the authorized owner read for every optional request.
pub fn discover_phy(
    capture: &PhyAuditDiscovery,
    batch: usize,
    endpoint: &dyn GeneralEndpoint,
    lookup: &mut impl FnMut(LookupRequest<'_>) -> Result<Vec<LookupPassage>, LookupError>,
) -> Result<ListResult<EgoCandidate>, AuditError> {
    Ok(discover_phy_for_features(capture, batch, Eligibility::EGO, endpoint, lookup)?.ego)
}

/// Discover the selected lists together, leaving acknowledged lists inactive.
pub fn discover_phy_for_features(
    capture: &PhyAuditDiscovery,
    batch: usize,
    features: Eligibility,
    endpoint: &dyn GeneralEndpoint,
    lookup: &mut impl FnMut(LookupRequest<'_>) -> Result<Vec<LookupPassage>, LookupError>,
) -> Result<AuditDiscoveryResult, AuditError> {
    let batch = capture.batches.get(batch).ok_or(AuditError::InvalidInput)?;
    run_batch_for_features(
        batch,
        endpoint,
        &EgoContract,
        &capture.contract,
        features,
        &mut |_: &OwnerCut, _, request: LookupRequest<'_>| lookup(request),
    )
}

pub fn phy_audit_lookups(
    phy: &PhyWriter,
    cut: PhyAuditCut,
    request: LookupRequest<'_>,
) -> Result<Vec<LookupPassage>, OwnerSessionError> {
    let hits: Vec<_> = match request {
        LookupRequest::Lexical(query) => phy
            .phy_audit_lexical_candidates(cut, query, discovery::MAX_LOOKUP_INPUTS)?
            .into_iter()
            .map(|id| (id, None))
            .collect(),
        LookupRequest::Graph { anchor } => {
            phy_audit_citation(phy, cut, anchor)?;
            let SourceId::Memory(anchor_id) = anchor.source else {
                return Err(OwnerSessionError::ManagementEvidence);
            };
            phy.phy_audit_graph_links(cut, MemoryId(anchor_id))?
                .into_iter()
                .map(|(id, edge)| {
                    (
                        id,
                        Some(GraphLink {
                            kind: format!("{:?}", edge.kind),
                            outgoing: edge.source == NodeRef::Memory(anchor_id),
                        }),
                    )
                })
                .collect()
        }
    };
    let mut passages = Vec::new();
    for (id, link) in hits {
        let Some((record, body)) = phy.phy_audit_evidence(cut, id)? else {
            continue;
        };
        let (field, text) = preferred_field(&body);
        let end = text.floor_char_boundary(discovery::MAX_LOOKUP_PASSAGE_BYTES.min(text.len()));
        if end != 0 {
            passages.push(LookupPassage {
                passage: passage(cut, &record, field, 0, &text[..end], Eligibility::BOTH),
                link,
            });
            if passages.len() == MAX_PASSAGES_PER_LOOKUP {
                break;
            }
        }
    }
    Ok(passages)
}

/// Return exact captured text and its owner-admitted timing metadata, provided
/// the revision still stands at the current PHY head.
pub fn phy_audit_citation(
    phy: &PhyWriter,
    cut: PhyAuditCut,
    citation: SourceCitation,
) -> Result<(MemoryRecord, String), OwnerSessionError> {
    let SourceId::Memory(id) = citation.source else {
        return Err(OwnerSessionError::ManagementEvidence);
    };
    if citation.owner_id != cut.owner_id || citation.history_generation != cut.history_generation {
        return Err(OwnerSessionError::ManagementStale);
    }
    let (record, body) = phy
        .phy_audit_evidence(cut, MemoryId(id))?
        .filter(|(record, _)| {
            record.revision == citation.revision && record.body_id.0 == citation.body_id
        })
        .ok_or(OwnerSessionError::ManagementStale)?;
    let field = match citation.field {
        TextField::Title => MemoryTextField::Title,
        TextField::Content => MemoryTextField::Content,
    };
    let text = field_text(&body, field)
        .get(citation.start_byte as usize..citation.end_byte as usize)
        .filter(|text| !text.is_empty())
        .ok_or(OwnerSessionError::ManagementEvidence)?;
    Ok((record, text.to_owned()))
}

fn field_text(body: &MemoryBody, field: MemoryTextField) -> &str {
    match field {
        MemoryTextField::Title => &body.title,
        MemoryTextField::Content => &body.content,
    }
}

fn preferred_field(body: &MemoryBody) -> (MemoryTextField, &str) {
    let field = if body.content.is_empty() {
        MemoryTextField::Title
    } else {
        MemoryTextField::Content
    };
    (field, field_text(body, field))
}

fn passage(
    cut: PhyAuditCut,
    record: &MemoryRecord,
    field: MemoryTextField,
    start: usize,
    text: &str,
    eligible: Eligibility,
) -> Passage {
    Passage {
        citation: SourceCitation {
            owner_id: cut.owner_id,
            history_generation: cut.history_generation,
            source: SourceId::Memory(record.id.0),
            revision: record.revision,
            body_id: record.body_id.0,
            field: match field {
                MemoryTextField::Title => TextField::Title,
                MemoryTextField::Content => TextField::Content,
            },
            start_byte: start as u32,
            end_byte: (start + text.len()) as u32,
        },
        grade: Grade::Settled,
        eligible,
        text: text.to_owned(),
    }
}
