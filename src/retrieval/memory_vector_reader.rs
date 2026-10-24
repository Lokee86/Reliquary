use crate::memory_store::MemoryReadView;
use crate::memory_vector_store::MemoryVectorReadView;
use crate::{CompatibilityProfileId, MemoryBodyId, MemoryVectorId, ScalarType};
use std::collections::HashMap;

/// Similarity is a Vectors service contract. The owner codec supplies the
/// finite/shape-safe arithmetic shared with compatibility verification.
pub(crate) fn cosine(left: &[f32], right: &[f32]) -> Option<f64> {
    crate::compatibility_vector::cosine(left, right)
}

/// Vector-service reader over the already mounted immutable owner handles.
/// Lazy payload hydration and scalar validation stay with Vectors.
pub(crate) struct MemoryVectorReader<'a> {
    owner: MemoryVectorReadView<'a>,
}
impl<'a> MemoryVectorReader<'a> {
    pub(crate) fn new(owner: MemoryVectorReadView<'a>) -> Self {
        Self { owner }
    }
}

pub(crate) enum MemoryVectorReadError {
    MemoryVectors(crate::MemoryVectorError),
    PackedVectors(crate::PackedVectorError),
    UnsupportedScalar(ScalarType),
    CorruptVector(&'static str),
}
impl From<crate::MemoryVectorError> for MemoryVectorReadError {
    fn from(value: crate::MemoryVectorError) -> Self {
        Self::MemoryVectors(value)
    }
}
impl From<crate::PackedVectorError> for MemoryVectorReadError {
    fn from(value: crate::PackedVectorError) -> Self {
        Self::PackedVectors(value)
    }
}

impl MemoryVectorReader<'_> {
    pub(crate) fn load(
        &self,
        memories: &MemoryReadView<'_>,
        profile_id: CompatibilityProfileId,
    ) -> Result<HashMap<MemoryBodyId, Vec<f32>>, MemoryVectorReadError> {
        let mut by_set: HashMap<MemoryVectorId, Vec<(MemoryBodyId, u64)>> = HashMap::new();
        for body_id in memories.current_body_ids() {
            if let Some(location) = self.owner.location(profile_id, body_id) {
                by_set
                    .entry(location.set_id)
                    .or_default()
                    .push((body_id, location.row));
            }
        }
        let mut output = HashMap::new();
        for (set_id, locations) in by_set {
            let set = self.owner.get(set_id)?;
            let packed = self.owner.packed(set.packed_vector_id)?;
            if packed.schema().scalar != ScalarType::F32 {
                return Err(MemoryVectorReadError::UnsupportedScalar(
                    packed.schema().scalar,
                ));
            }
            for (body_id, row) in locations {
                let ordinal = usize::try_from(row)
                    .map_err(|_| MemoryVectorReadError::CorruptVector("row ordinal"))?;
                let bytes = packed
                    .row(ordinal)
                    .ok_or(MemoryVectorReadError::CorruptVector("missing row"))?;
                output.insert(body_id, decode_f32_row(bytes)?);
            }
        }
        Ok(output)
    }
}

fn decode_f32_row(bytes: &[u8]) -> Result<Vec<f32>, MemoryVectorReadError> {
    if !bytes.len().is_multiple_of(4) {
        return Err(MemoryVectorReadError::CorruptVector("f32 row width"));
    }
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            let value = f32::from_le_bytes(chunk.try_into().expect("f32 width"));
            value
                .is_finite()
                .then_some(value)
                .ok_or(MemoryVectorReadError::CorruptVector("non-finite vector"))
        })
        .collect()
}
