use super::*;

pub(crate) fn validate_common_profile_prune(
    profiles: &CompatibilityProfileStore,
    packed: &PackedVectorStore,
    memory: &MemoryVectorStore,
    maintenance: &OwnerMaintenanceStore,
) -> Result<(), OwnerSessionError> {
    validate_markers(
        profiles,
        packed,
        maintenance,
        memory
            .infos()
            .into_iter()
            .map(|i| (i.compatibility_profile_id, i.packed_vector_id)),
    )
}

pub(crate) fn validate_archive_profile_prune(
    profiles: &CompatibilityProfileStore,
    packed: &PackedVectorStore,
    memory: &MemoryVectorStore,
    archives: &ArchiveVectorStore,
    generations: &VectorGenerationStore,
    maintenance: &OwnerMaintenanceStore,
) -> Result<(), OwnerSessionError> {
    let by_archive = archives
        .infos()
        .into_iter()
        .map(|i| (i.id, i.packed_vector_id))
        .collect::<BTreeMap<_, _>>();
    let referenced_archive_ids = generations
        .generations()
        .iter()
        .map(|g| g.archive_vector_id)
        .collect::<BTreeSet<_>>();
    let unowned = by_archive
        .iter()
        .filter_map(|(id, packed_id)| (!referenced_archive_ids.contains(id)).then_some(*packed_id))
        .collect::<BTreeSet<_>>();
    if packed.pruned_ids().iter().any(|id| unowned.contains(id)) {
        return Err(OwnerSessionError::Record(
            "pruned packed vector has an unowned Archive mapping".into(),
        ));
    }
    validate_markers(
        profiles,
        packed,
        maintenance,
        memory
            .infos()
            .into_iter()
            .map(|i| (i.compatibility_profile_id, i.packed_vector_id))
            .chain(generations.generations().iter().filter_map(|g| {
                by_archive
                    .get(&g.archive_vector_id)
                    .map(|id| (g.compatibility_profile_id, *id))
            })),
    )
}

fn validate_markers(
    profiles: &CompatibilityProfileStore,
    packed: &PackedVectorStore,
    maintenance: &OwnerMaintenanceStore,
    references: impl IntoIterator<Item = (CompatibilityProfileId, PackedVectorId)>,
) -> Result<(), OwnerSessionError> {
    let owners =
        references
            .into_iter()
            .fold(BTreeMap::<_, BTreeSet<_>>::new(), |mut m, (p, id)| {
                m.entry(id).or_default().insert(p);
                m
            });
    for id in packed.pruned_ids() {
        let digest = packed.prune_manifest_digest(id).ok_or_else(|| {
            OwnerSessionError::Record("pruned packed payload lacks manifest".into())
        })?;
        let manifest = maintenance.get(&digest).ok_or_else(|| {
            OwnerSessionError::Record("pruned packed payload lacks retention evidence".into())
        })?;
        let descriptors = manifest
            .descriptors
            .iter()
            .filter(|d| {
                d.target.domain == RetentionDomain::CompatibilityProfile
                    && d.payloads
                        .binary_search(&PrunedPayload::PackedVectors(id.0))
                        .is_ok()
            })
            .collect::<Vec<_>>();
        if descriptors.len() != 1 || descriptors[0].target.identity.len() != 32 {
            return Err(OwnerSessionError::Record(
                "ambiguous packed-vector retention evidence".into(),
            ));
        }
        let profile = CompatibilityProfileId(
            descriptors[0]
                .target
                .identity
                .as_slice()
                .try_into()
                .unwrap(),
        );
        if !profiles.is_retired(profile)
            || owners
                .get(&id)
                .is_none_or(|set| set != &BTreeSet::from([profile]))
            || !packed.marker_matches(id, digest)
        {
            return Err(OwnerSessionError::Record(
                "invalid retired-profile packed-vector proof".into(),
            ));
        }
    }
    Ok(())
}
