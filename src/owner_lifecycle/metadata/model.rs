#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RelMetadata {
    pub type_label: Option<String>,
    pub dependencies: Vec<String>,
}

pub const MAX_REL_TYPE_LABEL_BYTES: usize = 256;
pub const MAX_REL_DEPENDENCIES: usize = 4096;
pub const MAX_REL_DEPENDENCY_ID_BYTES: usize = 128;
