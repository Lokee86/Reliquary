use super::*;

pub(super) struct ProfileState {
    pub(super) profiles: CompatibilityProfileStore,
    pub(super) packed: PackedVectorStore,
    pub(super) memory: Vec<MemoryVectorInfo>,
    pub(super) archive: Vec<(CompatibilityProfileId, PackedVectorId)>,
    pub(super) unattributed_archive: BTreeSet<PackedVectorId>,
}

pub(super) fn profile_state(
    container: &Container,
    admission: &GraphAdmission,
) -> Result<ProfileState, String> {
    match container.identity().file_kind {
        FileKind::Phylactery => {
            let session = PhySession::open(container, admission, []).map_err(|e| e.to_string())?;
            Ok(ProfileState {
                profiles: session.common.profiles,
                packed: session.common.packed_vectors,
                memory: session.common.memory_vectors.infos(),
                archive: Vec::new(),
                unattributed_archive: BTreeSet::new(),
            })
        }
        FileKind::Reliquary => {
            let session =
                RelSession::open(container, admission, [], []).map_err(|e| e.to_string())?;
            let infos = session
                .archive_vectors
                .infos()
                .into_iter()
                .map(|info| (info.id, info.packed_vector_id))
                .collect::<BTreeMap<_, _>>();
            let generation_archive_ids = session
                .generations
                .generations()
                .iter()
                .map(|g| g.archive_vector_id)
                .collect::<BTreeSet<_>>();
            let archive = session
                .generations
                .generations()
                .iter()
                .map(|g| {
                    infos
                        .get(&g.archive_vector_id)
                        .map(|packed| (g.compatibility_profile_id, *packed))
                        .ok_or("generation has no Archive mapping")
                })
                .collect::<Result<Vec<_>, _>>()?;
            let unattributed_archive = infos
                .iter()
                .filter_map(|(id, packed)| {
                    (!generation_archive_ids.contains(id)).then_some(*packed)
                })
                .collect();
            Ok(ProfileState {
                profiles: session.common.profiles,
                packed: session.common.packed_vectors,
                memory: session.common.memory_vectors.infos(),
                archive,
                unattributed_archive,
            })
        }
    }
}

pub(super) fn packed_profile_ownership(
    state: &ProfileState,
) -> BTreeMap<PackedVectorId, BTreeSet<CompatibilityProfileId>> {
    let mut result = BTreeMap::new();
    for info in &state.memory {
        result
            .entry(info.packed_vector_id)
            .or_insert_with(BTreeSet::new)
            .insert(info.compatibility_profile_id);
    }
    for (profile, packed) in &state.archive {
        result
            .entry(*packed)
            .or_insert_with(BTreeSet::new)
            .insert(*profile);
    }
    result
}
