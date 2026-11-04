#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticScoringError {
    EmptyQuery,
    InvalidLimit,
    InvalidQueryVector,
}

/// Pure query/limit checks, ordered to match the public semantic search contract.
pub fn validate_semantic_request(
    query: &str,
    limit: usize,
    max_limit: usize,
) -> Result<(), SemanticScoringError> {
    if query.trim().is_empty() {
        return Err(SemanticScoringError::EmptyQuery);
    }
    validate_semantic_limit(limit, max_limit)
}

pub fn validate_semantic_limit(limit: usize, max_limit: usize) -> Result<(), SemanticScoringError> {
    if (1..=max_limit).contains(&limit) {
        Ok(())
    } else {
        Err(SemanticScoringError::InvalidLimit)
    }
}

/// Validates a query's dimensions after the caller has checked the packed scalar type.
pub fn validate_query_dimensions(
    query: &[f32],
    dimensions: u32,
) -> Result<(), SemanticScoringError> {
    if usize::try_from(dimensions).ok() == Some(query.len()) {
        Ok(())
    } else {
        Err(SemanticScoringError::InvalidQueryVector)
    }
}

/// Returns a finite, non-zero vector's L2 norm.
pub fn vector_norm(vector: &[f32]) -> Option<f64> {
    let mut norm_squared = 0.0_f64;
    for value in vector {
        let value = *value as f64;
        if !value.is_finite() {
            return None;
        }
        norm_squared += value * value;
    }
    (norm_squared > 0.0).then(|| norm_squared.sqrt())
}

/// Scores one little-endian f32 row without materializing or cloning a matrix.
/// The error strings map to `SemanticSearchError::CorruptMatrix` at the retrieval boundary.
pub fn cosine_f32_row(
    row: &[u8],
    query: &[f32],
    query_norm: f64,
) -> Result<Option<f64>, &'static str> {
    if row.len() != query.len().saturating_mul(4) {
        return Err("f32 row width");
    }
    let mut dot = 0.0_f64;
    let mut row_norm_squared = 0.0_f64;
    for (encoded, query_value) in row.chunks_exact(4).zip(query) {
        let value = f32::from_le_bytes(encoded.try_into().expect("f32 width")) as f64;
        if !value.is_finite() {
            return Err("non-finite row value");
        }
        dot += value * (*query_value as f64);
        row_norm_squared += value * value;
    }
    if row_norm_squared == 0.0 {
        return Ok(None);
    }
    let score = dot / (query_norm * row_norm_squared.sqrt());
    if !score.is_finite() {
        return Err("non-finite cosine");
    }
    Ok(Some(score))
}

/// Decodes one finite little-endian f32 vector row.
pub fn decode_f32_row(row: &[u8]) -> Result<Vec<f32>, &'static str> {
    if !row.len().is_multiple_of(4) {
        return Err("f32 row width");
    }
    row.chunks_exact(4)
        .map(|chunk| {
            let value = f32::from_le_bytes(chunk.try_into().expect("f32 width"));
            value
                .is_finite()
                .then_some(value)
                .ok_or("non-finite vector")
        })
        .collect()
}

/// Orders positive row scores by descending score, breaking ties by ascending ordinal,
/// then keeps at most `limit` entries.
pub fn rank_scores(mut scores: Vec<(usize, f64)>, limit: usize) -> Vec<(usize, f64)> {
    scores.retain(|(_, score)| *score > 0.0 && score.is_finite());
    scores.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    scores.truncate(limit);
    scores
}

#[cfg(test)]
mod tests {
    use super::{
        SemanticScoringError, cosine_f32_row, decode_f32_row, rank_scores,
        validate_query_dimensions, validate_semantic_limit, validate_semantic_request, vector_norm,
    };

    fn encode(values: &[f32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }

    #[test]
    fn vector_norm_rejects_zero_and_non_finite_vectors() {
        assert_eq!(vector_norm(&[0.0, 0.0]), None);
        assert_eq!(vector_norm(&[1.0, f32::INFINITY]), None);
        assert_eq!(vector_norm(&[f32::NAN]), None);
        assert_eq!(vector_norm(&[3.0, 4.0]), Some(5.0));
    }

    #[test]
    fn cosine_scores_a_borrowed_f32_row() {
        let query = [1.0, 0.0];
        let row = encode(&[0.8, 0.6]);
        let score = cosine_f32_row(&row, &query, 1.0).unwrap().unwrap();
        assert!((score - 0.8).abs() < 1e-6);
    }

    #[test]
    fn cosine_returns_none_for_zero_row() {
        let row = encode(&[0.0, 0.0]);
        assert_eq!(cosine_f32_row(&row, &[1.0, 0.0], 1.0).unwrap(), None);
    }

    #[test]
    fn cosine_reports_malformed_or_non_finite_rows() {
        assert_eq!(cosine_f32_row(&[0, 1], &[1.0], 1.0), Err("f32 row width"));
        assert_eq!(
            cosine_f32_row(&encode(&[f32::INFINITY]), &[1.0], 1.0),
            Err("non-finite row value")
        );
        assert_eq!(
            cosine_f32_row(&encode(&[1.0]), &[1.0], 0.0),
            Err("non-finite cosine")
        );
    }

    #[test]
    fn rank_scores_orders_ties_by_ordinal_and_truncates() {
        assert_eq!(
            rank_scores(vec![(3, 0.8), (2, 1.0), (0, 0.8), (1, 0.5)], 3),
            vec![(2, 1.0), (0, 0.8), (3, 0.8)]
        );
    }

    #[test]
    fn semantic_request_checks_empty_query_before_limit() {
        assert_eq!(
            validate_semantic_request(" ", 0, 1000),
            Err(SemanticScoringError::EmptyQuery)
        );
        assert_eq!(
            validate_semantic_request("query", 0, 1000),
            Err(SemanticScoringError::InvalidLimit)
        );
        assert_eq!(validate_semantic_limit(1000, 1000), Ok(()));
        assert_eq!(validate_query_dimensions(&[1.0, 0.0], 2), Ok(()));
        assert_eq!(
            validate_query_dimensions(&[1.0], 2),
            Err(SemanticScoringError::InvalidQueryVector)
        );
    }

    #[test]
    fn decodes_f32_rows_and_rejects_malformed_values() {
        assert_eq!(decode_f32_row(&encode(&[1.0, -0.5])), Ok(vec![1.0, -0.5]));
        assert_eq!(decode_f32_row(&[0, 1]), Err("f32 row width"));
        assert_eq!(
            decode_f32_row(&encode(&[f32::NAN])),
            Err("non-finite vector")
        );
    }

    #[test]
    fn rank_scores_drops_non_positive_and_non_finite_scores() {
        assert_eq!(
            rank_scores(vec![(0, -0.1), (1, 0.0), (2, f64::NAN), (3, 0.4)], 10),
            vec![(3, 0.4)]
        );
    }
}
