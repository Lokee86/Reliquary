use crate::{Cva, EntityCandidateError, GraphDirection, GraphError, MemoryId, Phylactery};

macro_rules! impl_candidate_neighbors {
    ($owner:ty) => {
        impl $owner {
            pub(super) fn graph_neighbor_ids_for_candidate(
                &self,
                memory_id: MemoryId,
                limit: usize,
            ) -> Result<Vec<MemoryId>, EntityCandidateError> {
                let owner = self;
                if limit == 0 {
                    return Ok(Vec::new());
                }
                let mut ids = Vec::new();
                for direction in [GraphDirection::Outgoing, GraphDirection::Incoming] {
                    match owner.graph_neighbors(memory_id, direction) {
                        Ok(neighbors) => {
                            ids.extend(neighbors.into_iter().map(|neighbor| neighbor.memory_id))
                        }
                        Err(GraphError::MissingNode(id)) if id == memory_id => {}
                        Err(error) => return Err(error.into()),
                    }
                }
                ids.sort_by_key(|id| id.0);
                ids.dedup();
                ids.truncate(limit);
                Ok(ids)
            }
        }
    };
}

impl_candidate_neighbors!(Cva);
impl_candidate_neighbors!(Phylactery);
