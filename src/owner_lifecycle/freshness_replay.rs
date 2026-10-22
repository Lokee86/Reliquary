//! Rebuild accepted Freshness state from the retained REL physical history.

use crate::freshness_dream_journal::DreamFreshnessJournal;
use crate::freshness_storage::FreshnessStore;
use crate::owner_lifecycle::{
    AcceptedExternalBirthCohort, AcceptedInsomniaBirthCohort, OwnerError,
    validate_external_birth_cohort, validate_insomnia_birth_cohort,
};
use crate::{Archive, Container};

pub(crate) fn rebuild_freshness_from_replay(
    container: &Container,
    archive: &Archive,
    memories: &crate::memory_store::MemoryStore,
    freshness: &mut FreshnessStore,
    dream_freshness: &mut DreamFreshnessJournal,
) -> Result<(), OwnerError> {
    freshness.begin_rebuild();
    if let Some(owner) = container.owner_uuid() {
        for (id, mutation, turn, admitted) in dream_freshness.pending_initializations() {
            if memories.first_mutation_id(id) == Some(mutation.as_str()) {
                let first_global = memories.first_global_version(id).ok_or_else(|| {
                    OwnerError::Freshness("birth lacks accepted first Memory revision".into())
                })?;
                let archive_version = archive
                    .record_versions()
                    .iter()
                    .filter(|record| record.global_version < first_global)
                    .map(|record| record.archive_version)
                    .max()
                    .unwrap_or(0);
                if turn != archive.activity_cut_at_archive_version(archive_version)? {
                    return Err(OwnerError::Freshness(
                        "publication birth differs from accepted first Memory activity cut".into(),
                    ));
                }
                freshness
                    .derive_birth(owner, id, turn, admitted)
                    .map_err(OwnerError::Freshness)?;
            } else if memories.contains_memory(id) {
                return Err(OwnerError::Freshness(
                    "publication intent differs from accepted first Memory mutation".into(),
                ));
            }
        }
        let mut cursor = container.physical_cursor()?;
        while let Some(frame) = cursor.next_frame()? {
            let object = frame.object;
            let payload = frame.payload.as_slice();
            let location = object.legacy_bytes();
            let end = u64::from_le_bytes(location[..8].try_into().unwrap())
                + 8
                + u64::from_le_bytes(location[8..].try_into().unwrap());
            if let Some(completion) = crate::insomnia::completion::decode_completion(payload)
                .map_err(|message| OwnerError::Freshness(message.into()))?
                && let Some(cohort) =
                    validate_insomnia_birth_cohort(archive, memories, &completion)?
            {
                derive_insomnia_births(owner, freshness, dream_freshness, cohort)?;
            }
            if let Some(publication) = crate::memory_external_publication::decode(payload)? {
                if let Some(cohort) =
                    validate_external_birth_cohort(archive, memories, &publication)?
                {
                    derive_external_births(owner, freshness, dream_freshness, cohort)?;
                }
            }
            freshness
                .ingest_scanned(payload, end)
                .map_err(OwnerError::Freshness)?;
        }
    } else {
        let mut cursor = container.physical_cursor()?;
        while let Some(frame) = cursor.next_frame()? {
            let object = frame.object;
            let payload = frame.payload.as_slice();
            let location = object.legacy_bytes();
            let end = u64::from_le_bytes(location[..8].try_into().unwrap())
                + 8
                + u64::from_le_bytes(location[8..].try_into().unwrap());
            freshness
                .ingest_scanned(payload, end)
                .map_err(OwnerError::Freshness)?;
        }
    }
    freshness.finish_rebuild().map_err(OwnerError::Freshness)?;
    if let Some(activation) = freshness.semantic_activation() {
        let at = activation
            .boundary
            .as_ref()
            .map_or(Some(0), |(conversation, node)| {
                archive.activity_position_for_turn(conversation, node)
            });
        if at != Some(activation.accepted_turn) {
            return Err(OwnerError::Freshness(
                "semantic activation boundary is unavailable".into(),
            ));
        }
    }
    Ok(())
}

fn derive_insomnia_births(
    owner: [u8; 16],
    freshness: &mut FreshnessStore,
    dream_freshness: &mut DreamFreshnessJournal,
    cohort: AcceptedInsomniaBirthCohort,
) -> Result<(), OwnerError> {
    for member in cohort.members {
        freshness
            .derive_birth(owner, member.id, cohort.turn, false)
            .map_err(OwnerError::Freshness)?;
        dream_freshness.derive_publication_birth(member.id);
    }
    Ok(())
}

fn derive_external_births(
    owner: [u8; 16],
    freshness: &mut FreshnessStore,
    dream_freshness: &mut DreamFreshnessJournal,
    cohort: AcceptedExternalBirthCohort,
) -> Result<(), OwnerError> {
    for member in cohort.members {
        freshness
            .derive_birth(owner, member.id, cohort.turn, false)
            .map_err(OwnerError::Freshness)?;
        dream_freshness.derive_publication_birth(member.id);
    }
    Ok(())
}
