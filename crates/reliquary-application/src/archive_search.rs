//! Hybrid ranking over one owner-qualified Archive generation.
use reliquary_owner_session::{
    ArchiveHybridView,
    model::{CompatibilityProfileId, FragmentPassage},
};
use reliquary_retrieval::{
    RetrievalConfig, combine_hybrid_scores, lexical_score_from_counts, lexical_terms,
    semantic_scoring::{cosine_f32_row, vector_norm},
    validate_hybrid_request,
};

/// An Archive fragment with its component scores and combined rank.
#[derive(Clone, Debug, PartialEq)]
pub struct ArchiveHybridHit {
    pub fragment_id: [u8; 32],
    pub ordinal: usize,
    pub conversation_id: String,
    pub start_node_id: String,
    pub end_node_id: String,
    pub passage: Option<FragmentPassage>,
    pub lexical_score: Option<f64>,
    pub semantic_score: Option<f64>,
    pub score: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArchiveHybridSearch {
    pub profile_id: CompatibilityProfileId,
    pub generation_id: [u8; 32],
    pub source_archive_version: u64,
    pub fragments_scored: usize,
    pub fragments_with_text: usize,
    pub truncated: bool,
    pub hits: Vec<ArchiveHybridHit>,
}

/// Dated evidence tied to one turn on the exact fragment path.
#[derive(Clone, Debug, PartialEq)]
pub struct ArchiveTurnTemporalEvidence {
    pub node_id: String,
    pub evidence: Vec<reliquary_retrieval::TemporalEvidence>,
}

/// Temporal hits retain the ordinary fragment score and identify the eligible turns.
#[derive(Clone, Debug, PartialEq)]
pub struct TemporalArchiveHitEvidence {
    pub fragment_id: [u8; 32],
    pub turns: Vec<ArchiveTurnTemporalEvidence>,
    pub partial: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TemporalArchiveHybridSearch {
    pub search: ArchiveHybridSearch,
    pub temporal_filter: reliquary_retrieval::TemporalFilter,
    pub temporal_evidence: Vec<TemporalArchiveHitEvidence>,
}

pub fn rank_archive_hybrid_temporal(
    view: ArchiveHybridView,
    text: &str,
    query: &[f32],
    config: RetrievalConfig,
    filter: reliquary_retrieval::TemporalFilter,
) -> Result<TemporalArchiveHybridSearch, String> {
    if filter.end_exclusive_ns <= filter.start_ns {
        return Err("invalid temporal filter".into());
    }
    let (search, temporal_evidence) = rank_archive(view, text, query, config, Some(&filter))?;
    Ok(TemporalArchiveHybridSearch {
        search,
        temporal_filter: filter,
        temporal_evidence,
    })
}

pub fn rank_archive_hybrid(
    view: ArchiveHybridView,
    text: &str,
    query: &[f32],
    config: RetrievalConfig,
) -> Result<ArchiveHybridSearch, String> {
    rank_archive(view, text, query, config, None).map(|(search, _)| search)
}

struct ScoredFragment {
    ordinal: usize,
    lexical: f64,
    semantic: f64,
    combined: f64,
    evidence: Option<TemporalArchiveHitEvidence>,
}

fn rank_archive(
    view: ArchiveHybridView,
    text: &str,
    query: &[f32],
    config: RetrievalConfig,
    filter: Option<&reliquary_retrieval::TemporalFilter>,
) -> Result<(ArchiveHybridSearch, Vec<TemporalArchiveHitEvidence>), String> {
    let weights = validate_hybrid_request(text, config).map_err(|error| format!("{error:?}"))?;
    let query_norm = vector_norm(query).ok_or("invalid Archive query vector")?;
    if query.len() != view.dimensions {
        return Err("Archive query dimensions differ from the profile".into());
    }
    let terms = lexical_terms(text);
    let lexical_counts = view
        .lexical_counts(&terms)
        .map_err(|error| error.to_string())?;
    let fragments = view.fragments();
    let mut ranked = Vec::new();
    for (ordinal, counts) in lexical_counts.into_iter().enumerate() {
        let mut temporal = None;
        let mut partial_lexical = None;
        if let Some(filter) = filter {
            let Some(turns) = view.fragment_turns(ordinal) else {
                continue;
            };
            if turns.is_empty() {
                continue;
            }
            let (evidence, eligible_text) = eligible_turns(turns, filter);
            if evidence.is_empty() {
                continue;
            }
            let partial = evidence.len() != turns.len();
            if partial {
                partial_lexical = Some(reliquary_retrieval::lexical_score(
                    &eligible_text.join("\n"),
                    &terms,
                ));
            }
            temporal = Some(TemporalArchiveHitEvidence {
                fragment_id: fragments[ordinal].id.0,
                turns: evidence,
                partial,
            });
        }
        let (lexical, semantic, combined) = if let Some(lexical) = partial_lexical {
            (lexical, 0.0, partial_score(lexical, weights.0))
        } else {
            let row = view
                .packed_row(ordinal)
                .ok_or("missing Archive vector row")?;
            let semantic = cosine_f32_row(row, query, query_norm)?
                .ok_or("zero Archive vector row")?
                .clamp(-1.0, 1.0);
            let lexical = lexical_score_from_counts(terms.len(), counts);
            (
                lexical,
                semantic,
                combine_hybrid_scores(lexical, semantic, weights),
            )
        };
        if combined > 0.0 {
            ranked.push(ScoredFragment {
                ordinal,
                lexical,
                semantic,
                combined,
                evidence: temporal,
            });
        }
    }
    let order = |left: &ScoredFragment, right: &ScoredFragment| {
        right
            .combined
            .total_cmp(&left.combined)
            .then_with(|| left.ordinal.cmp(&right.ordinal))
    };
    let truncated = ranked.len() > config.result_limit;
    if truncated {
        ranked.select_nth_unstable_by(config.result_limit, order);
        ranked.truncate(config.result_limit);
    }
    ranked.sort_unstable_by(order);
    let mut hits = Vec::with_capacity(ranked.len());
    let mut temporal_evidence = Vec::new();
    for hit in ranked {
        let fragment = &fragments[hit.ordinal];
        hits.push(ArchiveHybridHit {
            fragment_id: fragment.id.0,
            ordinal: hit.ordinal,
            conversation_id: fragment.conversation_id.clone(),
            start_node_id: fragment.start_node_id.clone(),
            end_node_id: fragment.end_node_id.clone(),
            passage: fragment.passage,
            lexical_score: (hit.lexical > 0.0).then_some(hit.lexical),
            semantic_score: (hit.semantic > 0.0).then_some(hit.semantic),
            score: hit.combined,
        });
        if let Some(evidence) = hit.evidence {
            temporal_evidence.push(evidence);
        }
    }
    Ok((
        ArchiveHybridSearch {
            profile_id: view.generation.compatibility_profile_id,
            generation_id: view.generation.id.0,
            source_archive_version: view.generation.source_archive_version,
            fragments_scored: fragments.len(),
            fragments_with_text: view.fragments_with_text(),
            truncated,
            hits,
        },
        temporal_evidence,
    ))
}

fn partial_score(lexical: f64, lexical_weight: f64) -> f64 {
    lexical * lexical_weight
}

fn eligible_turns<'a>(
    turns: &'a [reliquary_owner_session::model::ResolvedTurn],
    filter: &reliquary_retrieval::TemporalFilter,
) -> (Vec<ArchiveTurnTemporalEvidence>, Vec<&'a str>) {
    let mut evidence = Vec::new();
    let mut text = Vec::new();
    for turn in turns {
        let matches = crate::text_temporal_evidence(&turn.content, Some(turn.timestamp_ns), filter);
        if !matches.is_empty() {
            evidence.push(ArchiveTurnTemporalEvidence {
                node_id: turn.node_id.clone(),
                evidence: matches,
            });
            text.push(turn.content.as_str());
        }
    }
    (evidence, text)
}

#[cfg(test)]
mod temporal_tests {
    use super::*;
    use reliquary_owner_session::model::ResolvedTurn;
    use reliquary_retrieval::{TemporalBasis, TemporalFilter};

    fn turn(id: &str, time: i64, content: &str) -> ResolvedTurn {
        ResolvedTurn {
            node_id: id.into(),
            role: "user".into(),
            principal_id: None,
            timestamp_ns: time,
            content: content.into(),
        }
    }

    #[test]
    fn mentioned_relative_dates_use_each_turns_own_timestamp() {
        let day = 86_400_000_000_000i64;
        let turns = vec![
            turn("a", day, "gateway yesterday"),
            turn("b", 2 * day, "gateway yesterday"),
        ];
        let filter = TemporalFilter::new(TemporalBasis::Mentioned, 0, day).unwrap();
        let (evidence, _) = eligible_turns(&turns, &filter);
        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].node_id, "a");
    }

    #[test]
    fn exact_turns_exclude_gaps_and_partial_text_outside_window() {
        let turns = vec![turn("a", 1, "quasar"), turn("b", 3, "other")];
        let gap = TemporalFilter::new(TemporalBasis::Source, 2, 3).unwrap();
        assert!(eligible_turns(&turns, &gap).0.is_empty());
        let filter = TemporalFilter::new(TemporalBasis::Source, 3, 4).unwrap();
        let (evidence, text) = eligible_turns(&turns, &filter);
        assert_eq!(evidence[0].node_id, "b");
        assert_eq!(
            reliquary_retrieval::lexical_score(&text.join("\n"), &lexical_terms("quasar")),
            0.0
        );
        let filter = TemporalFilter::new(TemporalBasis::Source, 1, 2).unwrap();
        let (_, text) = eligible_turns(&turns, &filter);
        let lexical =
            reliquary_retrieval::lexical_score(&text.join("\n"), &lexical_terms("quasar"));
        assert!(lexical > 0.0);
        assert_eq!(partial_score(lexical, 0.45), lexical * 0.45);
        assert_eq!(partial_score(0.0, 0.45), 0.0);
    }
}
