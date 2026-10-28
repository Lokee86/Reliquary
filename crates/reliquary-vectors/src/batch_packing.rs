//! Pure assembly of an ordered F32 embedding batch into the existing packed
//! vector representation. Durable publication remains with the owner
//! coordinator and Vector Store.

use crate::compatibility_vector::validate_vectors;
use crate::{CompatibilityProfile, ProbeError};
use lodestone_packed::{PackedVectors, ScalarType, VectorSchema};

#[derive(Debug)]
pub enum BatchPackingError {
    Validation(ProbeError),
    Schema(String),
    Packed(String),
    InvalidMatrix,
}

impl From<ProbeError> for BatchPackingError {
    fn from(error: ProbeError) -> Self {
        Self::Validation(error)
    }
}

/// Validate `vectors` against the profile and encode them as row-major,
/// little-endian F32 bytes in their original order.
///
/// `expected_rows` is supplied by the coordinator from its ordered owner IDs;
/// the helper deliberately does not select, reorder, or retain those IDs.
pub fn pack_embedding_batch(
    profile: &CompatibilityProfile,
    vectors: Vec<Vec<f32>>,
    expected_rows: usize,
) -> Result<PackedVectors, BatchPackingError> {
    validate_vectors(
        profile.dimensions,
        profile.normalization,
        &vectors,
        expected_rows,
    )?;

    let schema = VectorSchema::new(profile.dimensions, ScalarType::F32)
        .map_err(|error| BatchPackingError::Schema(error.to_string()))?;
    let capacity = checked_f32_capacity(expected_rows, profile.dimensions)
        .ok_or(BatchPackingError::InvalidMatrix)?;

    let mut bytes = Vec::with_capacity(capacity);
    for row in vectors {
        for value in row {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    PackedVectors::from_bytes(schema, bytes)
        .map_err(|error| BatchPackingError::Packed(error.to_string()))
}

fn checked_f32_capacity(rows: usize, dimensions: u32) -> Option<usize> {
    let dimensions = usize::try_from(dimensions).ok()?;
    let row_bytes = dimensions.checked_mul(std::mem::size_of::<f32>())?;
    rows.checked_mul(row_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CompatibilityProfileId, EmbeddingMode, VectorNormalization};
    use reliquary_vector_store::CompatibilityProbeReference;

    fn profile(dimensions: u32, normalization: VectorNormalization) -> CompatibilityProfile {
        CompatibilityProfile {
            id: CompatibilityProfileId([0; 32]),
            dimensions,
            normalization,
            probe_suite_version: 1,
            compatibility_policy_version: 3,
            references: vec![CompatibilityProbeReference {
                mode: EmbeddingMode::Document,
                vector: vec![1.0],
            }],
        }
    }

    #[test]
    fn encodes_f32_little_endian_in_input_row_order() {
        let profile = profile(2, VectorNormalization::None);
        let rows = vec![vec![1.0, -2.5], vec![0.0, f32::from_bits(0x3f00_0001)]];

        let expected = [
            0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x20, 0xc0, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x3f,
        ];
        let packed = pack_embedding_batch(&profile, rows, 2).unwrap();
        assert_eq!(packed.bytes(), expected);
        assert_eq!(packed.count(), 2);
        assert_eq!(packed.schema().scalar, ScalarType::F32);
    }

    #[test]
    fn rejects_count_mismatch_before_encoding() {
        let profile = profile(2, VectorNormalization::None);
        let error = pack_embedding_batch(&profile, vec![vec![1.0, 2.0]], 2).unwrap_err();
        assert!(matches!(
            error,
            BatchPackingError::Validation(ProbeError::InvalidProfile("embedding count mismatch"))
        ));
    }

    #[test]
    fn rejects_wrong_width_and_non_finite_values() {
        let profile = profile(2, VectorNormalization::None);
        assert!(matches!(
            pack_embedding_batch(&profile, vec![vec![1.0]], 1),
            Err(BatchPackingError::Validation(ProbeError::InvalidProfile(
                "invalid embedding vector"
            )))
        ));
        assert!(matches!(
            pack_embedding_batch(&profile, vec![vec![1.0, f32::NAN]], 1),
            Err(BatchPackingError::Validation(ProbeError::InvalidProfile(
                "invalid embedding vector"
            )))
        ));
    }

    #[test]
    fn enforces_profile_normalization_policy() {
        let profile = profile(2, VectorNormalization::L2);
        assert!(pack_embedding_batch(&profile, vec![vec![0.6, 0.8]], 1).is_ok());
        assert!(matches!(
            pack_embedding_batch(&profile, vec![vec![1.0, 1.0]], 1),
            Err(BatchPackingError::Validation(ProbeError::InvalidProfile(
                "embedding not normalized"
            )))
        ));
    }

    #[test]
    fn capacity_calculation_rejects_overflow() {
        assert!(matches!(checked_f32_capacity(usize::MAX, 4), None));
    }

    #[test]
    fn invalid_schema_is_distinct_from_embedding_validation() {
        let profile = profile(0, VectorNormalization::None);
        assert!(matches!(
            pack_embedding_batch(&profile, vec![], 0),
            Err(BatchPackingError::Schema(_))
        ));
    }
}
