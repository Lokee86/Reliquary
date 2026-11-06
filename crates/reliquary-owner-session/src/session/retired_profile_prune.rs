//! Verified physical retirement of packed vectors owned only by retired profiles.

use super::*;
use crate::records::{
    HistoryPruneManifest, HistoryRetentionDescriptor, PrunedPayload, RetentionDomain,
    RetentionTarget,
};
use crate::{VerifiedOwnerCopyPlan, staged_copy::StagedCopy};
use reliquary_vector_store::{
    PackedVectorId,
    generation::{generation_receipt_target, relocate_generation_receipt},
    packed_id_in_frame,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetiredProfilePruneReport {
    pub owner_id: [u8; 16],
    pub retired_profiles: Vec<CompatibilityProfileId>,
    pub pruned_packed_vectors: usize,
    pub source_bytes: u64,
    pub transformed_bytes: u64,
    pub manifest_digest: [u8; 32],
}

/// Replace packed payloads referenced exclusively by explicitly selected,
/// already-retired profiles with authenticated unavailable markers.
pub fn prune_retired_profile_vectors_copy(
    plan: &VerifiedOwnerCopyPlan,
    source_path: &Path,
    destination: &Path,
    selected: &[CompatibilityProfileId],
    admission: &GraphAdmission,
) -> Result<RetiredProfilePruneReport, String> {
    if selected.is_empty() || selected.len() > 128 {
        return Err("profile pruning requires 1..=128 explicit retired profiles".into());
    }
    if destination.exists() {
        return Err("maintenance destination already exists".into());
    }
    let selected_count = selected.len();
    let selected = selected.iter().copied().collect::<BTreeSet<_>>();
    if selected.len() != selected_count {
        return Err("duplicate selected compatibility profile".into());
    }
    let source = Container::open_read_only(source_path).map_err(|e| e.to_string())?;
    plan.verify_source(&source, source_path)?;
    if !OwnerMaintenanceStore::open(&source)?.is_empty() {
        return Err("repeated destructive pruning requires a new lineage contract".into());
    }
    let state = profile_state(&source, admission)?;
    let selected = selected.into_iter().collect::<Vec<_>>();
    let ownership = packed_profile_ownership(&state);
    let mut by_profile = BTreeMap::<CompatibilityProfileId, Vec<PackedVectorId>>::new();
    for profile in &selected {
        if state.profiles.get(*profile).is_none() || !state.profiles.is_retired(*profile) {
            return Err("selected compatibility profile is absent or active".into());
        }
        let mut ids = ownership
            .iter()
            .filter_map(|(id, owners)| owners.contains(profile).then_some(*id))
            .collect::<Vec<_>>();
        ids.sort_by_key(|id| id.0);
        ids.dedup();
        if ids.is_empty() {
            return Err("selected retired profile has no packed payload".into());
        }
        if ids
            .iter()
            .any(|id| ownership[id].iter().any(|owner| owner != profile))
        {
            return Err("selected packed vector is shared by another profile".into());
        }
        if ids.iter().any(|id| state.unattributed_archive.contains(id)) {
            return Err("selected packed vector has an unowned Archive mapping".into());
        }
        by_profile.insert(*profile, ids);
    }
    let all_ids = by_profile
        .values()
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>();
    if all_ids.iter().any(|id| state.packed.is_pruned(*id)) {
        return Err("selected packed payload is already pruned".into());
    }
    let descriptors = selected
        .iter()
        .map(|profile| {
            let ids = &by_profile[profile];
            let mut structure = Sha256::new();
            structure.update(b"reliquary-retired-profile-structure-v1\0");
            structure.update(profile.0);
            for id in ids {
                let info = state
                    .packed
                    .info(*id)
                    .ok_or("selected packed payload is absent")?;
                structure.update(id.0);
                structure.update(info.schema.dimensions.to_le_bytes());
                structure.update([info.schema.scalar.tag()]);
                structure.update(info.count.to_le_bytes());
                structure.update(info.byte_len.to_le_bytes());
            }
            let mut tombstone = Sha256::new();
            tombstone.update(b"reliquary-profile-retirement-v1\0");
            tombstone.update(profile.0);
            Ok(HistoryRetentionDescriptor {
                target: RetentionTarget {
                    domain: RetentionDomain::CompatibilityProfile,
                    identity: profile.0.to_vec(),
                },
                tombstone_global_version: None,
                tombstone_domain_version: None,
                tombstone_digest: tombstone.finalize().into(),
                retained_structure_digest: structure.finalize().into(),
                payloads: ids
                    .iter()
                    .map(|id| PrunedPayload::PackedVectors(id.0))
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
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
    // Packed-vector markers are shorter than their original matrices. Every
    // subsequent generation record therefore moves, and its physical receipt
    // must point at the new record rather than the source's old offset.
    let mut relocated_generations = BTreeMap::new();
    let mut cursor = source.physical_cursor().map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    while let Some(mut frame) = cursor.next_frame().map_err(|e| e.to_string())? {
        bytes.clear();
        frame.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        let payload = if let Some(id) = packed_id_in_frame(&bytes).map_err(|e| e.to_string())?
            && all_ids.contains(&id)
        {
            if !replaced.insert(id) {
                return Err("duplicate selected packed payload".into());
            }
            state
                .packed
                .marker_bytes(id, manifest_digest)
                .map_err(|e| e.to_string())?
        } else if let Some(old) = generation_receipt_target(&bytes).map_err(|e| e.to_string())? {
            let new = *relocated_generations
                .get(&old.offset)
                .ok_or("generation receipt lost its source record")?;
            relocate_generation_receipt(&bytes, new)
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
    if replaced != all_ids {
        return Err("selected packed payload is not independently removable".into());
    }
    target.append(&manifest_bytes).map_err(|e| e.to_string())?;
    target.sync().map_err(|e| e.to_string())?;
    drop(target);
    plan.verify_source(&source, source_path)?;
    let copy =
        Container::open_read_only_as(&staged.path, plan.file_kind()).map_err(|e| e.to_string())?;
    let copied_state = profile_state(&copy, admission)?;
    if copied_state.packed.infos() != state.packed.infos()
        || copied_state.memory != state.memory
        || copied_state.archive != state.archive
        || copied_state.unattributed_archive != state.unattributed_archive
        || all_ids.iter().any(|id| {
            !copied_state.packed.is_pruned(*id)
                || copied_state.packed.prune_manifest_digest(*id) != Some(manifest_digest)
        })
    {
        return Err("pruned candidate changed vector identities or retention proof".into());
    }
    if copy.identity() != source.identity()
        || copy.owner_id() != source.owner_id()
        || copy.latest_version() != source.latest_version()
    {
        return Err("profile prune changed owner identity or clocks".into());
    }
    drop(copy);
    plan.verify_source(&source, source_path)?;
    fs::hard_link(&staged.path, destination).map_err(|e| e.to_string())?;
    Ok(RetiredProfilePruneReport {
        owner_id: source.owner_id(),
        retired_profiles: selected,
        pruned_packed_vectors: all_ids.len(),
        source_bytes,
        transformed_bytes: fs::metadata(destination).map_err(|e| e.to_string())?.len(),
        manifest_digest,
    })
}

mod ownership;
mod validation;
use ownership::{packed_profile_ownership, profile_state};
pub(super) use validation::{validate_archive_profile_prune, validate_common_profile_prune};
