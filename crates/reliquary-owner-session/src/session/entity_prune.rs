//! Terminal Entity summary pruning for REL and PHY owners.

use super::*;
use crate::records::{
    HistoryPruneManifest, HistoryRetentionDescriptor, PrunedPayload, RetentionDomain,
    RetentionTarget,
};
use crate::{VerifiedOwnerCopyPlan, staged_copy::StagedCopy};
use reliquary_perception_store::MemoryEntityResolutionStatus;
use reliquary_vector_store::generation::{generation_receipt_target, relocate_generation_receipt};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
};

mod proof;
pub(super) use proof::validate_entity_prune;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityPruneReport {
    pub owner_id: [u8; 16],
    pub pruned_entities: usize,
    pub pruned_descriptions: usize,
    pub source_bytes: u64,
    pub transformed_bytes: u64,
    pub manifest_digest: [u8; 32],
}

pub fn prune_terminal_entity_descriptions_copy(
    plan: &VerifiedOwnerCopyPlan,
    source_path: &Path,
    destination: &Path,
    selected_ids: &[EntityId],
    admission: &GraphAdmission,
) -> Result<EntityPruneReport, String> {
    if selected_ids.is_empty() || selected_ids.len() > 128 {
        return Err("Entity pruning requires 1..=128 explicit terminal identities".into());
    }
    if destination.exists() {
        return Err("maintenance destination already exists".into());
    }
    let selected = selected_ids.iter().copied().collect::<BTreeSet<_>>();
    if selected.len() != selected_ids.len() {
        return Err("duplicate Entity prune target".into());
    }
    let source =
        Container::open_read_only_as(source_path, plan.file_kind()).map_err(|e| e.to_string())?;
    plan.verify_source(&source, source_path)?;
    if !OwnerMaintenanceStore::open(&source)?.is_empty() {
        return Err("repeated destructive pruning requires a new lineage contract".into());
    }
    let state = entity_state(&source, admission)?;
    for id in &selected {
        if state.perception.entity_tombstone(*id).is_none() {
            return Err("Entity prune target is not terminally tombstoned".into());
        }
    }
    for mention in state.perception.mentions() {
        let active = match &mention.status {
            MemoryEntityResolutionStatus::Resolved { entity_id, .. } => {
                selected.contains(entity_id)
            }
            MemoryEntityResolutionStatus::Ambiguous { candidates } => {
                candidates.iter().any(|id| selected.contains(id))
            }
            MemoryEntityResolutionStatus::Unresolved { .. } => false,
        };
        if active {
            return Err("selected Entity is retained by an active mention".into());
        }
    }
    if state.perception.relationships().any(|relationship| {
        relationship
            .participants
            .iter()
            .any(|participant| selected.contains(&participant.entity.entity_id))
    }) {
        return Err("selected Entity is retained by an active Relationship".into());
    }
    if state.graph.active_relations().any(|relation| {
        [relation.source, relation.target]
            .into_iter()
            .any(|node| matches!(node, NodeRef::Entity(raw) if selected.contains(&EntityId(raw))))
    }) {
        return Err("selected Entity is retained by an active Graph relation".into());
    }
    let mut descriptors = selected
        .iter()
        .map(|id| proof::descriptor(&state.perception, *id, false))
        .collect::<Result<Vec<_>, _>>()?;
    descriptors.sort_by(|a, b| a.target.cmp(&b.target));
    let manifest = HistoryPruneManifest {
        owner_uuid: source.owner_id(),
        source_global_version: source.latest_version(),
        source_history_digest: plan.source_digest(),
        descriptors,
    };
    let manifest_digest = manifest.digest()?;
    let manifest_bytes = manifest.encode()?;
    let source_bytes = fs::metadata(source_path).map_err(|e| e.to_string())?.len();
    let staged = StagedCopy::create(destination)?;
    let mut target = Container::create(&staged.path, source.identity(), source.owner_id())
        .map_err(|e| e.to_string())?;
    let mut replaced = BTreeSet::new();
    let mut relocated_generations = BTreeMap::new();
    let mut cursor = source.physical_cursor().map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    while let Some(mut frame) = cursor.next_frame().map_err(|e| e.to_string())? {
        bytes.clear();
        frame.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        let mut handled = false;
        for id in &selected {
            if let Some((redacted, marker, proof)) =
                reliquary_perception_store::redact_entity_description_frame(
                    &bytes,
                    *id,
                    manifest_digest,
                )
                .map_err(|e| e.to_string())?
            {
                if !replaced.insert((proof.id, proof.revision)) {
                    return Err("duplicate Entity revision payload".into());
                }
                target.append(&redacted).map_err(|e| e.to_string())?;
                target.append(&marker).map_err(|e| e.to_string())?;
                handled = true;
                break;
            }
        }
        if !handled {
            let payload =
                if let Some(old) = generation_receipt_target(&bytes).map_err(|e| e.to_string())? {
                    let at = *relocated_generations
                        .get(&old.offset)
                        .ok_or("Entity prune lost vector generation")?;
                    relocate_generation_receipt(&bytes, at)
                        .map_err(|e| e.to_string())?
                        .to_vec()
                } else {
                    bytes.clone()
                };
            let written = target.append(&payload).map_err(|e| e.to_string())?;
            if bytes.starts_with(b"RELVGEN1") {
                relocated_generations.insert(frame.object().offset, written);
            }
        }
    }
    let expected = manifest
        .descriptors
        .iter()
        .map(|d| d.payloads.len())
        .sum::<usize>();
    if replaced.len() != expected {
        return Err("selected Entity description is not independently removable".into());
    }
    target.append(&manifest_bytes).map_err(|e| e.to_string())?;
    target.sync().map_err(|e| e.to_string())?;
    drop(target);
    plan.verify_source(&source, source_path)?;
    let copy =
        Container::open_read_only_as(&staged.path, plan.file_kind()).map_err(|e| e.to_string())?;
    let reopened = entity_state(&copy, admission)?;
    if copy.identity() != source.identity()
        || copy.owner_id() != source.owner_id()
        || copy.latest_version() != source.latest_version()
        || reopened.perception.version() != state.perception.version()
    {
        return Err("Entity prune changed owner identity or clocks".into());
    }
    for (id, revision) in &replaced {
        if reopened
            .perception
            .entity_revision(*id, *revision)
            .is_none_or(|entity| !entity.summary.is_empty())
            || reopened
                .perception
                .description_prune(*id, *revision)
                .is_none()
        {
            return Err("Entity description prune did not reopen with proof".into());
        }
    }
    drop(copy);
    plan.verify_source(&source, source_path)?;
    fs::hard_link(&staged.path, destination).map_err(|e| e.to_string())?;
    Ok(EntityPruneReport {
        owner_id: source.owner_id(),
        pruned_entities: selected.len(),
        pruned_descriptions: replaced.len(),
        source_bytes,
        transformed_bytes: fs::metadata(destination).map_err(|e| e.to_string())?.len(),
        manifest_digest,
    })
}

pub(super) struct EntityState {
    pub(super) perception: PerceptionStore,
    pub(super) graph: GraphStore,
}
pub(super) fn entity_state(
    container: &Container,
    admission: &GraphAdmission,
) -> Result<EntityState, String> {
    match container.identity().file_kind {
        FileKind::Reliquary => {
            let session =
                RelSession::open(container, admission, [], []).map_err(|e| e.to_string())?;
            Ok(EntityState {
                perception: session.common.perception,
                graph: session.common.graph,
            })
        }
        FileKind::Phylactery => {
            let session = PhySession::open(container, admission, []).map_err(|e| e.to_string())?;
            Ok(EntityState {
                perception: session.common.perception,
                graph: session.common.graph,
            })
        }
    }
}

fn hash_text(hash: &mut Sha256, value: &str) {
    hash.update((value.len() as u64).to_le_bytes());
    hash.update(value.as_bytes());
}

#[cfg(test)]
#[path = "entity_prune/tests.rs"]
mod tests;
#[cfg(test)]
#[path = "entity_prune/tests_negative.rs"]
mod tests_negative;
