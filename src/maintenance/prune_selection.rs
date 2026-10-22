//! Concrete target selection for the existing REL and PHY prune operations.

use crate::owner_lifecycle::LifecycleTarget;
use crate::{CompatibilityProfileId, EntityId, MemoryId, RelationshipId};
use std::collections::{BTreeSet, HashSet};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HistoryPruneRequest {
    pub targets: Vec<LifecycleTarget>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryPruneReport {
    pub pruned_conversations: usize,
    pub pruned_content_objects: usize,
    pub pruned_memory_bodies: usize,
    pub pruned_packed_vectors: usize,
    pub destroyed_body_bytes: u64,
    pub destroyed_packed_bytes: u64,
    pub source_file_bytes: u64,
    pub output_file_bytes: u64,
    pub manifest_digest: [u8; 32],
}

#[derive(Debug)]
pub(crate) struct ReliquaryPruneTargets {
    pub(crate) conversations: BTreeSet<String>,
    pub(crate) memories: HashSet<MemoryId>,
    pub(crate) compatibility_profiles: HashSet<CompatibilityProfileId>,
    pub(crate) descriptions: Vec<LifecycleTarget>,
}

#[derive(Debug)]
pub(crate) struct PhylacteryPruneTargets {
    pub(crate) memories: BTreeSet<MemoryId>,
    pub(crate) entities: BTreeSet<EntityId>,
    pub(crate) relationships: BTreeSet<RelationshipId>,
    pub(crate) compatibility_profiles: HashSet<CompatibilityProfileId>,
}

pub(crate) fn select_reliquary_targets(
    targets: Vec<LifecycleTarget>,
) -> Result<ReliquaryPruneTargets, &'static str> {
    let mut conversations = BTreeSet::new();
    let mut compatibility_profiles = HashSet::new();
    let mut memories = HashSet::new();
    let mut descriptions = Vec::new();
    for target in targets {
        match target {
            LifecycleTarget::Conversation(id) => {
                if !conversations.insert(id) {
                    return Err("duplicate prune target");
                }
            }
            target @ (LifecycleTarget::Entity(_) | LifecycleTarget::Relationship(_)) => {
                // REL's established policy checks duplicate description targets later,
                // alongside their retention proofs.
                descriptions.push(target);
            }
            LifecycleTarget::Memory(id) => {
                if !memories.insert(id) {
                    return Err("duplicate prune target");
                }
            }
            LifecycleTarget::CompatibilityProfile(id) => {
                if !compatibility_profiles.insert(id) {
                    return Err("duplicate prune target");
                }
            }
            _ => return Err("selected domain has no implemented retention contract"),
        }
    }
    if conversations.is_empty()
        && compatibility_profiles.is_empty()
        && memories.is_empty()
        && descriptions.is_empty()
    {
        return Err("pruning requires explicitly selected terminal tombstones");
    }
    Ok(ReliquaryPruneTargets {
        conversations,
        memories,
        compatibility_profiles,
        descriptions,
    })
}

pub(crate) fn select_phylactery_targets(
    targets: Vec<LifecycleTarget>,
) -> Result<PhylacteryPruneTargets, &'static str> {
    let mut memories = BTreeSet::new();
    let mut entities = BTreeSet::new();
    let mut relationships = BTreeSet::new();
    let mut compatibility_profiles = HashSet::new();
    for target in targets {
        let changed = match target {
            LifecycleTarget::Memory(id) => memories.insert(id),
            LifecycleTarget::Entity(id) => entities.insert(id),
            LifecycleTarget::Relationship(id) => relationships.insert(id),
            LifecycleTarget::CompatibilityProfile(id) => compatibility_profiles.insert(id),
            _ => return Err("target has no PHY retention contract"),
        };
        if !changed {
            return Err("duplicate prune target");
        }
    }
    if memories.is_empty()
        && entities.is_empty()
        && relationships.is_empty()
        && compatibility_profiles.is_empty()
    {
        return Err("pruning requires explicitly selected terminal targets");
    }
    Ok(PhylacteryPruneTargets {
        memories,
        entities,
        relationships,
        compatibility_profiles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rel_and_phy_keep_their_distinct_target_selection_policy() {
        let conversation = LifecycleTarget::Conversation("conversation".into());
        let selected = select_reliquary_targets(vec![conversation.clone()]).unwrap();
        assert!(selected.conversations.contains("conversation"));
        assert_eq!(
            select_phylactery_targets(vec![conversation]).unwrap_err(),
            "target has no PHY retention contract"
        );

        let entity = LifecycleTarget::Entity(EntityId([1; 32]));
        let selected = select_reliquary_targets(vec![entity.clone(), entity.clone()]).unwrap();
        assert_eq!(selected.descriptions, vec![entity.clone(), entity.clone()]);
        assert_eq!(
            select_phylactery_targets(vec![entity.clone(), entity]).unwrap_err(),
            "duplicate prune target"
        );
    }
}
