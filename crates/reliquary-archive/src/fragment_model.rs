#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FragmentId(pub [u8; 32]);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fragment {
    pub id: FragmentId,
    pub conversation_id: String,
    pub start_node_id: String,
    pub end_node_id: String,
}

pub const DEFAULT_FRAGMENT_TURNS: usize = 8;
pub const DEFAULT_FRAGMENT_OVERLAP: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FragmentConfig {
    pub turns: usize,
    pub overlap: usize,
}

impl Default for FragmentConfig {
    fn default() -> Self {
        Self {
            turns: DEFAULT_FRAGMENT_TURNS,
            overlap: DEFAULT_FRAGMENT_OVERLAP,
        }
    }
}
