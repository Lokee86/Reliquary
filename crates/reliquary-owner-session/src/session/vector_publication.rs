//! Owner-local source validation and content-addressed vector publication.
use super::*;
use reliquary_memory_core::PayloadState;
use reliquary_vector_store::{
    AcceptedMemoryMapping, MemoryBodyId as VectorBodyId, VectorNormalization,
};
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
pub struct MemoryVectorInput {
    owner_id: [u8; 16],
    profile: reliquary_vector_store::CompatibilityProfile,
    sources: Vec<MemoryRevision>,
    existing: Vec<Option<Vec<f32>>>,
}
impl MemoryVectorInput {
    pub fn owner_id(&self) -> [u8; 16] {
        self.owner_id
    }
    pub fn profile(&self) -> &reliquary_vector_store::CompatibilityProfile {
        &self.profile
    }
    pub fn sources(&self) -> &[MemoryRevision] {
        &self.sources
    }
    pub fn existing(&self) -> &[Option<Vec<f32>>] {
        &self.existing
    }
}

#[derive(Clone, Debug)]
pub struct MemoryVectorRow {
    pub memory_id: MemoryId,
    pub revision: u64,
    pub body_id: reliquary_memory_core::MemoryBodyId,
    pub vector: Vec<f32>,
}

impl CommonState {
    fn vector_input(
        &self,
        owner_id: [u8; 16],
        profile: CompatibilityProfileId,
        ids: &[MemoryId],
    ) -> Result<MemoryVectorInput, OwnerSessionError> {
        if ids.is_empty()
            || ids.len() > 64
            || ids.iter().copied().collect::<BTreeSet<_>>().len() != ids.len()
        {
            return Err(invalid("invalid vector source population"));
        }
        let profile = self
            .profiles
            .get_active(profile)
            .ok_or_else(|| invalid("embedding profile is unavailable"))?
            .clone();
        if ids
            .len()
            .checked_mul(profile.dimensions as usize)
            .and_then(|n| n.checked_mul(4))
            .is_none_or(|n| n > 8 << 20)
        {
            return Err(invalid("vector source population exceeds response bound"));
        }
        let mut sources = Vec::new();
        let mut existing = Vec::new();
        for id in ids {
            let revision = self.memories.current(*id)?;
            let PayloadState::Available(body) = &revision.payload else {
                return Err(invalid("vector body is unavailable"));
            };
            if revision.record.archived
                || body
                    .title
                    .len()
                    .saturating_add(body.content.len())
                    .saturating_add(1)
                    > 128 << 10
            {
                return Err(invalid("invalid vector source body"));
            }
            let row = self
                .memory_vectors
                .location(profile.id, VectorBodyId(revision.record.body_id.0))
                .map(|location| {
                    let mapping = self
                        .memory_vectors
                        .info(location.set_id)
                        .ok_or(MemoryVectorError::MissingObject)?;
                    Ok::<_, OwnerSessionError>(
                        self.packed_vectors
                            .read_f32_row(mapping.packed_vector_id, location.row)?,
                    )
                })
                .transpose()?;
            sources.push(revision);
            existing.push(row);
        }
        Ok(MemoryVectorInput {
            owner_id,
            profile,
            sources,
            existing,
        })
    }

    fn publish_vector_rows(
        &mut self,
        container: &mut Container,
        profile: CompatibilityProfileId,
        rows: &[MemoryVectorRow],
    ) -> Result<bool, OwnerSessionError> {
        let ids = rows.iter().map(|r| r.memory_id).collect::<Vec<_>>();
        let input = self.vector_input(container.owner_id(), profile, &ids)?;
        let mut missing = BTreeMap::new();
        for ((row, source), existing) in rows.iter().zip(&input.sources).zip(input.existing) {
            if row.revision != source.record.revision || row.body_id != source.record.body_id {
                return Err(invalid("vector source revision changed"));
            }
            let norm = row
                .vector
                .iter()
                .map(|v| f64::from(*v).powi(2))
                .sum::<f64>()
                .sqrt();
            if row.vector.len() != input.profile.dimensions as usize
                || row.vector.iter().any(|v| !v.is_finite())
                || !norm.is_finite()
                || (input.profile.normalization == VectorNormalization::L2
                    && (norm - 1.0).abs() > 1e-4)
            {
                return Err(invalid("vector row is incompatible with its profile"));
            }
            if let Some(existing) = existing {
                if !same_row(&existing, &row.vector) {
                    return Err(invalid("vector binding conflicts with accepted bytes"));
                }
            } else if let Some(prior) = missing.insert(row.body_id, row.vector.clone()) {
                if !same_row(&prior, &row.vector) {
                    return Err(invalid("same body has inconsistent vector rows"));
                }
            }
        }
        if missing.is_empty() {
            return Ok(false);
        }
        if missing
            .len()
            .checked_mul(input.profile.dimensions as usize)
            .and_then(|n| n.checked_mul(4))
            .is_none_or(|n| n > 8 << 20)
        {
            return Err(invalid("vector response exceeds publication bound"));
        }
        let body_ids = missing
            .keys()
            .map(|id| VectorBodyId(id.0))
            .collect::<Vec<_>>();
        let vectors = missing.into_values().collect::<Vec<_>>();
        let packed = self.packed_vectors.put_f32_rows(container, &vectors)?;
        container.sync()?;
        self.memory_vectors.put(
            container,
            &self.packed_vectors,
            &self.profiles,
            AcceptedMemoryMapping {
                profile,
                packed,
                packed_rows: body_ids.len() as u64,
                body_ids,
            },
        )?;
        container.sync()?;
        let affected = rows
            .iter()
            .flat_map(|row| self.memories.memory_ids_for_body(row.body_id))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        self.refresh_dream_index(&affected)?;
        Ok(true)
    }
}
fn invalid(message: &str) -> OwnerSessionError {
    OwnerSessionError::Record(message.into())
}
fn same_row(left: &[f32], right: &[f32]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(a, b)| a.to_bits() == b.to_bits())
}

impl RelSession {
    pub fn memory_vector_input(
        &self,
        profile: CompatibilityProfileId,
        ids: &[MemoryId],
    ) -> Result<MemoryVectorInput, OwnerSessionError> {
        self.common.vector_input(self.owner_id, profile, ids)
    }
}
impl PhySession {
    pub fn memory_vector_input(
        &self,
        profile: CompatibilityProfileId,
        ids: &[MemoryId],
    ) -> Result<MemoryVectorInput, OwnerSessionError> {
        self.common.vector_input(self.owner_id, profile, ids)
    }
}
impl PhyWriter {
    pub fn memory_vector_input(
        &self,
        profile: CompatibilityProfileId,
        ids: &[MemoryId],
    ) -> Result<MemoryVectorInput, OwnerSessionError> {
        self.common.vector_input(self.owner_id, profile, ids)
    }
}

mod writers;

#[cfg(test)]
mod tests;
