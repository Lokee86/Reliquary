use super::*;
use reliquary_freshness::semantic_policy::{SemanticPolicy, cosine_similarity};
use reliquary_owner_session::observation::{
    ObservationActivity, ObservationComparison, ObservationComparisonOutcome,
    ObservationInspection, ObservationOperation, ObservationPendingReason, ObservationReceptorKind,
    ObservationRevision, ObservationStanding,
};
use reliquary_owner_session::{OwnerSessionError, PerceptionAuditPage, RelSession};
use reliquary_perception::{ContributionState, HitVerdict, contribute};
use reliquary_switchboard::EmbeddingMode;
use std::collections::{BTreeMap, BTreeSet};

const MAX_HITS: usize = 5;
const MAX_PASSAGE_BYTES: usize = 4 * 1024;
const MAX_EMBEDDING_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

pub struct ContributionWork {
    pub next_source: u16,
    pub next_target: Option<ObservationId>,
    target: Option<ContributionTarget>,
}

struct ContributionTarget {
    revision: ObservationRevision,
    available: Vec<ContributionEvidence>,
}

struct ContributionEvidence {
    cite: ObservationCitation,
    text: String,
    activity: Option<ObservationActivity>,
    structural: bool,
}

/// One bounded source group and one exact target head, with independent
/// durable source and target cursors. The same capture works with no audit
/// candidates and skips already compared exact passages.
pub fn capture_contribution(
    rel: &RelSession,
    page: &PerceptionAuditPage,
    samples_only: bool,
) -> Result<ContributionWork, OwnerSessionError> {
    let position = usize::from(page.contribution_cursor);
    let end = (position + MAX_HITS).min(page.sources.len());
    if samples_only && !page.sources[position..].iter().any(|source| source.sampled) {
        return Ok(ContributionWork {
            next_source: page.sources.len() as u16,
            next_target: None,
            target: None,
        });
    }
    if samples_only
        && !page.sources[position..end]
            .iter()
            .any(|source| source.sampled)
    {
        return Ok(ContributionWork {
            next_source: end as u16,
            next_target: None,
            target: None,
        });
    }
    let Some(inspection) = rel
        .observations_after(page.contribution_target_after, 1)
        .pop()
    else {
        return Ok(ContributionWork {
            next_source: end as u16,
            next_target: None,
            target: None,
        });
    };
    let id = inspection.revision.id;
    let target = if inspection.revision.publication_version <= page.cut.observation
        && accepts_contribution(inspection.standing)
    {
        let mut evidence = Vec::new();
        let mut seen_lineage = BTreeSet::new();
        for source in &page.sources[position..end] {
            if samples_only && !source.sampled {
                continue;
            }
            let Some(row) = rel
                .perception_memory_evidence(&page.cut, &[source.memory])?
                .pop()
            else {
                continue;
            };
            if !seen_lineage.insert(row.source_lineage) {
                continue;
            }
            let passage_end = row
                .content
                .floor_char_boundary(MAX_PASSAGE_BYTES.min(row.content.len()));
            let passage = memory_passage(
                &page.cut,
                &row,
                MemoryTextField::Content,
                0,
                passage_end,
                source.sampled,
            )
            .or_else(|| {
                memory_passage(
                    &page.cut,
                    &row,
                    MemoryTextField::Title,
                    0,
                    row.title
                        .floor_char_boundary(MAX_PASSAGE_BYTES.min(row.title.len())),
                    source.sampled,
                )
            });
            if let Some(passage) = passage {
                let cite = owner_citation(passage.citation);
                if can_compare(rel, &inspection, cite) {
                    let neighbors = rel.perception_graph_neighbors(source.memory);
                    let structural =
                        inspection.revision.draft.citations.iter().any(|prior| {
                            matches!(prior.source, ObservationSource::Memory(id)
                            if id == source.memory.0 || neighbors.contains(&MemoryId(id)))
                        }) || (!inspection.revision.draft.profile.participants().is_empty()
                            && rel
                                .perception_participant_evidence(&page.cut, row.metadata.source)?
                                .iter()
                                .any(|anchor| {
                                    inspection.revision.draft.profile.participants().iter().any(
                                        |participant| participant.entity_id == anchor.entity_id,
                                    )
                                }));
                    let activity = source
                        .newly_settled
                        .then(|| rel.perception_source_activity(&cite))
                        .flatten();
                    evidence.push(ContributionEvidence {
                        cite,
                        text: passage.text,
                        activity,
                        structural,
                    });
                }
            }
        }
        Some(ContributionTarget {
            revision: inspection.revision.clone(),
            available: evidence,
        })
    } else {
        None
    };
    Ok(ContributionWork {
        next_source: page.contribution_cursor,
        next_target: Some(id),
        target,
    })
}

pub fn capture_duplicate_contribution(
    rel: &RelSession,
    id: ObservationId,
    revision: u64,
    evidence: &[ObservationCitation],
) -> Result<ContributionWork, OwnerSessionError> {
    let target = rel
        .observation(id)
        .filter(|head| head.revision.revision == revision && accepts_contribution(head.standing))
        .map(|head| {
            let mut unique = BTreeSet::new();
            let available = evidence
                .iter()
                .copied()
                .filter(|cite| unique.insert(*cite) && can_compare(rel, &head, *cite))
                .take(MAX_HITS)
                .map(|cite| {
                    let mut text = rel.observation_citation_text(&cite)?;
                    let len = text.floor_char_boundary(MAX_PASSAGE_BYTES.min(text.len()));
                    text.truncate(len);
                    let cite = ObservationCitation {
                        end_byte: cite.start_byte + len as u32,
                        ..cite
                    };
                    Ok(ContributionEvidence {
                        cite,
                        text,
                        activity: None,
                        structural: true,
                    })
                })
                .collect::<Result<Vec<_>, OwnerSessionError>>()?;
            Ok::<_, OwnerSessionError>(ContributionTarget {
                revision: head.revision.clone(),
                available,
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
    rel: &RelSession,
    head: &ObservationInspection<'_>,
    cite: ObservationCitation,
) -> bool {
    !head.revision.draft.citations.contains(&cite)
        && rel
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
) -> Result<Vec<ObservationOperation>, OwnerSessionError> {
    let Some(target) = &work.target else {
        return Ok(Vec::new());
    };
    let profile = routes.embedding_profile();
    let receptors = &target.revision.draft.receptors;
    if target.available.is_empty() || receptors.is_empty() {
        return Ok(Vec::new());
    }
    let rebound = if receptors
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
    let input = target
        .available
        .iter()
        .map(|item| item.text.clone())
        .collect::<Vec<_>>();
    let vectors = embed(routes, EmbeddingMode::Document, &input)?;
    let policy = SemanticPolicy::default();
    let mut matches = Vec::new();
    let mut model_hits = BTreeMap::new();
    for (index, (evidence, vector)) in target.available.iter().zip(&vectors).enumerate() {
        let mut strength = 0i16;
        let mut review = evidence.structural;
        for (receptor_index, receptor) in receptors.iter().enumerate() {
            let query = rebound
                .as_ref()
                .map_or(&receptor.vector, |rows| &rows[receptor_index]);
            let cosine = cosine_similarity(vector, query).map_err(|_| {
                OwnerSessionError::Record("incompatible Perception receptor embedding".into())
            })?;
            let amount = policy.bonus(cosine).map_err(|_| {
                OwnerSessionError::Record("invalid Perception receptor score".into())
            })?;
            strength = strength.max(amount);
            review |= amount > 0 && receptor.kind != ObservationReceptorKind::Support;
        }
        if strength > 0 || review {
            let slot = format!("p{index}");
            if review {
                model_hits.insert(slot.clone(), evidence.text.clone());
            }
            matches.push((slot, evidence, strength as u8));
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
            .map_err(|_| {
                OwnerSessionError::Record("Perception contribution decision not accepted".into())
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
