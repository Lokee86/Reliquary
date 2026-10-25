use crate::owner_lifecycle::semantic_archive_read::SemanticArchiveRead;
use crate::{CompatibilityProfileId, CvaError, VectorGeneration};
pub(crate) struct SemanticArchiveBatch {
    pub(crate) profile: crate::CompatibilityProfile,
    pub(crate) archive_version: u64,
    pub(crate) fragment_ids: Vec<crate::FragmentId>,
    pub(crate) texts: Vec<String>,
}
pub(crate) fn pending_fragments(
    read: &SemanticArchiveRead<'_>,
) -> Result<Vec<(u64, crate::FragmentId)>, CvaError> {
    let owner = read.owner_uuid().ok_or(CvaError::InvalidContainerIdentity(
        "semantic Freshness requires REL UUID",
    ))?;
    let mut floor = read.settlement_cut();
    if let Some(activation) = read.activation() {
        let at = match &activation.boundary {
            None => Some(0),
            Some((c, n)) => read.activity_position(c, n),
        };
        if read.owner_uuid() != Some(activation.owner_uuid) || at != Some(activation.accepted_turn)
        {
            return Err(CvaError::Freshness(
                "semantic activation boundary is unavailable or conflicting".into(),
            ));
        }
        floor = floor.max(activation.accepted_turn);
    }
    let mut pending = Vec::new();
    for fragment in read.fragments() {
        let Some(at) = read.fragment_position(fragment.id)? else {
            continue;
        };
        if at > floor {
            let event_id =
                crate::freshness::historical_semantic::live_semantic_event_id(owner, fragment.id);
            if !read.completed(&event_id).map_err(CvaError::Freshness)? {
                pending.push((at, fragment.id, event_id));
            }
        }
    }
    Ok(crate::freshness::historical_semantic::order_live_pending_events(pending))
}
pub(crate) fn prepare_batch(
    read: &SemanticArchiveRead<'_>,
    profile_id: CompatibilityProfileId,
    current: Option<VectorGeneration>,
) -> Result<Option<SemanticArchiveBatch>, CvaError> {
    let pending = pending_fragments(read)?;
    if pending.is_empty() {
        return Ok(None);
    }
    let archive_version = read.archive_version();
    if let Some(generation) = current {
        let set = read.archive_vectors(generation.archive_vector_id)?;
        if pending.iter().all(|(_, id)| {
            read.fragment_version(*id)
                .is_some_and(|v| v <= generation.source_archive_version)
                && set.fragment_ids.contains(id)
        }) {
            return Ok(None);
        }
    }
    let profile = read
        .profile(profile_id)
        .ok_or_else(|| CvaError::Freshness("missing semantic embedding profile".into()))?;
    let fragment_ids = read
        .fragments()
        .into_iter()
        .map(|f| f.id)
        .collect::<Vec<_>>();
    let mut texts = Vec::with_capacity(fragment_ids.len());
    for id in &fragment_ids {
        texts.push(read.text(*id)?);
    }
    Ok(Some(SemanticArchiveBatch {
        profile,
        archive_version,
        fragment_ids,
        texts,
    }))
}
pub(crate) fn pack_batch(
    batch: SemanticArchiveBatch,
    vectors: Vec<Vec<f32>>,
    current: Option<VectorGeneration>,
) -> Result<Option<crate::memory_vector_store::SemanticArchivePublicationPlan>, CvaError> {
    crate::compatibility_profile_probe::validate_embedding_batch(
        &batch.profile,
        &vectors,
        batch.texts.len(),
    )
    .map_err(|e| CvaError::Freshness(e.to_string()))?;
    if current.is_some_and(|g| g.source_archive_version > batch.archive_version) {
        return Ok(None);
    }
    let schema = crate::VectorSchema::new(batch.profile.dimensions, crate::ScalarType::F32)
        .map_err(|e| CvaError::Freshness(e.to_string()))?;
    let packed = crate::PackedVectors::from_bytes(
        schema,
        vectors
            .into_iter()
            .flatten()
            .flat_map(f32::to_le_bytes)
            .collect(),
    )
    .map_err(|e| CvaError::Freshness(e.to_string()))?;
    Ok(Some(
        crate::memory_vector_store::SemanticArchivePublicationPlan {
            profile_id: batch.profile.id,
            archive_version: batch.archive_version,
            fragment_ids: batch.fragment_ids,
            packed,
        },
    ))
}
