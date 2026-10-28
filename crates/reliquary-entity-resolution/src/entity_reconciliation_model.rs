use crate::EntityId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityReconciliationRelation {
    SameIdentity,
    RelatedDistinct,
    Uncertain,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityReconciliationCandidate {
    pub left: EntityId,
    pub right: EntityId,
    pub deterministic: bool,
    pub score: u16,
}
