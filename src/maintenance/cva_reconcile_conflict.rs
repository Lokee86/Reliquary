use crate::{
    EpisodeId, FileId, FragmentId, GraphRelationKind, MemoryId, SemanticGraphRelationKind,
    SemanticNodeRef,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CvaReconcileConflict {
    ImportEpoch {
        import_id: String,
    },
    SourceTurn {
        conversation_id: String,
        node_id: String,
    },
    Branch {
        conversation_id: String,
        branch_id: String,
        existing_leaf_node_id: String,
        incoming_leaf_node_id: String,
    },
    Episode {
        episode_id: EpisodeId,
    },
    Fragment {
        fragment_id: FragmentId,
    },
    File {
        file_id: FileId,
    },
    ConversationTombstone {
        conversation_id: String,
    },
    MemoryRevision {
        memory_id: MemoryId,
        expected_revision: u64,
        current_revision: u64,
        incoming_revision: u64,
    },
    MemoryMutation {
        memory_id: MemoryId,
        mutation_id: String,
        incoming_revision: u64,
    },
    MemorySemanticMutation {
        memory_id: MemoryId,
        incoming_revision: u64,
    },
    InsomniaCompletion {
        episode_id: EpisodeId,
        existing_model: String,
        existing_version: String,
        incoming_model: String,
        incoming_version: String,
    },
    GraphRelation {
        source: MemoryId,
        target: MemoryId,
        relation_kind: GraphRelationKind,
        left_states: Vec<bool>,
        right_states: Vec<bool>,
    },
    SemanticGraphRelation {
        source: SemanticNodeRef,
        target: SemanticNodeRef,
        relation_kind: SemanticGraphRelationKind,
        left_states: Vec<bool>,
        right_states: Vec<bool>,
    },
}

impl CvaReconcileConflict {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::ImportEpoch { .. } => "import_epoch",
            Self::SourceTurn { .. } => "source_turn",
            Self::Branch { .. } => "branch",
            Self::Episode { .. } => "episode",
            Self::Fragment { .. } => "fragment",
            Self::File { .. } => "file",
            Self::ConversationTombstone { .. } => "conversation_tombstone",
            Self::MemoryRevision { .. } => "memory_revision",
            Self::MemoryMutation { .. } => "memory_mutation",
            Self::MemorySemanticMutation { .. } => "memory_semantic_mutation",
            Self::InsomniaCompletion { .. } => "insomnia_completion",
            Self::GraphRelation { .. } => "graph_relation",
            Self::SemanticGraphRelation { .. } => "semantic_graph_relation",
        }
    }
}
