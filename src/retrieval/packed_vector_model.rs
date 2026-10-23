use lodestone_packed::VectorSchema;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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
pub enum VectorNormalization {
    None,
    L2,
}

impl VectorNormalization {
    pub(crate) fn tag(self) -> u8 {
        match self {
            Self::None => 0,
            Self::L2 => 1,
        }
    }

    pub(crate) fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::None),
            1 => Some(Self::L2),
            _ => None,
        }
    }
}
