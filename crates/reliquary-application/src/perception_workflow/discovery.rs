use super::*;
use reliquary_ego::{EgoCandidate, EgoContract};
use reliquary_memory_audit::{
    AuditError, Batch, BatchLimits, BatchResult, CONTRACT_VERSION, Participant, WorkItem,
    plan_batches, run_batch,
};
use reliquary_owner_session::observation::ObservationStanding;
use reliquary_owner_session::{
    AuditDiscoveryCursor, OwnerSessionError, PerceptionAuditPage, RelSession,
};
use reliquary_perception::{ObservationProfile, PerceptionContract};
use std::collections::BTreeSet;

// Leave room for schema, source coordinates, participant context and wire
// encoding. The auditor enforces the actual serialized route limit.
const MAX_CAPTURE_TEXT_BYTES: usize = 96 * 1024;
const MAX_PARTICIPANT_SLOTS: usize = 16;
const MAX_LOOKUP_INPUTS: usize = 32;
const MAX_LOOKUP_PASSAGE_BYTES: usize = 2 * 1024;
const MAX_LOCAL_SOURCES: usize = 8;
const MAX_LOCAL_PASSAGE_BYTES: usize = 1024;

pub struct AuditDiscovery {
    pub key: [u8; 32],
    pub next: AuditDiscoveryCursor,
    contract: PerceptionContract,
    batches: Vec<Batch>,
}

pub type AuditDiscoveryResult = BatchResult<EgoCandidate, ObservationProposal>;

impl AuditDiscovery {
    pub fn batch_count(&self) -> usize {
        self.batches.len()
    }
}

/// Read only enough source text to plan the next bounded discovery request
/// group. Discovery cursors record exact offsets and survive a process restart.
pub fn capture_discovery(
    rel: &RelSession,
    page: &PerceptionAuditPage,
    route_identity: [u8; 32],
) -> Result<AuditDiscovery, OwnerSessionError> {
    if page.cut.source_removal != rel.archive_source_removal_version() {
        return Err(OwnerSessionError::ManagementStale);
    }
    let mut cursor = page.discovery;
    let mut work: Vec<WorkItem> = Vec::new();
    let mut participants = Vec::new();
    let mut participant_ids = BTreeSet::new();
    let mut local_sources = BTreeSet::new();
    let mut bytes = 0usize;
    let total = page.sources.len() + page.derived.len();
    while usize::from(cursor.source) < total && bytes + 4 <= MAX_CAPTURE_TEXT_BYTES {
        let position = usize::from(cursor.source);
        if let Some(source) = page.sources.get(position) {
            let Some(evidence) = rel
                .perception_memory_evidence(&page.cut, &[source.memory])?
                .pop()
            else {
                cursor = next_source(cursor);
                continue;
            };
            if !source.ego_only {
                take_participants(
                    rel,
                    &page.cut,
                    evidence.metadata.source,
                    &mut participant_ids,
                    &mut participants,
                )?;
                for neighbor in rel.perception_graph_neighbors(source.memory) {
                    if local_sources.len() == MAX_LOCAL_SOURCES {
                        break;
                    }
                    local_sources.insert(neighbor);
                }
            }
            // A single source may have another field or extend past the
            // capture budget; continue it at the exact byte on the next poll.
            let text = match cursor.field {
                MemoryTextField::Title => &evidence.title,
                MemoryTextField::Content => &evidence.content,
            };
            let start = cursor.byte as usize;
            let end =
                text.floor_char_boundary((start + MAX_CAPTURE_TEXT_BYTES - bytes).min(text.len()));
            if end < start || !text.is_char_boundary(start) {
                return Err(OwnerSessionError::ManagementStale);
            }
            if let Some(mut passage) = memory_passage(
                &page.cut,
                &evidence,
                cursor.field,
                start,
                end,
                source.sampled,
            ) {
                if source.ego_only {
                    passage.eligible = Eligibility::EGO;
                }
                bytes += passage.text.len();
                work.push(WorkItem {
                    mandatory: passage.eligible,
                    passage,
                });
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
                MemoryTextField::Content => next_source(cursor),
            };
        } else {
            let id = page.derived[position - page.sources.len()];
            if let Some(head) = rel.observation(id).filter(|head| {
                head.standing == ObservationStanding::Current
                    && head.revision.publication_version <= page.cut.observation
            }) {
                let text = &head.revision.draft.proposition;
                if bytes + text.len() > MAX_CAPTURE_TEXT_BYTES {
                    break;
                }
                work.push(WorkItem {
                    passage: Passage {
                        citation: audit_citation(head.revision.full_citation()),
                        grade: Grade::Fresh,
                        eligible: Eligibility::PERCEPTION,
                        text: text.clone(),
                    },
                    mandatory: Eligibility::PERCEPTION,
                });
                bytes += text.len();
            }
            cursor = next_source(cursor);
        }
    }
    let captured_sources = work
        .iter()
        .map(|item| item.passage.citation.source)
        .collect::<BTreeSet<_>>();
    for entity in &participants {
        for neighbor in rel.perception_relationship_sources(
            reliquary_owner_session::model::EntityId(entity.entity_id),
        ) {
            if local_sources.len() == MAX_LOCAL_SOURCES {
                break;
            }
            local_sources.insert(neighbor);
        }
    }
    for id in local_sources {
        if bytes + MAX_LOCAL_PASSAGE_BYTES > MAX_CAPTURE_TEXT_BYTES
            || captured_sources.contains(&SourceId::Memory(id.0))
        {
            continue;
        }
        let Some(evidence) = rel.perception_memory_evidence(&page.cut, &[id])?.pop() else {
            continue;
        };
        let end = evidence
            .content
            .floor_char_boundary(MAX_LOCAL_PASSAGE_BYTES.min(evidence.content.len()));
        if let Some(passage) = memory_passage(
            &page.cut,
            &evidence,
            MemoryTextField::Content,
            0,
            end,
            false,
        ) {
            take_participants(
                rel,
                &page.cut,
                evidence.metadata.source,
                &mut participant_ids,
                &mut participants,
            )?;
            bytes += passage.text.len();
            work.push(WorkItem {
                mandatory: Eligibility::NONE,
                passage,
            });
        }
    }
    let profile = if participants.len() >= 2 {
        ObservationProfile::Relational
    } else {
        ObservationProfile::Ordinary
    };
    let contract = PerceptionContract { profile };
    let key = capture_key(
        route_identity,
        page.cut.source_removal,
        &work,
        &participants,
    )?;
    let batches = plan_batches(
        audit_owner_cut(&page.cut),
        &work,
        &participants,
        BatchLimits::default(),
        &EgoContract,
        &contract,
    )
    .map_err(|_| OwnerSessionError::ManagementLimit)?;
    Ok(AuditDiscovery {
        key,
        next: cursor,
        batches,
        contract,
    })
}

fn take_participants(
    rel: &RelSession,
    cut: &PerceptionSourceCut,
    source: reliquary_owner_session::PerceptionMemorySource,
    seen: &mut BTreeSet<[u8; 32]>,
    slots: &mut Vec<Participant>,
) -> Result<(), OwnerSessionError> {
    if slots.len() < MAX_PARTICIPANT_SLOTS {
        for anchor in rel.perception_participant_evidence(cut, source)? {
            if seen.insert(anchor.entity_id.0) {
                slots.push(Participant {
                    owner_id: cut.owner_id,
                    entity_id: anchor.entity_id.0,
                    revision: anchor.entity_revision,
                    label: anchor.canonical_name,
                });
                if slots.len() == MAX_PARTICIPANT_SLOTS {
                    break;
                }
            }
        }
    }
    Ok(())
}

fn next_source(cursor: AuditDiscoveryCursor) -> AuditDiscoveryCursor {
    AuditDiscoveryCursor {
        source: cursor.source + 1,
        field: MemoryTextField::Title,
        byte: 0,
    }
}

fn capture_key(
    route: [u8; 32],
    source_removal: u64,
    work: &[WorkItem],
    participants: &[Participant],
) -> Result<[u8; 32], OwnerSessionError> {
    let mut hash = Sha256::new();
    hash.update(b"reliquary-perception-audit-capture-v1\0");
    hash.update(CONTRACT_VERSION.to_le_bytes());
    hash.update(route);
    if source_removal != 0 {
        hash.update(source_removal.to_le_bytes());
    }
    for item in work {
        hash.update(
            serde_json::to_vec(&item.passage.citation)
                .map_err(|_| OwnerSessionError::ManagementEvidence)?,
        );
        hash.update([
            u8::from(item.mandatory.ego),
            u8::from(item.mandatory.perception),
        ]);
    }
    for p in participants {
        hash.update(p.owner_id);
        hash.update(p.entity_id);
        hash.update(p.revision.to_le_bytes());
        hash.update((p.label.len() as u64).to_le_bytes());
        hash.update(p.label.as_bytes());
    }
    Ok(hash.finalize().into())
}

pub fn discover(
    capture: &AuditDiscovery,
    batch: usize,
    endpoint: &dyn GeneralEndpoint,
    lookup: &mut impl FnMut(Feature, &str) -> Result<Vec<Passage>, LookupError>,
) -> Result<AuditDiscoveryResult, AuditError> {
    let batch = capture.batches.get(batch).ok_or(AuditError::InvalidInput)?;
    run_batch(
        batch,
        endpoint,
        &EgoContract,
        &capture.contract,
        &mut |_: &OwnerCut, feature, query: &str| lookup(feature, query),
    )
}

pub fn audit_lookups(
    rel: &RelSession,
    cut: &PerceptionSourceCut,
    feature: Feature,
    query: &str,
) -> Result<Vec<Passage>, OwnerSessionError> {
    if cut.source_removal != rel.archive_source_removal_version() {
        return Err(OwnerSessionError::ManagementStale);
    }
    let ids = rel.perception_lexical_candidates(query, MAX_LOOKUP_INPUTS);
    let rows = rel.perception_source_metadata(cut, &ids)?;
    let mut passages = Vec::new();
    for row in rows {
        if feature == Feature::Ego && row.grade != PerceptionSourceGrade::Fresh {
            continue;
        }
        let Some(evidence) = rel.perception_memory_evidence(cut, &[row.source.id])?.pop() else {
            continue;
        };
        let passage = memory_passage(
            cut,
            &evidence,
            MemoryTextField::Content,
            0,
            evidence
                .content
                .floor_char_boundary(MAX_LOOKUP_PASSAGE_BYTES.min(evidence.content.len())),
            false,
        );
        if let Some(passage) = passage {
            passages.push(passage);
            if passages.len() == reliquary_memory_audit::MAX_PASSAGES_PER_LOOKUP {
                break;
            }
        }
    }
    Ok(passages)
}

/// Ego's handoff stays Fresh if an accepted turn changes a source grade
/// during discovery.
pub fn retain_current_ego_evidence(
    rel: &RelSession,
    candidates: &mut Vec<EgoCandidate>,
) -> Result<(), OwnerSessionError> {
    let cut = rel.perception_source_cut();
    for candidate in candidates.iter_mut() {
        let mut retained = Vec::new();
        for source in candidate.sources.drain(..) {
            let cite = owner_citation(source);
            let ObservationSource::Memory(id) = cite.source else {
                continue;
            };
            let current = rel.perception_source_metadata(&cut, &[MemoryId(id)])?;
            if current.iter().any(|row| {
                row.grade == PerceptionSourceGrade::Fresh
                    && row.source.revision == cite.revision
                    && row.source.body_id.0 == cite.body_id
            }) && rel.observation_citation_text(&cite).is_ok()
            {
                retained.push(source);
            }
        }
        candidate.sources = retained;
    }
    candidates.retain(|candidate| !candidate.sources.is_empty());
    Ok(())
}
