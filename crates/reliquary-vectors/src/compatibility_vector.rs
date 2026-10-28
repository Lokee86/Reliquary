use crate::{ProbeError, VectorNormalization};

pub fn validate_vectors(
    dimensions: u32,
    normalization: VectorNormalization,
    vectors: &[Vec<f32>],
    expected: usize,
) -> Result<(), ProbeError> {
    if vectors.len() != expected {
        return Err(ProbeError::InvalidProfile("embedding count mismatch"));
    }
    let dimensions = usize::try_from(dimensions).map_err(|_| ProbeError::SizeOverflow)?;
    for vector in vectors {
        if vector.len() != dimensions || vector.iter().any(|value| !value.is_finite()) {
            return Err(ProbeError::InvalidProfile("invalid embedding vector"));
        }
        if normalization == VectorNormalization::L2 {
            let norm = vector
                .iter()
                .map(|value| (*value as f64) * (*value as f64))
                .sum::<f64>()
                .sqrt();
            if (norm - 1.0).abs() > 1e-4 {
                return Err(ProbeError::InvalidProfile("embedding not normalized"));
            }
        }
    }
    Ok(())
}

pub fn cosine(left: &[f32], right: &[f32]) -> Option<f64> {
    if left.is_empty() || left.len() != right.len() {
        return None;
    }
    let (mut dot, mut left_norm, mut right_norm) = (0.0, 0.0, 0.0);
    for (&left, &right) in left.iter().zip(right) {
        let (left, right) = (left as f64, right as f64);
        if !left.is_finite() || !right.is_finite() {
            return None;
        }
        dot += left * right;
        left_norm += left * left;
        right_norm += right * right;
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        return None;
    }
    Some(dot / (left_norm * right_norm).sqrt())
}
