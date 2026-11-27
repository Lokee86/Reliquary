//! Capture and discover Ego topics from the active PHY's settled Memories.

use super::*;
use reliquary_ego::EgoContract;
use reliquary_memory_audit::{
    AuditError, AuditMode, Batch, BatchLimits, GraphLink, LookupPassage, MAX_MEMORIES,
    MAX_PASSAGES_PER_LOOKUP, WorkItem, plan_batches_in_mode, run_batch_with_lookup,
};
use reliquary_owner_session::{
    AuditDiscoveryCursor, OwnerSessionError, PhyAuditCut, PhyWriter,
    model::{MemoryBody, MemoryRecord, NodeRef},
};
use reliquary_perception::{ObservationProfile, PerceptionContract};

const INACTIVE_PERCEPTION: PerceptionContract = PerceptionContract {
    profile: ObservationProfile::Ordinary,
};

pub struct PhyAuditDiscovery {
    pub key: [u8; 32],
    pub next: AuditDiscoveryCursor,
    batches: Vec<Batch>,
}

impl PhyAuditDiscovery {
    pub fn batch_count(&self) -> usize {
        self.batches.len()
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
    mut cursor: AuditDiscoveryCursor,
    route_identity: [u8; 32],
    limits: BatchLimits,
) -> Result<PhyAuditDiscovery, OwnerSessionError> {
    if usize::from(cursor.source) > sources.len() || sources.len() > MAX_MEMORIES {
        return Err(OwnerSessionError::ManagementLimit);
    }
    if !cut.is_at_or_before(phy.phy_audit_cut()) {
        return Err(OwnerSessionError::ManagementStale);
    }
    let mut work = Vec::new();
    let mut bytes = 0;
    while usize::from(cursor.source) < sources.len()
        && bytes + 4 <= discovery::MAX_CAPTURE_TEXT_BYTES
    {
        let id = sources[usize::from(cursor.source)];
        let Some((record, body)) = phy.phy_audit_evidence(cut, id)? else {
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
            work.push(WorkItem {
                passage: passage(cut, &record, cursor.field, start, &text[start..end]),
                mandatory: Eligibility::EGO,
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
    let mut digest = Sha256::new();
    digest.update(b"reliquary-phy-audit-capture-v1\0");
    digest.update(cut.owner_id);
    digest.update(cut.history_generation);
    digest.update(cut.memory.to_le_bytes());
    digest.update(cut.graph.to_le_bytes());
    digest.update(cut.settlements.to_le_bytes());
    digest.update((limits.max_input_bytes as u64).to_le_bytes());
    digest.update((limits.max_response_bytes as u64).to_le_bytes());
    digest.update(discovery::capture_key(route_identity, 0, &work, &[])?);
    let batches = plan_batches_in_mode(
        owner_cut(cut),
        AuditMode::PhySettled,
        &work,
        &[],
        limits,
        &EgoContract,
        &INACTIVE_PERCEPTION,
    )
    .map_err(|_| OwnerSessionError::ManagementLimit)?;
    Ok(PhyAuditDiscovery {
        key: digest.finalize().into(),
        next: cursor,
        batches,
    })
}

/// Run inference outside of the owner writer lease. The callback reacquires
/// the authorized owner read for every optional request.
pub fn discover_phy(
    capture: &PhyAuditDiscovery,
    batch: usize,
    endpoint: &dyn GeneralEndpoint,
    lookup: &mut impl FnMut(LookupRequest<'_>) -> Result<Vec<LookupPassage>, LookupError>,
) -> Result<ListResult<EgoCandidate>, AuditError> {
    let batch = capture.batches.get(batch).ok_or(AuditError::InvalidInput)?;
    let result = run_batch_with_lookup(
        batch,
        endpoint,
        &EgoContract,
        &INACTIVE_PERCEPTION,
        &mut |_: &OwnerCut, _, request| lookup(request),
    )?;
    Ok(result.ego)
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
        let (field, text) = if body.content.is_empty() {
            (MemoryTextField::Title, &body.title)
        } else {
            (MemoryTextField::Content, &body.content)
        };
        let end = text.floor_char_boundary(discovery::MAX_LOOKUP_PASSAGE_BYTES.min(text.len()));
        if end != 0 {
            passages.push(LookupPassage {
                passage: passage(cut, &record, field, 0, &text[..end]),
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

fn passage(
    cut: PhyAuditCut,
    record: &MemoryRecord,
    field: MemoryTextField,
    start: usize,
    text: &str,
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
        eligible: Eligibility::EGO,
        text: text.to_owned(),
    }
}
