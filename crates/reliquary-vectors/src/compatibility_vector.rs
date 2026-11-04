use crate::ProbeError;
use reliquary_vector_store::VectorNormalization;
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
        if vector.len() != dimensions || vector.iter().any(|v| !v.is_finite()) {
            return Err(ProbeError::InvalidProfile("invalid embedding vector"));
        }
        if normalization == VectorNormalization::L2 {
            let norm = vector
                .iter()
                .map(|v| (*v as f64) * (*v as f64))
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
    let (mut dot, mut lnorm, mut rnorm) = (0.0, 0.0, 0.0);
    for (&l, &r) in left.iter().zip(right) {
        let (l, r) = (l as f64, r as f64);
        if !l.is_finite() || !r.is_finite() {
            return None;
        }
        dot += l * r;
        lnorm += l * l;
        rnorm += r * r
    }
    if lnorm == 0.0 || rnorm == 0.0 {
        return None;
    }
    Some(dot / (lnorm * rnorm).sqrt())
}
