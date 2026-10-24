//! Coherent borrowed Memory retrieval facts from one concrete mounted owner.
use super::{PhylacteryOwner, ReliquaryOwner};
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryReadView;
use crate::memory_vector_store::MemoryVectorReadView;
use crate::{CommunitySnapshot, CompatibilityProfileId};

pub(crate) struct MemoryRetrievalFacts<'a> {
    pub(crate) memories: MemoryReadView<'a>,
    pub(crate) graph: &'a GraphStore,
    pub(crate) communities: Option<&'a CommunitySnapshot>,
    pub(crate) vectors: MemoryVectorReadView<'a>,
    pub(crate) memory_version: u64,
    pub(crate) vector_bindings: usize,
}

macro_rules! retrieval_facts {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn memory_retrieval_facts(
                &self,
                profile: CompatibilityProfileId,
            ) -> MemoryRetrievalFacts<'_> {
                MemoryRetrievalFacts {
                    memories: MemoryReadView::new(&self.memories, &self.container),
                    graph: &self.graph,
                    communities: self.communities.latest(),
                    vectors: MemoryVectorReadView::new(
                        &self.memory_vectors,
                        &self.packed_vectors,
                        &self.container,
                    ),
                    memory_version: self.memories.memory_version(),
                    vector_bindings: self.memory_vectors.binding_count(profile),
                }
            }
        }
    };
}
retrieval_facts!(ReliquaryOwner);
retrieval_facts!(PhylacteryOwner);
