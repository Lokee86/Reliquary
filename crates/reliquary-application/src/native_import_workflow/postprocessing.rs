//! Checked import stages over normal Application preparation and publication.
use super::{dream::dream_pass, settle_native_import};
use crate::{
    EntityMemoryMention, EntityOwnerSession, EntityResolutionProposal, EntityWorkflowError,
    PreparedEntityMention, prepare_memory_mention,
};
use reliquary_owner_session::{
    CreateRelationshipRequest, OwnerSessionError, RelWriter,
    model::{
        CompatibilityProfileId, MemoryEntityMentionKey, MemoryEntityResolution, MemoryId,
        Relationship, RelationshipId,
    },
    records::{ImportComplete, ImportEpochStatus, ImportFreshnessSettlement},
};

/// Additional required work from the caller's frozen processing plan.
/// Every import also requires completed extraction and first-cycle Dream.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NativeImportPostprocessingPlan {
    pub entity_mentions: Vec<MemoryEntityMentionKey>,
    pub relationships: Vec<(RelationshipId, u64)>,
    pub memory_vector_profiles: Vec<CompatibilityProfileId>,
}

#[derive(Clone, Debug)]
pub struct NativeImportPostprocessingOutcome {
    pub settlement: ImportFreshnessSettlement,
    pub completion: ImportComplete,
    pub did_complete: bool,
}

pub fn finish_native_import_postprocessing(
    writer: &mut RelWriter,
    import_id: &str,
    plan: &NativeImportPostprocessingPlan,
    completed_at_ns: i64,
) -> Result<NativeImportPostprocessingOutcome, OwnerSessionError> {
    let status = writer.read_session().import_status(import_id);
    if let Some(ImportEpochStatus::Open(start)) = status {
        if completed_at_ns < start.started_at_ns {
            return Err(incomplete("completion time precedes import start"));
        }
        let cohort: std::collections::BTreeSet<_> = writer
            .native_import_memory_ids(import_id)?
            .into_iter()
            .collect();
        for &id in &cohort {
            let (pass, _) = dream_pass(import_id, id);
            if !writer.initial_dream_settled(&pass) {
                return Err(incomplete(
                    "an imported Memory has no settled first-cycle Dream",
                ));
            }
        }

        let read = writer.read_session();
        for &key in &plan.entity_mentions {
            if !cohort.contains(&MemoryId(key.memory_id)) || read.mention(key).is_none() {
                return Err(incomplete(
                    "a required import Entity mention has no accepted resolution",
                ));
            }
        }
        let owner = read
            .owner_id()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        for &(id, revision) in &plan.relationships {
            if read.relationship_tombstone(id).is_some()
                || !read
                    .relationship_revision(id, revision)
                    .is_some_and(|relationship| {
                        relationship.evidence.iter().any(|evidence| {
                            evidence.owner_id == owner
                                && cohort.contains(&MemoryId(evidence.memory_id))
                        })
                    })
            {
                return Err(incomplete(
                    "a required import relationship has no accepted revision",
                ));
            }
        }
        for &profile in &plan.memory_vector_profiles {
            if !writer
                .native_import_missing_memory_vectors(import_id, profile)?
                .is_empty()
            {
                return Err(incomplete(
                    "a required import profile has missing Memory vectors",
                ));
            }
        }
        settle_native_import(writer, import_id)?;
    } else if !matches!(status, Some(ImportEpochStatus::Completed { .. })) {
        return Err(incomplete("unknown native import epoch"));
    }

    if !writer
        .read_session()
        .has_native_import_historical_freshness(import_id)
    {
        return Err(incomplete(
            "import has no durable historical Freshness snapshot",
        ));
    }
    let settlement = writer
        .read_session()
        .native_import_settlement(import_id)
        .ok_or_else(|| incomplete("import has no durable historical Freshness settlement"))?
        .clone();
    let (completion, did_complete) = writer.complete_native_import(import_id, completed_at_ns)?;
    Ok(NativeImportPostprocessingOutcome {
        settlement,
        completion,
        did_complete,
    })
}

pub fn prepare_native_import_entity_mention(
    writer: &RelWriter,
    import_id: &str,
    input: EntityMemoryMention,
) -> Result<PreparedEntityMention, EntityWorkflowError> {
    writer.validate_native_import_memory(import_id, input.memory_id)?;
    prepare_memory_mention(EntityOwnerSession::Rel(writer.read_session()), input)
}

pub fn commit_native_import_entity_mention(
    writer: &mut RelWriter,
    import_id: &str,
    proposal: EntityResolutionProposal,
    updated_at_ns: i64,
) -> Result<MemoryEntityResolution, EntityWorkflowError> {
    let command = proposal.into_mention_commit(updated_at_ns)?;
    Ok(writer.commit_native_import_entity_mention(import_id, command)?)
}

pub fn publish_native_import_relationship(
    writer: &mut RelWriter,
    import_id: &str,
    request: CreateRelationshipRequest,
) -> Result<Relationship, OwnerSessionError> {
    writer.create_native_import_relationship(import_id, request)
}

fn incomplete(reason: &str) -> OwnerSessionError {
    OwnerSessionError::Record(reason.into())
}
