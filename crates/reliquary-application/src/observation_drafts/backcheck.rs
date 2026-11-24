use super::capture::{Capture, Excerpt, text_field};
use super::{Error, Result};
use reliquary_owner_session::model::{CompatibilityProfileId, MemoryId, MemoryTextField};
use reliquary_owner_session::observation::{
    ObservationCitation, ObservationReceptor, ObservationReceptorKind, ObservationSource,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) const MAX_HITS: usize = 3;
pub(super) const MAX_HIT_BYTES: usize = 8 * 1024;
const LOOKUP_LIMIT: usize = 8;
const POOL_LIMIT: usize = 128;
const EXCERPT_BYTES: usize = 1536;

#[derive(Clone)]
struct Match {
    id: MemoryId,
    kind: ObservationReceptorKind,
    description: String,
    score: f64,
}

/// Use metadata and indexed candidate lookups before reading a source body.
pub(super) fn optional_hits(
    capture: &mut Capture<'_>,
    claim: &str,
    receptors: &[ObservationReceptor],
    profile: Option<CompatibilityProfileId>,
    original: &[Excerpt],
    required: Vec<Excerpt>,
) -> Result<Vec<Excerpt>> {
    use ObservationReceptorKind::{Challenge, Qualification};

    let mut result = required;
    if result.len() == MAX_HITS {
        return Ok(result);
    }
    let anchors: Vec<_> = original
        .iter()
        .filter_map(|row| match row.citation.source {
            ObservationSource::Memory(id) => Some(MemoryId(id)),
            ObservationSource::Observation(_) => None,
        })
        .collect();
    let mut graph = BTreeSet::new();
    for &id in &anchors {
        graph.extend(capture.rel.perception_graph_neighbors(id));
    }
    let anchor = anchors.first().copied();
    let profile = profile.filter(|&id| capture.rel.active_compatibility_profile(id).is_some());
    let mut ranked = Vec::new();
    for receptor in receptors {
        let mut ids: BTreeMap<MemoryId, f64> = graph.iter().map(|id| (*id, 0.12)).collect();
        for query in [&receptor.description, claim] {
            for (rank, id) in capture
                .rel
                .perception_lexical_candidates(query, LOOKUP_LIMIT)
                .into_iter()
                .enumerate()
            {
                let lexical = 0.31 - 0.02 * rank as f64;
                ids.entry(id)
                    .and_modify(|prior| *prior = prior.max(lexical))
                    .or_insert(lexical);
            }
        }
        if let Some((anchor, profile)) = anchor.zip(profile) {
            match capture
                .rel
                .dream_candidate_pool(anchor, profile, &receptor.vector)
            {
                Ok(candidates) => {
                    for id in candidates.ids {
                        if ids.len() == POOL_LIMIT {
                            break;
                        }
                        ids.entry(id).or_insert(0.0);
                    }
                }
                Err(reliquary_owner_session::OwnerSessionError::StaleDreamCut) => {}
                Err(error) => return Err(Error::Owner(error)),
            }
        }
        let ids_in_scope = ids.keys().copied().collect::<Vec<_>>();
        let eligible = capture
            .rel
            .perception_source_metadata(&capture.cut, &ids_in_scope)
            .map_err(Error::Owner)?;
        let mut matches = Vec::new();
        for row in eligible {
            let id = row.source.id;
            let vector_score = if let Some(profile) = profile {
                capture
                    .rel
                    .memory_vector_row(profile, row.source.body_id)
                    .map_err(Error::Owner)?
                    .and_then(|vector| reliquary_vectors::cosine(&vector, &receptor.vector))
                    .filter(|score| *score > 0.0)
                    .unwrap_or(0.0)
            } else {
                0.0
            };
            let score = ids[&id].max(vector_score);
            if score > 0.0 {
                matches.push(Match {
                    id,
                    kind: receptor.kind,
                    description: receptor.description.clone(),
                    score,
                });
            }
        }
        matches.sort_by(compare);
        ranked.extend(matches.into_iter().take(LOOKUP_LIMIT));
    }
    ranked.sort_by(compare);
    // Reserve the strongest available challenge and qualification before filling by rank.
    for kind in [Challenge, Qualification] {
        for candidate in ranked.iter().filter(|candidate| candidate.kind == kind) {
            if result.len() == MAX_HITS {
                break;
            }
            if admit(capture, candidate, original, &mut result)? {
                break;
            }
        }
    }
    for candidate in &ranked {
        if result.len() == MAX_HITS {
            break;
        }
        admit(capture, candidate, original, &mut result)?;
    }
    Ok(result)
}

fn compare(left: &Match, right: &Match) -> std::cmp::Ordering {
    right.score.total_cmp(&left.score).then_with(|| {
        (left.id, left.kind, &left.description).cmp(&(right.id, right.kind, &right.description))
    })
}

fn admit(
    capture: &mut Capture<'_>,
    candidate: &Match,
    original: &[Excerpt],
    selected: &mut Vec<Excerpt>,
) -> Result<bool> {
    let Some(cite) = window_citation(capture, candidate.id, &candidate.description)? else {
        return Ok(false);
    };
    let same_window = |previous: &Excerpt| {
        previous.citation.source == cite.source
            && previous.citation.revision == cite.revision
            && previous.citation.field == cite.field
            && previous.citation.start_byte < cite.end_byte
            && cite.start_byte < previous.citation.end_byte
    };
    if original.iter().chain(&*selected).any(same_window) {
        return Ok(false);
    }
    let (mut row, _) = capture.citation(cite)?;
    let remaining = MAX_HIT_BYTES - selected.iter().map(Excerpt::bytes).sum::<usize>();
    if row.text.len() > remaining {
        return Ok(false);
    }
    row.trim_context(remaining - row.text.len());
    selected.push(row);
    Ok(true)
}

pub(super) fn window_citation(
    capture: &mut Capture<'_>,
    id: MemoryId,
    query: &str,
) -> Result<Option<ObservationCitation>> {
    let owner_id = capture.cut.owner_id;
    let history_generation = capture.cut.history_generation;
    let Some(row) = capture.memory(id)? else {
        return Ok(None);
    };
    let field = if row.content.trim().is_empty() {
        MemoryTextField::Title
    } else {
        MemoryTextField::Content
    };
    let text = text_field(row, field);
    let Some((start_byte, end_byte)) = window(text, query) else {
        return Ok(None);
    };
    Ok(Some(ObservationCitation {
        owner_id,
        history_generation,
        source: ObservationSource::Memory(id.0),
        revision: row.metadata.source.revision,
        body_id: row.metadata.source.body_id.0,
        field,
        start_byte,
        end_byte,
    }))
}

fn window(text: &str, query: &str) -> Option<(u32, u32)> {
    // Lowercasing ASCII preserves byte coordinates for UTF-8 source strings.
    let normalized = text.to_ascii_lowercase();
    let first_match = query
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|term| term.len() >= 4)
        .take(24)
        .filter_map(|term| normalized.find(&term.to_ascii_lowercase()))
        .min()
        .unwrap_or(0);
    let mut start = first_match.saturating_sub(EXCERPT_BYTES / 4);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    let mut end = text.len().min(start + EXCERPT_BYTES);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let value = text.get(start..end)?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    start += value.len() - value.trim_start().len();
    Some((
        u32::try_from(start).ok()?,
        u32::try_from(start + trimmed.len()).ok()?,
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn windows_keep_byte_exact_unicode_excerpts() {
        let text = format!(
            "{} Counterexample: admitted évidence. {}",
            "日本語 ".repeat(400),
            "続き ".repeat(400)
        );
        let (start, end) = super::window(&text, "counterexample admitted").unwrap();
        assert!(text[start as usize..end as usize].contains("Counterexample: admitted évidence."));
        assert!(end - start <= super::EXCERPT_BYTES as u32);
    }
}
