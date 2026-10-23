use crate::lexical_contract::lexical_terms;
use crate::lexical_search::lexical_score;
use crate::search::combine_scores;
use crate::{
    DEFAULT_LEXICAL_WEIGHT, DEFAULT_SEARCH_CANDIDATE_LIMIT, DEFAULT_SEARCH_RESULT_LIMIT,
    DEFAULT_SEMANTIC_WEIGHT,
};

#[test]
fn old_default_weights_and_limits_are_restored() {
    assert_eq!(DEFAULT_LEXICAL_WEIGHT, 0.45);
    assert_eq!(DEFAULT_SEMANTIC_WEIGHT, 0.55);
    assert_eq!(DEFAULT_SEARCH_CANDIDATE_LIMIT, 30);
    assert_eq!(DEFAULT_SEARCH_RESULT_LIMIT, 10);
    assert_eq!(combine_scores(0.8, 0.0), 0.8);
    assert_eq!(combine_scores(0.0, 0.9), 0.9);
    assert!((combine_scores(0.8, 0.6) - 0.69).abs() < 1e-9);
}

#[test]
fn lexical_scoring_matches_original_coverage_density_formula() {
    let terms = lexical_terms("Alpha, beta alpha!");
    assert_eq!(terms, vec!["alpha", "beta"]);
    let score = lexical_score("alpha alpha alpha", &terms);
    assert!((score - 0.5375).abs() < 1e-9);
}
