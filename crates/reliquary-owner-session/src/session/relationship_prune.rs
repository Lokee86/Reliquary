//! Verified REL/PHY pruning of terminal Relationship descriptions.
use super::*;
use crate::records::{
    HistoryPruneManifest, HistoryRetentionDescriptor, PrunedPayload, RetentionDomain,
    RetentionTarget,
};
use crate::{VerifiedOwnerCopyPlan, staged_copy::StagedCopy};
use reliquary_perception_store::RelationshipId;
use reliquary_vector_store::generation::{generation_receipt_target, relocate_generation_receipt};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
};

mod proof;
pub(super) use proof::validate_relationship_prune;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationshipPruneReport {
    pub owner_id: [u8; 16],
    pub pruned_relationships: usize,
    pub pruned_descriptions: usize,
    pub source_bytes: u64,
    pub transformed_bytes: u64,
    pub manifest_digest: [u8; 32],
}
pub fn prune_terminal_relationship_descriptions_copy(
    plan: &VerifiedOwnerCopyPlan,
    source_path: &Path,
    destination: &Path,
    selected_ids: &[RelationshipId],
    admission: &GraphAdmission,
) -> Result<RelationshipPruneReport, String> {
    if selected_ids.is_empty() || selected_ids.len() > 128 {
        return Err("Relationship pruning requires 1..=128 explicit terminal identities".into());
    }
    if destination.exists() {
        return Err("maintenance destination already exists".into());
    }
    let selected = selected_ids.iter().copied().collect::<BTreeSet<_>>();
    if selected.len() != selected_ids.len() {
        return Err("duplicate Relationship prune target".into());
    }
    let source =
        Container::open_read_only_as(source_path, plan.file_kind()).map_err(|e| e.to_string())?;
    plan.verify_source(&source, source_path)?;
    if !OwnerMaintenanceStore::open(&source)?.is_empty() {
        return Err("repeated destructive pruning requires a new lineage contract".into());
    }
    let state = super::entity_prune::entity_state(&source, admission)?;
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
    let digest = manifest.digest()?;
    let source_bytes = fs::metadata(source_path).map_err(|e| e.to_string())?.len();
    let staged = StagedCopy::create(destination)?;
    let mut dest = Container::create(&staged.path, source.identity(), source.owner_id())
        .map_err(|e| e.to_string())?;
    let mut seen = BTreeSet::new();
    let mut generations = BTreeMap::new();
    let mut cursor = source.physical_cursor().map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    while let Some(mut frame) = cursor.next_frame().map_err(|e| e.to_string())? {
        raw.clear();
        frame.read_to_end(&mut raw).map_err(|e| e.to_string())?;
        let mut handled = false;
        for id in &selected {
            if let Some((redacted, proof_bytes, marker)) =
                reliquary_perception_store::redact_relationship_description_frame(&raw, *id, digest)
                    .map_err(|e| e.to_string())?
            {
                if !seen.insert((marker.id, marker.revision)) {
                    return Err("duplicate Relationship revision payload".into());
                }
                dest.append(&redacted).map_err(|e| e.to_string())?;
                dest.append(&proof_bytes).map_err(|e| e.to_string())?;
                handled = true;
                break;
            }
        }
        if !handled {
            let payload =
                if let Some(old) = generation_receipt_target(&raw).map_err(|e| e.to_string())? {
                    let offset = *generations
                        .get(&old.offset)
                        .ok_or("Relationship prune lost vector generation")?;
                    relocate_generation_receipt(&raw, offset)
                        .map_err(|e| e.to_string())?
                        .to_vec()
                } else {
                    raw.clone()
                };
            let at = dest.append(&payload).map_err(|e| e.to_string())?;
            if raw.starts_with(b"RELVGEN1") {
                generations.insert(frame.object().offset, at);
            }
        }
    }
    let expected = manifest
        .descriptors
        .iter()
        .map(|d| d.payloads.len())
        .sum::<usize>();
    if seen.len() != expected {
        return Err("Relationship revision is not independently removable".into());
    }
    dest.append(&manifest.encode()?)
        .map_err(|e| e.to_string())?;
    dest.sync().map_err(|e| e.to_string())?;
    drop(dest);
    plan.verify_source(&source, source_path)?;
    let copy =
        Container::open_read_only_as(&staged.path, plan.file_kind()).map_err(|e| e.to_string())?;
    let after = super::entity_prune::entity_state(&copy, admission)?;
    if copy.owner_id() != source.owner_id()
        || copy.identity() != source.identity()
        || copy.latest_version() != source.latest_version()
        || after.perception.version() != state.perception.version()
        || after.perception.relationship_version() != state.perception.relationship_version()
        || after.graph.history().copied().collect::<Vec<_>>()
            != state.graph.history().copied().collect::<Vec<_>>()
        || after.perception.entities().cloned().collect::<Vec<_>>()
            != state.perception.entities().cloned().collect::<Vec<_>>()
        || after.perception.mentions().cloned().collect::<Vec<_>>()
            != state.perception.mentions().cloned().collect::<Vec<_>>()
    {
        return Err("Relationship prune changed retained owner state or clocks".into());
    }
    for (id, revision) in &seen {
        if after
            .perception
            .relationship_revision(*id, *revision)
            .is_none_or(|r| !r.summary.is_empty())
            || after
                .perception
                .relationship_description_prune(*id, *revision)
                .is_none_or(|p| p.manifest_digest != digest)
        {
            return Err("Relationship prune does not reopen with exact revision proof".into());
        }
    }
    for record in state.perception.relationships() {
        if after.perception.relationship(record.id) != Some(record) {
            return Err("Relationship prune changed active Relationship".into());
        }
    }
    drop(after);
    drop(copy);
    plan.verify_source(&source, source_path)?;
    fs::hard_link(&staged.path, destination).map_err(|e| e.to_string())?;
    Ok(RelationshipPruneReport {
        owner_id: source.owner_id(),
        pruned_relationships: selected.len(),
        pruned_descriptions: seen.len(),
        source_bytes,
        transformed_bytes: fs::metadata(destination).map_err(|e| e.to_string())?.len(),
        manifest_digest: digest,
    })
}
fn hash_text(hash: &mut Sha256, value: &str) {
    hash.update((value.len() as u64).to_le_bytes());
    hash.update(value.as_bytes());
}
#[cfg(test)]
#[path = "relationship_prune/tests.rs"]
mod tests;
#[cfg(test)]
#[path = "relationship_prune/tests_negative.rs"]
mod tests_negative;
