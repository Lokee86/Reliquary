use crate::{CommunitySemanticName, CommunitySnapshot, Entity, Memory, SemanticGraphRelation};

pub type RuntimeKnowledgeState = (
    Vec<Memory>,
    Vec<Entity>,
    Vec<SemanticGraphRelation>,
    Option<CommunitySnapshot>,
    Vec<CommunitySemanticName>,
);
