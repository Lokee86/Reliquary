//! Hybrid ranking over one owner-qualified Archive generation.
use reliquary_owner_session::{ArchiveHybridView, model::CompatibilityProfileId};
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

struct ScoredFragment {
    ordinal: usize,
    lexical: f64,
    semantic: f64,
    combined: f64,
}

pub fn rank_archive_hybrid(
    view: ArchiveHybridView,
    text: &str,
    query: &[f32],
    config: RetrievalConfig,
) -> Result<ArchiveHybridSearch, String> {
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
        let row = view
            .packed_row(ordinal)
            .ok_or("missing Archive vector row")?;
        let semantic = cosine_f32_row(row, query, query_norm)?
            .ok_or("zero Archive vector row")?
            .clamp(-1.0, 1.0);
        let lexical = lexical_score_from_counts(terms.len(), counts);
        let combined = combine_hybrid_scores(lexical, semantic, weights);
        if combined > 0.0 {
            ranked.push(ScoredFragment {
                ordinal,
                lexical,
                semantic,
                combined,
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
    let hits = ranked
        .into_iter()
        .map(|hit| {
            let fragment = &fragments[hit.ordinal];
            ArchiveHybridHit {
                fragment_id: fragment.id.0,
                ordinal: hit.ordinal,
                conversation_id: fragment.conversation_id.clone(),
                start_node_id: fragment.start_node_id.clone(),
                end_node_id: fragment.end_node_id.clone(),
                lexical_score: (hit.lexical > 0.0).then_some(hit.lexical),
                semantic_score: (hit.semantic > 0.0).then_some(hit.semantic),
                score: hit.combined,
            }
        })
        .collect();
    Ok(ArchiveHybridSearch {
        profile_id: view.generation.compatibility_profile_id,
        generation_id: view.generation.id.0,
        source_archive_version: view.generation.source_archive_version,
        fragments_scored: fragments.len(),
        fragments_with_text: view.fragments_with_text(),
        truncated,
        hits,
    })
}
