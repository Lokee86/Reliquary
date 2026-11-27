use super::*;
use crate::observation_drafts::ObservationRead;
use reliquary_freshness::semantic_policy::{SemanticPolicy, cosine_similarity};
use reliquary_owner_session::model::CompatibilityProfileId;
use reliquary_owner_session::observation::{
    ObservationActivity, ObservationComparison, ObservationComparisonOutcome,
    ObservationInspection, ObservationOperation, ObservationPendingReason, ObservationReceptorKind,
    ObservationRevision, ObservationStanding,
};
use reliquary_owner_session::{
    AuditWorkSource, OwnerSessionError, PerceptionAuditPage, PhyObservationCut, PhyWriter,
    RelSession,
};
use reliquary_perception::{ContributionState, HitVerdict, NativeDecisionError, contribute};
use reliquary_switchboard::EmbeddingMode;
use std::collections::{BTreeMap, BTreeSet};

const MAX_HITS: usize = 5;
const MAX_PASSAGE_BYTES: usize = 4 * 1024;
const MAX_EMBEDDING_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const PHY_RECEPTOR_MATCH_COSINE: f64 = 0.75;

/// Short-lived exact passage vectors for one owner and configured route.
#[derive(Default)]
pub struct ContributionEmbeddingCache {
    rows: BTreeMap<ObservationCitation, Vec<f32>>,
}

pub struct ContributionWork {
    pub next_source: u16,
    pub next_target: Option<ObservationId>,
    target: Option<ContributionTarget>,
}

impl ContributionWork {
    /// Pins the exact semantic input without retaining any passage text.
    pub fn decision_key(&self) -> Option<[u8; 32]> {
        let target = self.target.as_ref()?;
        let mut hash = Sha256::new();
        hash.update(target.revision.id.0);
        hash.update(target.revision.revision.to_le_bytes());
        for row in &target.available {
            hash.update(serde_json::to_vec(&row.cite).ok()?);
        }
        Some(hash.finalize().into())
    }
}

pub enum ContributionFailure {
    MalformedReply,
    Owner(OwnerSessionError),
}

impl From<OwnerSessionError> for ContributionFailure {
    fn from(value: OwnerSessionError) -> Self {
        Self::Owner(value)
    }
}

struct ContributionTarget {
    revision: ObservationRevision,
    available: Vec<ContributionEvidence>,
    phy: bool,
}

struct ContributionEvidence {
    cite: ObservationCitation,
    text: String,
    activity: Option<ObservationActivity>,
    structural: bool,
    vector: Option<Vec<f32>>,
}

/// One bounded source group and one exact target head, with independent
/// durable source and target cursors. The same capture works with no audit
/// candidates and skips already compared exact passages.
pub fn capture_contribution(
    rel: &RelSession,
    page: &PerceptionAuditPage,
    samples_only: bool,
    profile: Option<CompatibilityProfileId>,
) -> Result<ContributionWork, OwnerSessionError> {
    if page.cut.source_removal != rel.archive_source_removal_version() {
        return Err(OwnerSessionError::ManagementStale);
    }
    capture_source_contribution(
        ObservationRead::Rel(rel),
        &page.cut,
        &page.sources,
        page.contribution_cursor,
        page.contribution_target_after,
        samples_only,
        profile,
    )
}

pub fn capture_phy_contribution(
    phy: &PhyWriter,
    cut: PhyObservationCut,
    sources: &[MemoryId],
    source_cursor: u16,
    target_after: Option<ObservationId>,
    profile: Option<CompatibilityProfileId>,
) -> Result<ContributionWork, OwnerSessionError> {
    super::phy_discovery::check_cut(phy, cut)?;
    if sources.len() > usize::from(u16::MAX) {
        return Err(OwnerSessionError::ManagementLimit);
    }
    let sources: Vec<_> = sources
        .iter()
        .map(|&memory| AuditWorkSource {
            memory,
            sampled: false,
            newly_settled: false,
            ego_only: false,
        })
        .collect();
    capture_source_contribution(
        ObservationRead::Phy(phy),
        &cut.into(),
        &sources,
        source_cursor,
        target_after,
        false,
        profile,
    )
}

fn capture_source_contribution(
    owner: ObservationRead<'_>,
    cut: &PerceptionSourceCut,
    sources: &[AuditWorkSource],
    source_cursor: u16,
    target_after: Option<ObservationId>,
    samples_only: bool,
    profile: Option<CompatibilityProfileId>,
) -> Result<ContributionWork, OwnerSessionError> {
    let position = usize::from(source_cursor);
    if position > sources.len() {
        return Err(OwnerSessionError::ManagementStale);
    }
    let end = (position + MAX_HITS).min(sources.len());
    if position == end || (samples_only && !sources[position..].iter().any(|source| source.sampled))
    {
        return Ok(ContributionWork {
            next_source: sources.len() as u16,
            next_target: None,
            target: None,
        });
    }
    if samples_only && !sources[position..end].iter().any(|source| source.sampled) {
        return Ok(ContributionWork {
            next_source: end as u16,
            next_target: None,
            target: None,
        });
    }
    let Some(inspection) = owner.observations_after(target_after, 1).pop() else {
        return Ok(ContributionWork {
            next_source: end as u16,
            next_target: None,
            target: None,
        });
    };
    let id = inspection.revision.id;
    let target = if inspection.revision.publication_version <= cut.observation
        && accepts_contribution(inspection.standing)
        && owner.observation_revision_source_eligible(id, inspection.revision.revision)
    {
        let mut evidence = Vec::new();
        let mut seen_lineage = BTreeSet::new();
        for source in &sources[position..end] {
            if samples_only && !source.sampled {
                continue;
            }
            let Some(row) = owner
                .perception_memory_evidence(cut, &[source.memory])?
                .pop()
            else {
                continue;
            };
            if seen_lineage.contains(&row.source_lineage) {
                continue;
            }
            if let Some((cite, text)) = contribution_passage(cut, &row) {
                if can_compare(owner, &inspection, cite) {
                    seen_lineage.insert(row.source_lineage);
                    let neighbors = match owner {
                        ObservationRead::Rel(rel) => rel.perception_graph_neighbors(source.memory),
                        ObservationRead::Phy(phy) => phy
                            .perception_graph_links(cut, source.memory)?
                            .into_iter()
                            .map(|(neighbor, _)| neighbor)
                            .collect(),
                    };
                    let structural =
                        inspection.revision.draft.citations.iter().any(|prior| {
                            matches!(prior.source, ObservationSource::Memory(id)
                            if id == source.memory.0 || neighbors.contains(&MemoryId(id)))
                        }) || (!inspection.revision.draft.profile.participants().is_empty()
                            && owner
                                .perception_participant_evidence(cut, row.metadata.source)?
                                .iter()
                                .any(|anchor| {
                                    inspection.revision.draft.profile.participants().iter().any(
                                        |participant| participant.entity_id == anchor.entity_id,
                                    )
                                }));
                    let activity = match owner {
                        ObservationRead::Rel(rel) if source.newly_settled => {
                            rel.perception_source_activity(&cite)
                        }
                        _ => None,
                    };
                    let whole_phy_document = matches!(owner, ObservationRead::Phy(_))
                        && cite.field == MemoryTextField::Content
                        && text.len() == row.content.len();
                    let vector = if let Some(profile) =
                        profile.filter(|_| source.sampled || whole_phy_document)
                    {
                        owner.memory_vector_row(profile, row.metadata.source.body_id)?
                    } else {
                        None
                    };
                    evidence.push(ContributionEvidence {
                        cite,
                        text: text.to_owned(),
                        activity,
                        structural,
                        vector,
                    });
                }
            }
        }
        Some(ContributionTarget {
            revision: inspection.revision.clone(),
            available: evidence,
            phy: matches!(owner, ObservationRead::Phy(_)),
        })
    } else {
        None
    };
    Ok(ContributionWork {
        next_source: source_cursor,
        next_target: Some(id),
        target,
    })
}

fn contribution_passage<'a>(
    cut: &PerceptionSourceCut,
    row: &'a PerceptionMemoryEvidence,
) -> Option<(ObservationCitation, &'a str)> {
    let source = row.metadata.source;
    [
        (&row.content, MemoryTextField::Content),
        (&row.title, MemoryTextField::Title),
    ]
    .into_iter()
    .find_map(|(text, field)| {
        let end = text.floor_char_boundary(MAX_PASSAGE_BYTES.min(text.len()));
        let text = &text[..end];
        (!text.trim().is_empty()).then_some((
            ObservationCitation {
                owner_id: cut.owner_id,
                history_generation: cut.history_generation,
                source: ObservationSource::Memory(source.id.0),
                revision: source.revision,
                body_id: source.body_id.0,
                field,
                start_byte: 0,
                end_byte: end as u32,
            },
            text,
        ))
    })
}

pub fn capture_duplicate_contribution(
    rel: &RelSession,
    id: ObservationId,
    revision: u64,
    evidence: &[ObservationCitation],
) -> Result<ContributionWork, OwnerSessionError> {
    capture_owner_duplicate_contribution(ObservationRead::Rel(rel), id, revision, evidence)
}

pub fn capture_phy_duplicate_contribution(
    phy: &PhyWriter,
    id: ObservationId,
    revision: u64,
    evidence: &[ObservationCitation],
) -> Result<ContributionWork, OwnerSessionError> {
    capture_owner_duplicate_contribution(ObservationRead::Phy(phy), id, revision, evidence)
}

fn capture_owner_duplicate_contribution(
    owner: ObservationRead<'_>,
    id: ObservationId,
    revision: u64,
    evidence: &[ObservationCitation],
) -> Result<ContributionWork, OwnerSessionError> {
    let target = owner
        .observation(id)
        .filter(|head| {
            head.revision.revision == revision
                && accepts_contribution(head.standing)
                && owner.observation_revision_source_eligible(id, revision)
        })
        .map(|head| {
            let mut unique = BTreeSet::new();
            let mut available = Vec::new();
            for &cite in evidence {
                if can_compare(owner, &head, cite) {
                    let mut text = owner.observation_citation_text(&cite)?;
                    let len = text.floor_char_boundary(MAX_PASSAGE_BYTES.min(text.len()));
                    text.truncate(len);
                    let cite = ObservationCitation {
                        end_byte: cite.start_byte + len as u32,
                        ..cite
                    };
                    if !text.trim().is_empty()
                        && unique.insert(cite)
                        && can_compare(owner, &head, cite)
                    {
                        available.push(ContributionEvidence {
                            cite,
                            text,
                            activity: None,
                            structural: true,
                            vector: None,
                        });
                        if available.len() == MAX_HITS {
                            break;
                        }
                    }
                }
            }
            Ok::<_, OwnerSessionError>(ContributionTarget {
                revision: head.revision.clone(),
                available,
                phy: matches!(owner, ObservationRead::Phy(_)),
            })
        })
        .transpose()?;
    Ok(ContributionWork {
        next_source: 0,
        next_target: None,
        target,
    })
}

fn accepts_contribution(standing: ObservationStanding) -> bool {
    matches!(
        standing,
        ObservationStanding::Current
            | ObservationStanding::Pending(ObservationPendingReason::NewEvidence)
    )
}

fn can_compare(
    owner: ObservationRead<'_>,
    head: &ObservationInspection<'_>,
    cite: ObservationCitation,
) -> bool {
    !head.revision.draft.citations.contains(&cite)
        && owner
            .observation_comparison(&ObservationComparison {
                id: head.revision.id,
                target_revision: head.revision.revision,
                citation: cite,
                outcome: ObservationComparisonOutcome::NoMaterialEffect,
            })
            .is_none()
}

pub fn evaluate_contribution(
    work: &ContributionWork,
    routes: &PerceptionRoutes,
    cache: &mut ContributionEmbeddingCache,
) -> Result<Vec<ObservationOperation>, ContributionFailure> {
    let Some(target) = &work.target else {
        return Ok(Vec::new());
    };
    let profile = routes.embedding_profile();
    let receptors = &target.revision.draft.receptors;
    if target.available.is_empty() {
        return Ok(Vec::new());
    }
    let needs_receptor = |evidence: &ContributionEvidence| !target.phy || !evidence.structural;
    let score_receptors = !receptors.is_empty() && target.available.iter().any(needs_receptor);
    if !target.phy && !score_receptors {
        return Ok(Vec::new());
    }
    let rebound = if score_receptors
        && receptors
            .iter()
            .any(|receptor| receptor.embedding_profile != profile)
    {
        let descriptions = receptors
            .iter()
            .map(|receptor| receptor.description.clone())
            .collect::<Vec<_>>();
        Some(embed(routes, EmbeddingMode::Query, &descriptions)?)
    } else {
        None
    };
    if cache.rows.len() >= MAX_HITS * 4 {
        cache.rows.clear();
    }
    let missing = target
        .available
        .iter()
        .filter(|evidence| score_receptors && needs_receptor(evidence))
        .filter(|item| item.vector.is_none() && !cache.rows.contains_key(&item.cite))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        let documents = missing
            .iter()
            .map(|item| item.text.clone())
            .collect::<Vec<_>>();
        let vectors = embed(routes, EmbeddingMode::Document, &documents)?;
        cache.rows.extend(
            missing
                .iter()
                .zip(vectors)
                .map(|(item, vector)| (item.cite, vector)),
        );
    }
    let rel_policy = (!target.phy).then(SemanticPolicy::default);
    let mut matches = Vec::new();
    let mut model_hits = BTreeMap::new();
    for (index, evidence) in target.available.iter().enumerate() {
        let slot = format!("p{index}");
        if !needs_receptor(evidence) {
            model_hits.insert(slot.clone(), evidence.text.clone());
            matches.push((slot, evidence, 0));
            continue;
        }
        if !score_receptors {
            continue;
        }
        let vector = evidence
            .vector
            .as_ref()
            .or_else(|| cache.rows.get(&evidence.cite))
            .ok_or_else(|| {
                OwnerSessionError::Record("missing Perception passage embedding".into())
            })?;
        let mut strength = 0u8;
        let mut matched = false;
        let mut review = evidence.structural;
        for (receptor_index, receptor) in receptors.iter().enumerate() {
            let query = rebound
                .as_ref()
                .map_or(&receptor.vector, |rows| &rows[receptor_index]);
            let hit = if let Some(policy) = rel_policy {
                let cosine = cosine_similarity(vector, query).map_err(|_| {
                    OwnerSessionError::Record("incompatible Perception receptor embedding".into())
                })?;
                let amount = policy.bonus(cosine).map_err(|_| {
                    OwnerSessionError::Record("invalid Perception receptor score".into())
                })?;
                strength = strength.max(amount as u8);
                amount > 0
            } else {
                reliquary_vectors::cosine(vector, query).ok_or_else(|| {
                    OwnerSessionError::Record("incompatible Perception receptor embedding".into())
                })? >= PHY_RECEPTOR_MATCH_COSINE
            };
            matched |= hit;
            review |= hit && receptor.kind != ObservationReceptorKind::Support;
        }
        if matched || review {
            if review {
                model_hits.insert(slot.clone(), evidence.text.clone());
            }
            matches.push((slot, evidence, strength));
        }
    }

    let decision = if model_hits.is_empty() {
        None
    } else {
        Some(
            contribute(
                routes.contribution.as_ref(),
                &ContributionState {
                    existing_observation: target.revision.draft.proposition.clone(),
                    evidence_hits: model_hits,
                },
            )
            .map_err(|error| match error {
                NativeDecisionError::InvalidReply(_) => ContributionFailure::MalformedReply,
                _ => {
                    OwnerSessionError::Record("Perception contribution decision unavailable".into())
                        .into()
                }
            })?,
        )
    };
    let id = target.revision.id;
    let revision = target.revision.revision;
    let mut operations = Vec::new();
    let mut triggering = Vec::new();
    for (slot, row, strength) in matches {
        let outcome = match decision.as_ref().and_then(|result| result.hits.get(&slot)) {
            Some(hit) if hit.triggers_review() => {
                triggering.push(row.cite);
                match hit.verdict {
                    HitVerdict::MaterialQualification => {
                        ObservationComparisonOutcome::MaterialQualification
                    }
                    _ => ObservationComparisonOutcome::MaterialChallenge,
                }
            }
            Some(hit) if hit.verdict == HitVerdict::WeakOrUncertain => {
                ObservationComparisonOutcome::WeakOrUncertain
            }
            _ => ObservationComparisonOutcome::NoMaterialEffect,
        };
        operations.push(ObservationOperation::Comparison {
            comparison: ObservationComparison {
                id,
                target_revision: revision,
                citation: row.cite,
                outcome,
            },
        });
        if let Some(activity) = row.activity.filter(|_| strength > 0) {
            operations.push(ObservationOperation::SemanticAward {
                id,
                expected_revision: revision,
                activity,
                evidence: row.cite,
                amount: strength,
            });
        }
    }
    if !triggering.is_empty() {
        operations.push(ObservationOperation::Pending {
            id,
            expected_revision: revision,
            reason: ObservationPendingReason::NewEvidence,
            evidence: triggering,
        });
    }
    Ok(operations)
}

fn embed(
    routes: &PerceptionRoutes,
    mode: EmbeddingMode,
    input: &[String],
) -> Result<Vec<Vec<f32>>, OwnerSessionError> {
    let vectors = if routes.embedding.supports_bounded_embedding() {
        routes
            .embedding
            .embed_bounded(mode, input, MAX_EMBEDDING_RESPONSE_BYTES)
    } else {
        routes.embedding.embed(mode, input)
    }
    .map_err(|_| {
        OwnerSessionError::Record("Perception contribution embedding unavailable".into())
    })?;
    if vectors.len() != input.len()
        || vectors.iter().any(|vector| {
            vector.len() != routes.embedding.dimensions() as usize
                || vector.iter().any(|value| !value.is_finite())
                || vector.iter().all(|value| *value == 0.0)
        })
    {
        return Err(OwnerSessionError::Record(
            "invalid Perception passage embeddings".into(),
        ));
    }
    Ok(vectors)
}
