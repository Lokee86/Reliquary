//! Fresh-history Memory projection admission, independent of obsolete edges.
use super::GraphStore;
use crate::{Container, GraphError, HistoryGenerationId, MemoryId, SemanticNodeRef};
use sha2::{Digest, Sha256};
const MAGIC: &[u8; 8] = b"CVAGBL01";
impl GraphStore {
    #[cfg(test)]
    pub(crate) fn historical_baseline_identity(
        &self,
    ) -> Option<([u8; 16], crate::HistoryGenerationId, [u8; 32])> {
        self.rewrite_baseline
    }
    pub(crate) fn publish_rewrite_baseline(
        &mut self,
        container: &mut Container,
        owner: [u8; 16],
        generation: HistoryGenerationId,
        policy: [u8; 32],
        memories: &[MemoryId],
    ) -> Result<(), GraphError> {
        container.require_storage_format_writable()?;
        if self.rewrite_baseline.is_some() || self.graph_version() != 0 {
            return Err(GraphError::InvalidGraphVersion);
        }
        self.ensure_format(container)?;
        for id in memories {
            self.ensure_semantic_node(container, SemanticNodeRef::memory(*id))?;
        }
        let mut bytes = MAGIC.to_vec();
        bytes.extend(owner);
        bytes.extend(generation.as_bytes());
        bytes.extend(policy);
        bytes.extend(
            u32::try_from(memories.len())
                .map_err(|_| GraphError::NodeIdExhausted)?
                .to_le_bytes(),
        );
        for id in memories {
            bytes.extend(id.0);
        }
        bytes.extend(Sha256::digest(&bytes));
        self.ingest_rewrite_baseline(&bytes)?;
        container.append(&bytes)?;
        self.rebuild_topologies()
    }
    pub(crate) fn ingest_rewrite_baseline(&mut self, bytes: &[u8]) -> Result<bool, GraphError> {
        if !bytes.starts_with(MAGIC) {
            return Ok(false);
        }
        if self.rewrite_baseline.is_some() || self.graph_version() != 0 || bytes.len() < 108 {
            return Err(GraphError::InvalidGraphVersion);
        }
        let count = u32::from_le_bytes(bytes[72..76].try_into().unwrap()) as usize;
        if count.checked_mul(32).and_then(|n| n.checked_add(108)) != Some(bytes.len())
            || Sha256::digest(&bytes[..bytes.len() - 32]).as_slice() != &bytes[bytes.len() - 32..]
        {
            return Err(GraphError::InvalidNodeMapping);
        }
        let owner = bytes[8..24].try_into().unwrap();
        let generation = HistoryGenerationId::from_bytes(bytes[24..40].try_into().unwrap());
        let policy = bytes[40..72].try_into().unwrap();
        if owner == [0; 16] || generation == HistoryGenerationId::ZERO || policy == [0; 32] {
            return Err(GraphError::InvalidGraphVersion);
        }
        let mut seen = std::collections::HashSet::new();
        let mut previous_node = None;
        for raw in bytes[76..bytes.len() - 32].chunks_exact(32) {
            let id = MemoryId(raw.try_into().unwrap());
            if !seen.insert(id)
                || !self
                    .node_by_semantic
                    .contains_key(&SemanticNodeRef::memory(id))
            {
                return Err(GraphError::InvalidNodeMapping);
            }
            let node = self.node_by_semantic[&SemanticNodeRef::memory(id)].0;
            if previous_node.is_some_and(|previous| node <= previous) {
                return Err(GraphError::InvalidNodeMapping);
            }
            previous_node = Some(node);
            self.memory_relation_nodes.insert(id);
            self.rewrite_baseline_memories.push(id);
        }
        self.rewrite_baseline = Some((owner, generation, policy));
        Ok(true)
    }
    pub(crate) fn rewrite_baseline_memories(&self) -> Option<&[MemoryId]> {
        self.rewrite_baseline
            .as_ref()
            .map(|_| self.rewrite_baseline_memories.as_slice())
    }
    pub(crate) fn validate_rewrite_baseline(
        &self,
        owner: Option<[u8; 16]>,
        genesis: Option<&crate::RewriteGenesis>,
    ) -> Result<(), GraphError> {
        if let Some((baseline_owner, generation, policy)) = self.rewrite_baseline {
            let Some(genesis) = genesis else {
                return Err(GraphError::InvalidGraphVersion);
            };
            if owner != Some(baseline_owner)
                || genesis.generation != generation
                || genesis.policy_digest != policy
            {
                return Err(GraphError::InvalidGraphVersion);
            }
        }
        Ok(())
    }
}
