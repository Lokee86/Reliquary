use crate::search::deduplicate_candidates;
use crate::{
    Cva, EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingMode, Fragment, FragmentConfig,
    RetrievalConfig, SearchCandidate, VectorNormalization,
};
use std::fs;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-hybrid-search-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

struct RetrievalEndpoint;

impl RetrievalEndpoint {
    fn vector(mode: EmbeddingMode, input: &str) -> Vec<f32> {
        if mode == EmbeddingMode::Query && input == "alpha" {
            return vec![1.0, 0.0];
        }
        if mode == EmbeddingMode::Document {
            if input.contains("semantic-strong") {
                return vec![1.0, 0.0];
            }
            if input.contains("semantic-near") {
                return vec![0.8, 0.6];
            }
            if input.contains("semantic-orthogonal") {
                return vec![0.0, 1.0];
            }
        }
        vec![0.6, 0.8]
    }
}

impl EmbeddingEndpoint for RetrievalEndpoint {
    fn dimensions(&self) -> u32 {
        2
    }

    fn normalization(&self) -> VectorNormalization {
        VectorNormalization::L2
    }

    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        Ok(inputs
            .iter()
            .map(|input| Self::vector(mode, input))
            .collect())
    }
}

fn add_conversation(cva: &mut Cva, conversation: &str, turns: usize, text: &str) -> Vec<Fragment> {
    for index in 0..turns {
        cva.append_node(
            format!("{conversation}-n{index}"),
            conversation.into(),
            (index > 0).then(|| format!("{conversation}-n{}", index - 1)),
            if index % 2 == 0 { "user" } else { "assistant" }.into(),
            index as i64,
            text,
        )
        .unwrap();
    }
    cva.materialize_path_fragments(
        conversation,
        &format!("{conversation}-n{}", turns - 1),
        FragmentConfig::default(),
        false,
    )
    .unwrap()
}

#[test]
fn hybrid_search_preserves_single_channel_scores_and_blends_shared_hits() {
    let path = test_path("hybrid.cva");
    let endpoint = RetrievalEndpoint;
    let mut cva = Cva::create(path).unwrap();
    add_conversation(&mut cva, "hybrid", 8, "alpha semantic-near");
    add_conversation(&mut cva, "semantic", 8, "semantic-strong");
    add_conversation(&mut cva, "lexical", 8, "alpha semantic-orthogonal");
    let profile = cva.establish_compatibility_profile(&endpoint).unwrap();
    cva.build_archive_vector_generation(profile.id, &endpoint)
        .unwrap();
    let archive_version = cva.archive_version();
    let vector_version = cva.vector_version();
    let global_version = cva.container.latest_version();

    let results = cva.search(profile.id, &endpoint, "alpha").unwrap();
    assert_eq!(cva.archive_version(), archive_version);
    assert_eq!(cva.vector_version(), vector_version);
    assert_eq!(cva.container.latest_version(), global_version);
    let hybrid = results
        .iter()
        .find(|item| item.fragment.conversation_id == "hybrid")
        .unwrap();
    let semantic = results
        .iter()
        .find(|item| item.fragment.conversation_id == "semantic")
        .unwrap();
    let lexical = results
        .iter()
        .find(|item| item.fragment.conversation_id == "lexical")
        .unwrap();
    assert!((hybrid.lexical_score - 1.0).abs() < 1e-9);
    assert!((hybrid.semantic_score - 0.8).abs() < 1e-6);
    assert!((hybrid.combined_score - 0.89).abs() < 1e-6);
    assert!((semantic.combined_score - 1.0).abs() < 1e-9);
    assert!((lexical.combined_score - 1.0).abs() < 1e-9);
    assert!(semantic.generation_id.is_some());
    assert!(lexical.generation_id.is_none());
}

#[test]
fn diversification_penalizes_overlapping_ranges_from_same_conversation() {
    let path = test_path("diversify.cva");
    let mut cva = Cva::create(path).unwrap();
    let overlapping = add_conversation(&mut cva, "c1", 14, "content");
    let other = add_conversation(&mut cva, "c2", 8, "content");
    assert_eq!(overlapping.len(), 2);
    let candidates = vec![
        candidate(overlapping[0].clone(), 0.95),
        candidate(overlapping[1].clone(), 0.90),
        candidate(other[0].clone(), 0.80),
    ];
    let archive = crate::archive::ArchiveReadView::new(&cva.archive, &cva.container);
    let diversified = crate::search_rank::diversify_candidates(&archive, candidates, 3).unwrap();
    assert_eq!(diversified[0].fragment.id, overlapping[0].id);
    assert_eq!(diversified[1].fragment.id, other[0].id);
}

#[test]
fn duplicate_ranges_are_removed_and_default_result_limit_is_ten() {
    let fake = Fragment {
        id: crate::FragmentId([1; 32]),
        conversation_id: "c".into(),
        start_node_id: "a".into(),
        end_node_id: "b".into(),
    };
    let mut duplicate = fake.clone();
    duplicate.id = crate::FragmentId([2; 32]);
    let deduped = deduplicate_candidates(vec![candidate(fake, 0.9), candidate(duplicate, 0.1)]);
    assert_eq!(deduped.len(), 1);

    let path = test_path("limit.cva");
    let endpoint = RetrievalEndpoint;
    let mut cva = Cva::create(path).unwrap();
    for index in 0..15 {
        add_conversation(&mut cva, &format!("c{index}"), 8, "alpha semantic-near");
    }
    let profile = cva.establish_compatibility_profile(&endpoint).unwrap();
    cva.build_archive_vector_generation(profile.id, &endpoint)
        .unwrap();
    assert_eq!(
        cva.search(profile.id, &endpoint, "alpha").unwrap().len(),
        10
    );
    assert_eq!(
        cva.search_with_config(
            profile.id,
            &endpoint,
            "alpha",
            RetrievalConfig {
                candidate_limit: 8,
                result_limit: 3,
                lexical_weight: 1.0,
                semantic_weight: 3.0,
            }
        )
        .unwrap()
        .len(),
        3
    );
}

fn candidate(fragment: Fragment, score: f64) -> SearchCandidate {
    SearchCandidate {
        fragment,
        lexical_score: 0.0,
        semantic_score: 0.0,
        combined_score: score,
        generation_id: None,
    }
}
