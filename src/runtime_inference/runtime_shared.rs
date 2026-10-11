#[derive(Debug)]
pub enum ReliquaryRuntimeHostError {
    Operation(String),
    LockPoisoned,
    ThreadPanicked,
}

impl std::fmt::Display for ReliquaryRuntimeHostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Operation(message) => f.write_str(message),
            Self::LockPoisoned => f.write_str("Reliquary runtime host lock is poisoned"),
            Self::ThreadPanicked => f.write_str("Reliquary runtime host worker panicked"),
        }
    }
}

impl std::error::Error for ReliquaryRuntimeHostError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeSemanticOwnerKind {
    Reliquary,
    Phylactery,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSemanticOwner {
    pub owner_id: String,
    pub kind: RuntimeSemanticOwnerKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeEchoTurn {
    pub message_id: String,
    pub events: Vec<crate::EchoEvent>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeEchoSource {
    pub owner_id: String,
    pub conversation_id: String,
    pub start_message_id: String,
    pub end_message_id: String,
    pub turns: Vec<RuntimeEchoTurn>,
}

pub const MAX_MEMORY_SEARCH_QUERY_BYTES: usize = 512;
pub const DEFAULT_MEMORY_SEARCH_RESULTS: usize = 5;
pub const MAX_MEMORY_SEARCH_RESULTS: usize = 10;
pub const DEFAULT_MEMORY_SEARCH_DEPTH: usize = 3;
pub const MAX_MEMORY_SEARCH_DEPTH: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub struct MemorySearchItem {
    pub memory: crate::Memory,
    pub seed_score: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MemorySearchLane {
    pub owner_id: String,
    pub retrieval: crate::MemoryRetrievalResult,
    pub items: Vec<MemorySearchItem>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MemorySearchResult {
    pub reliquary: MemorySearchLane,
    pub phylactery: Option<MemorySearchLane>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VisibleMemorySearchOwner {
    pub owner: RuntimeSemanticOwner,
    pub lane: Option<MemorySearchLane>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VisibleMemorySearchResult {
    pub owners: Vec<VisibleMemorySearchOwner>,
    pub traversal_depth: usize,
}

#[path = "runtime_host_routes.rs"]
mod routes;
pub use routes::ReliquaryRuntimeRoutes;
