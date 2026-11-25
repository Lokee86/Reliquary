use super::{OwnerSessionError, RelSession, RelWriter};
use reliquary_ego_store::EgoWebSynthesis;

pub(super) const REL_DEPENDENCY_MAGIC: &[u8; 8] = b"RELDEP01";
pub const MAX_REL_DEPENDENCIES: usize = 4096;
const HEADER_LEN: usize = 36;
const ID_LEN: usize = 16;

/// Current owner-local declaration. The default denotes a REL without a record.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RelDependencyDeclaration {
    pub generation: u64,
    pub owner_ids: Vec<[u8; 16]>,
}

pub fn canonical_rel_dependency_ids(
    owner: [u8; 16],
    mut owner_ids: Vec<[u8; 16]>,
) -> Result<Vec<[u8; 16]>, OwnerSessionError> {
    if owner_ids.len() > MAX_REL_DEPENDENCIES
        || owner_ids.iter().any(|id| *id == owner || *id == [0; 16])
    {
        return Err(OwnerSessionError::ManagementEvidence);
    }
    owner_ids.sort_unstable();
    owner_ids.dedup();
    Ok(owner_ids)
}

impl RelDependencyDeclaration {
    pub(super) fn replay(
        &mut self,
        owner: [u8; 16],
        payload: &[u8],
    ) -> Result<(), OwnerSessionError> {
        let invalid = || OwnerSessionError::Record("invalid REL dependency declaration".into());
        let header = payload.get(..HEADER_LEN).ok_or_else(invalid)?;
        let generation = u64::from_le_bytes(header[24..32].try_into().unwrap());
        let count = u32::from_le_bytes(header[32..36].try_into().unwrap()) as usize;
        if header[8..24] != owner
            || count > MAX_REL_DEPENDENCIES
            || payload.len() != HEADER_LEN + count * ID_LEN
            || self.generation.checked_add(1) != Some(generation)
        {
            return Err(invalid());
        }
        let owner_ids: Vec<[u8; 16]> = payload[HEADER_LEN..]
            .chunks_exact(ID_LEN)
            .map(|bytes| bytes.try_into().unwrap())
            .collect();
        if owner_ids.windows(2).any(|pair| pair[0] >= pair[1])
            || owner_ids.iter().any(|id| *id == owner || *id == [0; 16])
        {
            return Err(invalid());
        }
        *self = Self {
            generation,
            owner_ids,
        };
        Ok(())
    }

    fn encode(&self, owner: [u8; 16]) -> Vec<u8> {
        let mut payload = Vec::with_capacity(HEADER_LEN + self.owner_ids.len() * ID_LEN);
        payload.extend_from_slice(REL_DEPENDENCY_MAGIC);
        payload.extend_from_slice(&owner);
        payload.extend_from_slice(&self.generation.to_le_bytes());
        payload.extend_from_slice(&(self.owner_ids.len() as u32).to_le_bytes());
        for id in &self.owner_ids {
            payload.extend_from_slice(id);
        }
        payload
    }
}

impl RelSession {
    pub fn rel_dependencies(&self) -> &RelDependencyDeclaration {
        &self.rel_dependencies
    }

    /// Edges used by the Gateway must be known even after publication errors.
    pub fn rel_dependency_ids(&self) -> Result<Vec<[u8; 16]>, OwnerSessionError> {
        self.ensure_write_certain()?;
        Ok(self.rel_dependencies.owner_ids.clone())
    }

    /// Ambient context uses synthesis authored against the current Memory cut.
    pub fn current_ego_synthesis(&self) -> Option<&EgoWebSynthesis> {
        self.common
            .ego
            .synthesis()
            .filter(|value| value.source_memory_version == self.memory_version())
            .filter(|_| {
                self.archive_source_removal_version() == 0
                    || self
                        .common
                        .memories
                        .memory_ids()
                        .all(|id| !self.memory_source_removed(id))
            })
    }
}

impl RelWriter {
    /// Publish a complete declaration after the Gateway has validated the mounted graph.
    pub fn replace_rel_dependencies(
        &mut self,
        expected_generation: u64,
        owner_ids: Vec<[u8; 16]>,
    ) -> Result<(RelDependencyDeclaration, bool), OwnerSessionError> {
        self.ensure_write_ready()?;
        let owner_ids = canonical_rel_dependency_ids(self.owner_id(), owner_ids)?;
        let current = self.session.rel_dependencies();
        if current.generation != expected_generation {
            return Err(OwnerSessionError::ManagementStale);
        }
        if current.owner_ids == owner_ids {
            return Ok((current.clone(), false));
        }
        let next = RelDependencyDeclaration {
            generation: current.generation.checked_add(1).ok_or_else(|| {
                OwnerSessionError::Record("REL dependency generation exhausted".into())
            })?,
            owner_ids,
        };
        let payload = next.encode(self.owner_id());
        if let Err(error) = self
            .container
            .append(&payload)
            .and_then(|_| self.container.sync())
        {
            self.session.write_poisoned = true;
            return Err(error.into());
        }
        self.session.writer_lease_proof = self.container.writer_lease_proof();
        self.session.rel_dependencies = next.clone();
        Ok((next, true))
    }
}
