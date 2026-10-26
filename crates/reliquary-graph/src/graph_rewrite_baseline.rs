//! Graph-owned rewrite baseline bytes. Authorization remains with higher owners.
use super::GraphStore;
use crate::{GraphError, MemoryId, SemanticNodeRef};
use reliquary_container::Container;
use sha2::{Digest, Sha256};
const MAGIC: &[u8; 8] = b"CVAGBL01";
impl GraphStore {
    pub fn historical_baseline_identity(&self) -> Option<([u8; 16], [u8; 16], [u8; 32])> {
        self.rewrite_baseline
    }
    pub fn publish_rewrite_baseline(
        &mut self,
        container: &mut Container,
        admission: &crate::GraphAdmission,
        owner: [u8; 16],
        generation: [u8; 16],
        policy: [u8; 32],
        memories: &[MemoryId],
    ) -> Result<(), GraphError> {
        if self.rewrite_baseline.is_some() || self.graph_version() != 0 {
            return Err(GraphError::InvalidGraphVersion);
        }
        for id in memories {
            if !admission.historical_memories.contains(id) {
                return Err(GraphError::MissingMemory(*id));
            }
        }
        self.ensure_format(container)?;
        for id in memories {
            self.ensure_semantic_node(container, SemanticNodeRef::memory(*id))?;
        }
        let mut bytes = MAGIC.to_vec();
        bytes.extend(owner);
        bytes.extend(generation);
        bytes.extend(policy);
        bytes.extend(
            u32::try_from(memories.len())
                .map_err(|_| GraphError::NodeIdExhausted)?
                .to_le_bytes(),
        );
        for id in memories {
            bytes.extend(id);
        }
        bytes.extend(Sha256::digest(&bytes));
        self.ingest_rewrite_baseline(&bytes)?;
        container.append(&bytes)?;
        self.excluded_nodes.extend(
            memories
                .iter()
                .filter(|id| !admission.current_memories.contains(*id))
                .map(|id| SemanticNodeRef::memory(*id)),
        );
        self.rebuild_topologies()
    }
    pub fn ingest_rewrite_baseline(&mut self, bytes: &[u8]) -> Result<bool, GraphError> {
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
        let owner: [u8; 16] = bytes[8..24].try_into().unwrap();
        let generation: [u8; 16] = bytes[24..40].try_into().unwrap();
        let policy: [u8; 32] = bytes[40..72].try_into().unwrap();
        if owner == [0; 16] || generation == [0; 16] || policy == [0; 32] {
            return Err(GraphError::InvalidGraphVersion);
        }
        let mut seen = std::collections::HashSet::new();
        let mut previous_node = None;
        for raw in bytes[76..bytes.len() - 32].chunks_exact(32) {
            let id: MemoryId = raw.try_into().unwrap();
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
    pub fn rewrite_baseline_memories(&self) -> Option<&[MemoryId]> {
        self.rewrite_baseline
            .as_ref()
            .map(|_| self.rewrite_baseline_memories.as_slice())
    }
}
