#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PhylacteryProfile {
    pub display_name: Option<String>,
}

pub const MAX_PHYLACTERY_PROFILE_NAME_BYTES: usize = 256;
