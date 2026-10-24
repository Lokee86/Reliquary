use crate::packed_vector_store::PackedVectorReadView;
use crate::{PackedVectorId, PackedVectors};
use std::collections::HashMap;

/// Vectors-owned lazy row reader for External Sources similarity search.
/// Matrix hydration remains cached for the lifetime of one search operation.
pub(crate) struct ExternalVectorReader<'a> {
    packed: PackedVectorReadView<'a>,
    matrices: HashMap<PackedVectorId, PackedVectors>,
}

impl<'a> ExternalVectorReader<'a> {
    pub(crate) fn new(packed: PackedVectorReadView<'a>) -> Self {
        Self {
            packed,
            matrices: HashMap::new(),
        }
    }

    pub(crate) fn cosine_row(
        &mut self,
        matrix: [u8; 32],
        row: u32,
        dimensions: u32,
        query: &[f32],
    ) -> Result<(Vec<f32>, f64), ()> {
        let id = PackedVectorId(matrix);
        if !self.matrices.contains_key(&id) {
            let packed = self.packed.get(id).map_err(|_| ())?;
            self.matrices.insert(id, packed);
        }
        let stride = (dimensions as usize).checked_mul(4).ok_or(())?;
        let start = (row as usize).checked_mul(stride).ok_or(())?;
        let end = start.checked_add(stride).ok_or(())?;
        let bytes = self.matrices[&id].bytes().get(start..end).ok_or(())?;
        let values: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
            .collect();
        let dot = values
            .iter()
            .zip(query)
            .map(|(a, b)| f64::from(*a) * f64::from(*b))
            .sum::<f64>();
        let norm = values
            .iter()
            .map(|value| f64::from(*value).powi(2))
            .sum::<f64>()
            .sqrt()
            * query
                .iter()
                .map(|value| f64::from(*value).powi(2))
                .sum::<f64>()
                .sqrt();
        Ok((values, dot / norm))
    }
}
