use lodestone_packed::VectorSchema;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddingMode {
    Query,
    Document,
}
impl EmbeddingMode {
    pub fn tag(self) -> u8 {
        match self {
            Self::Query => 1,
            Self::Document => 2,
        }
    }
    pub fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Query),
            2 => Some(Self::Document),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VectorNormalization {
    None,
    L2,
}
impl VectorNormalization {
    pub fn tag(self) -> u8 {
        match self {
            Self::None => 0,
            Self::L2 => 1,
        }
    }
    pub fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::None),
            1 => Some(Self::L2),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompatibilityProbeReference {
    pub mode: EmbeddingMode,
    pub vector: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompatibilityProfile {
    pub id: CompatibilityProfileId,
    pub dimensions: u32,
    pub normalization: VectorNormalization,
    pub probe_suite_version: u32,
    pub compatibility_policy_version: u32,
    pub references: Vec<CompatibilityProbeReference>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompatibilityProfileId(pub [u8; 32]);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArchiveVectorId(pub [u8; 32]);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArchiveFragmentId(pub [u8; 32]);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryBodyId(pub [u8; 32]);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryVectorId(pub [u8; 32]);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PackedVectorId(pub [u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackedVectorInfo {
    pub id: PackedVectorId,
    pub schema: VectorSchema,
    pub count: u64,
    pub byte_len: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackedVectorStats {
    pub objects: usize,
    pub rows: u64,
    pub matrix_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveVectorInfo {
    pub id: ArchiveVectorId,
    pub packed_vector_id: PackedVectorId,
    pub rows: u64,
    pub max_fragment_archive_version: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveVectorStats {
    pub objects: usize,
    pub rows: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveVectorRecord {
    pub id: ArchiveVectorId,
    pub packed_vector_id: PackedVectorId,
    pub fragment_ids: Vec<ArchiveFragmentId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryVectorInfo {
    pub id: MemoryVectorId,
    pub compatibility_profile_id: CompatibilityProfileId,
    pub packed_vector_id: PackedVectorId,
    pub rows: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryVectorLocation {
    pub set_id: MemoryVectorId,
    pub row: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryVectorRecord {
    pub id: MemoryVectorId,
    pub compatibility_profile_id: CompatibilityProfileId,
    pub packed_vector_id: PackedVectorId,
    pub memory_body_ids: Vec<MemoryBodyId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryVectorStats {
    pub objects: usize,
    pub rows: u64,
    pub bindings: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct VectorGenerationId(pub [u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VectorGeneration {
    pub id: VectorGenerationId,
    pub compatibility_profile_id: CompatibilityProfileId,
    pub archive_vector_id: ArchiveVectorId,
    pub source_archive_version: u64,
    pub vector_version: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VectorGenerationStats {
    pub generations: usize,
    pub active_profiles: usize,
    pub vector_version: u64,
}
