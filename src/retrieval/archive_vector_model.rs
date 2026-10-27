use crate::{FragmentId, PackedVectorId};
pub use reliquary_vector_store::{ArchiveVectorId, ArchiveVectorInfo, ArchiveVectorStats};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveVectorSet {
    pub id: ArchiveVectorId,
    pub packed_vector_id: PackedVectorId,
    pub fragment_ids: Vec<FragmentId>,
}
