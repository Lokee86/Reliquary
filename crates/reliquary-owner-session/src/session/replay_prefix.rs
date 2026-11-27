//! Exact owner identity/version cuts at the physical effect prefix.
use super::dream_population::BirthPopulationReplay;
use super::identity_receipt::{IdentityHistory, LocalProjection};
use super::identity_separation::{SeparationHistory, SeparationProjection};
use super::observations::{
    ObservationAuthority, ObservationPrefix, PhyObservationHistory, PhySettlements,
};
use super::*;
use crate::records::dream::InferredExplicitFrontier;
use crate::records::dream::dream_begin_codec::{self, BeginPopulation, BeginRecord};
use crate::records::user_memory::{UserMemoryEffect, UserMemoryRecord};
use reliquary_perception_store::observation::ObservationStore;
use std::io::Read;

#[derive(Default)]
pub(super) struct DreamReplayPrefix {
    pub birth_versions: BTreeMap<MemoryId, u64>,
    pub explicit_admissions: Vec<DreamBeginAdmission>,
    pub native_admissions: Vec<NativeBirthAdmission>,
    pub explicit_frontiers: Vec<InferredExplicitFrontier>,
    pub infer_explicit_populations: bool,
}

pub(super) struct NativeBirthAdmission {
    pub begin: BeginRecord,
    pub first_memory_version: u64,
    pub memory_cut: u64,
}

pub(super) struct DreamArchivePrefix<'a> {
    pub archive: &'a Archive,
    pub cuts: &'a BTreeMap<u64, (u64, u64)>,
}

fn graph_prefix_frame(prefix: &[u8]) -> bool {
    graph_prefix_frame_with_identity(prefix, false)
}

fn graph_and_identity_prefix_frame(prefix: &[u8]) -> bool {
    graph_prefix_frame_with_identity(prefix, true)
}

fn graph_and_phy_prefix_frame(prefix: &[u8]) -> bool {
    if prefix.starts_with(b"RELEGO01") {
        return true;
    }
    let body = match reliquary_container::decode_publication(prefix) {
        Ok(Some(envelope)) => envelope.body,
        Ok(None) => prefix,
        Err(_) => return true,
    };
    [
        b"RELMBOD1".as_slice(),
        b"RELETM01",
        super::observations::PHY_PREFIX,
    ]
    .iter()
    .any(|marker| body.starts_with(marker))
        || graph_prefix_frame(prefix)
}

fn graph_prefix_frame_with_identity(prefix: &[u8], identity: bool) -> bool {
    if crate::records::import::is_import_epoch_marker(prefix) {
        return true;
    }
    let body = match reliquary_container::decode_publication(prefix) {
        Ok(Some(envelope)) => envelope.body,
        Ok(None) => prefix,
        Err(_) => return true,
    };
    let families: &[&[u8]] = &[
        b"RELDR",
        b"RELMCOR",
        b"RELMRET",
        b"CVDREAM2",
        b"RELINSC7",
        b"RELINSC8",
        b"RELCPRO1",
        b"RELPVRN1",
        b"RELMVEC1",
        b"RELGPB",
        b"RELASC",
        b"RELGRF01",
        b"RELCOM01",
        b"RELCOM02",
        b"RELCMP01",
        b"RELCMN01",
        b"RELEPB",
        b"RELPER01",
        b"RELERH01",
        b"RELERL01",
    ];
    crate::records::memory_prune_proof::memory_history_record(body)
        || families.iter().any(|marker| body.starts_with(marker))
        // Packed objects normally arrive through separately proved positions.
        // Preserve the existing scan fence for an incomplete object header.
        || (prefix.len() < 64 && prefix.starts_with(b"RELPVEC1"))
        || (identity
            && [
                super::observations::PREFIX,
                super::observations::PHY_PREFIX,
                super::identity_receipt::PREFIX,
                super::identity_separation::PREFIX,
                b"RELMBOD1",
                b"RELETM01",
                b"RELRLP01",
            ]
            .iter()
            .any(|marker| body.starts_with(marker)))
}

pub(super) fn validate_graph_prefix(
    container: &Container,
    memory_store: &MemoryStore,
    packed_store: &PackedVectorStore,
    identity_context: Option<(&PerceptionStore, [u8; 32])>,
    phy_generation: Option<(&PerceptionStore, [u8; 32])>,
    pruned_bodies: &BTreeSet<MemoryBodyId>,
    observation_archive: Option<(&Archive, &BTreeMap<u64, (u64, u64)>)>,
    compatibility_profiles: &CompatibilityProfileStore,
    dream_archive: Option<DreamArchivePrefix<'_>>,
    dreams: &mut DreamReplayPrefix,
) -> Result<
    (
        IdentityHistory,
        SeparationHistory,
        ObservationStore,
        PhyObservationHistory,
    ),
    OwnerSessionError,
> {
    let mut observations = ObservationStore::default();
    let mut phy_observations = PhyObservationHistory::default();
    let mut settled = BTreeSet::new();
    let mut receipts = IdentityHistory::default();
    let mut separations = SeparationHistory::default();
    let mut perception_cut = 0;
    let mut retained_ordinal_cut = None;
    let mut observed_bodies = BTreeSet::new();
    let mut memories = std::collections::BTreeSet::new();
    let mut memory_bodies = std::collections::BTreeSet::new();
    let mut profiles = std::collections::BTreeSet::new();
    let mut active_profiles = std::collections::BTreeSet::new();
    let mut packed_vectors = std::collections::BTreeSet::new();
    let mut entities = std::collections::BTreeSet::new();
    let mut graph_version = 0;
    let mut memory_head = 0;
    let mut birth_population = BirthPopulationReplay::new(memory_store);
    let mut imports = ImportEpochStore::prefix(container.owner_id());
    let mut native_births = BTreeMap::new();
    let mut native_receipt = None;
    let mut native_episodes = BTreeSet::new();
    // The packed store checked the full matrices. Advance their admitted
    // positions alongside the other frames to preserve the mapping cut.
    let mut packed_positions: Vec<_> = packed_store.object_positions().collect();
    packed_positions.sort_unstable_by_key(|(object, _)| object.offset);
    let mut packed_positions = packed_positions.into_iter().peekable();
    let select: fn(&[u8]) -> bool = if identity_context.is_some() {
        graph_and_identity_prefix_frame
    } else if phy_generation.is_some() {
        graph_and_phy_prefix_frame
    } else {
        graph_prefix_frame
    };
    let mut frames = container.matching_physical_cursor(select)?;
    let mut buffer = Vec::new();
    while let Some(mut reader) = frames.next_frame()? {
        let object = reader.object();
        buffer.clear();
        if identity_context.is_some() || phy_generation.is_some() {
            // A timed envelope can occupy 25 bytes. The standalone Memory
            // body ID follows the eight-byte marker in the next 32 bytes.
            buffer.resize(object.length.min(65) as usize, 0);
            reader
                .read_exact(&mut buffer)
                .map_err(ContainerError::from)?;
            let envelope = reliquary_container::decode_publication(&buffer)?;
            let body = envelope
                .as_ref()
                .map_or(buffer.as_slice(), |entry| entry.body);
            if let Some(id) = reliquary_memory_core::body_id_in_frame(body)? {
                observed_bodies.insert(id);
                continue;
            }
        }
        reader
            .read_to_end(&mut buffer)
            .map_err(ContainerError::from)?;
        let frame = reliquary_container::ScannedFrame {
            object,
            payload: &buffer,
        };
        while let Some((_, id)) =
            packed_positions.next_if(|(object, _)| object.offset < frame.object.offset)
        {
            packed_vectors.insert(id.0);
        }
        if crate::records::import::is_import_epoch_marker(frame.payload) {
            imports
                .ingest_prefix(frame.payload)
                .map_err(OwnerSessionError::Record)?;
            if imports.open_epoch().is_none() {
                native_births.clear();
                native_receipt = None;
                native_episodes.clear();
            }
            continue;
        }
        if let Some((_, generation)) = phy_generation
            && frame.payload.starts_with(b"RELEGO01")
        {
            if let Some(publication) =
                reliquary_ego_store::EgoStore::phy_publication_in_frame(frame.payload)?
            {
                super::ego::validate_phy_publication_at_prefix(
                    &publication,
                    generation,
                    memory_store,
                    memory_head,
                    &observed_bodies,
                )?;
            }
            continue;
        }
        if let Some((perception, generation)) = phy_generation {
            let envelope = reliquary_container::decode_publication(frame.payload)?;
            let body = envelope.as_ref().map_or(frame.payload, |entry| entry.body);
            if body.starts_with(super::observations::PHY_PREFIX) {
                let cut = PhyObservationCut {
                    audit: PhyAuditCut {
                        owner_id: container.owner_id(),
                        history_generation: generation,
                        memory: memory_head,
                        graph: graph_version,
                        settlements: settled.len() as u64,
                    },
                    entity: perception_cut,
                    retained_resolution: retained_ordinal_cut,
                    observation: phy_observations.store.frontier(),
                };
                let prefix = ObservationPrefix {
                    owner: cut.audit.owner_id,
                    generation,
                    memories: memory_store,
                    profiles: compatibility_profiles,
                    memory_cut: memory_head,
                    perception,
                    perception_cut,
                    retained_ordinal_cut,
                    observed_bodies: Some(&observed_bodies),
                    unobserved_suffix_bodies: None,
                    source_eligibility: ObservationAuthority::Phy(PhySettlements::Physical(
                        &settled,
                    )),
                };
                phy_observations.admit(frame.payload, prefix, cut, &active_profiles)?;
                continue;
            }
        }
        if let Some((perception, history_generation)) = identity_context {
            let envelope = reliquary_container::decode_publication(frame.payload)?;
            let body = envelope.as_ref().map_or(frame.payload, |entry| entry.body);
            if body.starts_with(super::observations::PHY_PREFIX) {
                return Err(OwnerSessionError::Record(
                    "PHY Observation on a REL owner".into(),
                ));
            }
            if body.starts_with(super::observations::PREFIX) {
                let (archive, archive_cuts) = observation_archive.ok_or_else(|| {
                    OwnerSessionError::Record("Observation Archive replay context absent".into())
                })?;
                let archive_cut = envelope
                    .as_ref()
                    .and_then(|entry| archive_cuts.get(&entry.version))
                    .map_or(0, |&(cut, _)| cut);
                let prefix = ObservationPrefix {
                    owner: container.owner_id(),
                    generation: history_generation,
                    memories: memory_store,
                    profiles: compatibility_profiles,
                    memory_cut: memory_head,
                    perception,
                    perception_cut,
                    retained_ordinal_cut,
                    observed_bodies: Some(&observed_bodies),
                    unobserved_suffix_bodies: None,
                    source_eligibility: super::observations::ObservationAuthority::Rel(
                        super::source_eligibility::SourceEligibility::new(
                            archive,
                            container.owner_id(),
                            archive_cut,
                        ),
                    ),
                };
                prefix.admit(
                    &mut observations,
                    frame.payload,
                    archive_cuts,
                    &active_profiles,
                )?;
                continue;
            }
            if body.starts_with(super::identity_receipt::PREFIX) {
                let projection = LocalProjection {
                    memories: memory_store,
                    memory_cut: memory_head,
                    observed_bodies: Some(&observed_bodies),
                    pruned_bodies,
                    perception,
                    perception_cut,
                    retained_ordinal_cut,
                };
                receipts.replay(frame.payload, container.owner_id(), projection)?;
                continue;
            }
            if body.starts_with(super::identity_separation::PREFIX) {
                let projection = SeparationProjection {
                    owner: container.owner_id(),
                    history_generation,
                    receipts: &receipts,
                    perception,
                    memory_cut: memory_head,
                    perception_cut,
                };
                separations.replay(frame.payload, projection)?;
                continue;
            }
        }
        if identity_context.is_some() || phy_generation.is_some() {
            if let Some(ordinal) =
                reliquary_perception_store::retained_resolution_ordinal_in_frame(frame.payload)?
            {
                retained_ordinal_cut = Some(ordinal);
            }
            if let Some(version) =
                reliquary_perception_store::perception_version_in_frame(frame.payload)?
            {
                perception_cut = version;
            }
        }
        if let Some((mut frontier, begin)) =
            crate::records::dream_frontier::FrontierRecord::decode_with_begin(frame.payload)
                .map_err(OwnerSessionError::Record)?
        {
            validate_frontier_cut(
                container.owner_id(),
                memory_store,
                memory_head,
                graph_version,
                &frontier,
            )?;
            if let Some(begin) = begin {
                let birth_version = *dreams
                    .birth_versions
                    .get(&begin.source_id)
                    .ok_or(OwnerSessionError::StaleDreamCut)?;
                match begin.population {
                    BeginPopulation::ExplicitSet(ids) => {
                        if &ids != birth_population.at(birth_version)? {
                            return Err(OwnerSessionError::StaleDreamCut);
                        }
                        if dreams.infer_explicit_populations {
                            frontier.begin = None;
                            dreams.explicit_frontiers.push(InferredExplicitFrontier {
                                position: frame.object,
                                frontier,
                                turn: begin.turn,
                                first_memory_version: birth_version,
                            });
                        } else {
                            dreams.explicit_admissions.push(DreamBeginAdmission {
                                owner_uuid: container.owner_id(),
                                pass_id: begin.pass_id,
                                source_id: begin.source_id,
                                turn: begin.turn,
                                graph_version_before: begin.graph_version_before,
                                preexisting_ids: ids,
                                memory_cut: memory_head,
                            });
                        }
                    }
                    BeginPopulation::NativeBirth {
                        first_memory_version,
                        ..
                    } => {
                        let epoch = imports
                            .open_epoch()
                            .filter(|epoch| {
                                epoch.format_version == 2
                                    && imports.settlement(&epoch.import_id).is_none()
                            })
                            .ok_or(OwnerSessionError::StaleDreamCut)?;
                        let (first, episode_id) = native_births
                            .get(&begin.source_id)
                            .copied()
                            .ok_or(OwnerSessionError::StaleDreamCut)?;
                        let archive = dream_archive
                            .as_ref()
                            .ok_or(OwnerSessionError::StaleDreamCut)?;
                        let &(archive_cut, turn) = archive
                            .cuts
                            .get(&frame.object.offset)
                            .ok_or(OwnerSessionError::StaleDreamCut)?;
                        if first != first_memory_version
                            || birth_version != first_memory_version
                            || first_memory_version <= epoch.pre.memory_version
                            || begin.pass_id
                                != RelWriter::native_import_dream_identity(
                                    &epoch.import_id,
                                    begin.source_id,
                                )
                                .0
                            || turn != begin.turn
                        {
                            return Err(OwnerSessionError::StaleDreamCut);
                        }
                        if native_receipt.is_none() {
                            let key = super::native_import::decode_import_key(&epoch.import_key)
                                .ok_or(OwnerSessionError::StaleDreamCut)?;
                            let version = archive
                                .archive
                                .import_receipt_archive_version(&key)
                                .ok_or(OwnerSessionError::StaleDreamCut)?;
                            let receipt = archive
                                .archive
                                .import_receipt(&key)?
                                .ok_or(OwnerSessionError::StaleDreamCut)?;
                            native_receipt = Some((
                                version,
                                super::native_import::ImportSourceMap::from_receipt(
                                    receipt.as_ref(),
                                ),
                            ));
                        }
                        let (receipt_version, mapped) =
                            native_receipt.as_ref().expect("native receipt loaded");
                        if *receipt_version > archive_cut {
                            return Err(OwnerSessionError::StaleDreamCut);
                        }
                        if native_episodes.insert(episode_id)
                            && !super::native_import::native_dream_episode_is_mapped(
                                archive.archive,
                                episode_id,
                                |conversation, node| mapped.contains(conversation, node),
                            )?
                        {
                            return Err(OwnerSessionError::StaleDreamCut);
                        }
                        dreams.native_admissions.push(NativeBirthAdmission {
                            begin,
                            first_memory_version,
                            memory_cut: memory_head,
                        });
                    }
                    _ => return Err(OwnerSessionError::StaleDreamCut),
                }
            }
        } else if frame.payload.starts_with(b"CVDREAM2") {
            let begin =
                dream_begin_codec::decode(frame.payload).map_err(OwnerSessionError::Record)?;
            match begin.population {
                BeginPopulation::ExplicitSet(ids) => {
                    dreams.explicit_admissions.push(DreamBeginAdmission {
                        owner_uuid: container.owner_id(),
                        pass_id: begin.pass_id,
                        source_id: begin.source_id,
                        turn: begin.turn,
                        graph_version_before: begin.graph_version_before,
                        preexisting_ids: ids,
                        memory_cut: memory_store
                            .first_memory_version(begin.source_id)
                            .unwrap_or_default(),
                    });
                }
                BeginPopulation::CompactCut { .. } => (),
                BeginPopulation::NativeBirth { .. } => {
                    return Err(OwnerSessionError::StaleDreamCut);
                }
            }
        }
        if frame.payload.starts_with(b"RELDR")
            && ![
                b"RELDRM01",
                b"RELDRP02",
                b"RELDRG01",
                b"RELDRS01",
                b"RELDRF01",
                b"RELDRF02",
                b"RELDRB01",
                b"RELDRC01",
            ]
            .iter()
            .any(|magic| frame.payload.starts_with(magic.as_slice()))
        {
            return Err(OwnerSessionError::Record(
                "unsupported Dream carrier".into(),
            ));
        }
        if reliquary_container::decode_publication(frame.payload)?.is_some_and(|p| {
            p.body.starts_with(b"RELDR")
                || p.body.starts_with(b"RELMCOR")
                || p.body.starts_with(b"RELMRET")
        }) {
            return Err(OwnerSessionError::Record(
                "wrapped grouped owner carrier is unsupported".into(),
            ));
        }
        let user_memory =
            UserMemoryRecord::decode(frame.payload).map_err(OwnerSessionError::Record)?;
        if let Some(record) = &user_memory {
            validate_user_memory_prefix(
                container,
                memory_store,
                memory_head,
                graph_version,
                record,
            )?;
        }
        let batch = crate::records::dream_batch::DreamBatchRecord::decode(frame.payload)
            .map_err(OwnerSessionError::Record)?;
        if let Some(record) = &batch {
            validate_batch_cut(
                container.owner_id(),
                memory_store,
                memory_head,
                graph_version,
                record,
            )?;
        }
        let children: Vec<&[u8]> = match &batch {
            Some(record) => record
                .lifecycles
                .iter()
                .map(|l| l.delta.as_slice())
                .chain(record.graph_records.iter().map(Vec::as_slice))
                .collect(),
            None => user_memory
                .as_ref()
                .map_or_else(|| vec![frame.payload], UserMemoryRecord::history_frames),
        };
        for child in children {
            let payload = reliquary_container::decode_publication(child)?
                .filter(|envelope| {
                    crate::records::memory_prune_proof::memory_history_record(envelope.body)
                })
                .map_or(child, |envelope| envelope.body);
            if payload.starts_with(b"RELMCRT1") {
                let record = reliquary_memory_core::decode_grouped_creation(payload)?;
                memory_head = record.memory_version;
                memories.insert(record.id.0);
                memory_bodies.insert(record.body_id.0);
                if record.revision == 1 {
                    dreams
                        .birth_versions
                        .insert(record.id, record.memory_version);
                }
            } else if crate::records::insomnia_completion::is_grouped_completion(payload) {
                let completion = GroupedInsomniaCompletion::decode(payload)
                    .map_err(OwnerSessionError::Record)?;
                if identity_context.is_some() {
                    observed_bodies.extend(completion.bodies.iter().map(|body| body.id));
                }
                // Memory replay validated the creations in contiguous version
                // order. Every sibling shares the first version of this frame.
                let mut cohort_birth = None;
                let native = imports.open_epoch().is_some_and(|epoch| {
                    epoch.format_version == 2 && imports.settlement(&epoch.import_id).is_none()
                });
                for record in completion.memory_records {
                    let record = reliquary_memory_core::decode_grouped_creation(&record)?;
                    memory_head = record.memory_version;
                    memories.insert(record.id.0);
                    memory_bodies.insert(record.body_id.0);
                    let first = *cohort_birth.get_or_insert(record.memory_version);
                    dreams.birth_versions.insert(record.id, first);
                    if native
                        && record.revision == 1
                        && record.source_episode_id == Some(completion.episode_id)
                    {
                        native_births.insert(
                            record.id,
                            (first, reliquary_archive::EpisodeId(completion.episode_id)),
                        );
                    }
                }
            }
            if let Some((_, version)) = reliquary_memory_core::history_change_in_frame(payload)? {
                memory_head = version;
            }
            if let Some(record) =
                crate::records::dream_empty::InitialDreamSettlement::decode(payload)
                    .map_err(OwnerSessionError::Record)?
            {
                if phy_generation.is_some() {
                    settled.insert(record.source);
                }
                if let Some(delta) = record.memory_delta {
                    memory_head = reliquary_memory_core::history_change_in_frame(&delta)?
                        .ok_or(OwnerSessionError::StaleDreamCut)?
                        .1;
                }
            }
            if let Some(profile) = reliquary_vector_store::profile::decode_profile(payload)?
                && profiles.insert(profile.id.0)
            {
                active_profiles.insert(profile.id.0);
            }
            if let Some(retired) = reliquary_vector_store::profile::decode_retirement(payload)? {
                active_profiles.remove(&retired.0);
            }
            if let Some(packed) = reliquary_vector_store::store::packed_id_in_frame(payload)? {
                packed_vectors.insert(packed.0);
            }
            if let Some(mapping) = reliquary_vector_store::memory::sources_in_frame(payload)? {
                if mapping
                    .bodies
                    .iter()
                    .any(|id| !memory_bodies.contains(&id.0))
                {
                    return Err(OwnerSessionError::Record(
                        "Memory vector references a body outside its physical prefix".into(),
                    ));
                }
                if !profiles.contains(&mapping.profile.0)
                    || !packed_vectors.contains(&mapping.packed.0)
                {
                    return Err(OwnerSessionError::Record(
                        "Memory vector references backing outside its physical prefix".into(),
                    ));
                }
            }
            match reliquary_perception_store::source_publication(payload)? {
                Some(reliquary_perception_store::SourcePublication::EntityCreation(id)) => {
                    entities.insert(id.0);
                }
                Some(reliquary_perception_store::SourcePublication::Mention(key))
                    if !memories.contains(&key.memory_id) =>
                {
                    return Err(OwnerSessionError::Record(
                        "Entity mention references a Memory outside its physical prefix".into(),
                    ));
                }
                _ => {}
            }
            if let Some((owner, effect)) = reliquary_graph::user_association_in_frame(payload)? {
                if owner != container.owner_id()
                    || effect.graph_version != graph_version
                    || [effect.source, effect.target].iter().any(|endpoint| {
                        !memory_store
                            .record_at_cut(MemoryId(endpoint.id), memory_head)
                            .is_some_and(|r| {
                                r.revision == endpoint.revision
                                    && r.body_id.0 == endpoint.body
                                    && !r.archived
                            })
                    })
                {
                    return Err(OwnerSessionError::Record(
                        "user association lacks owner physical-prefix proof".into(),
                    ));
                }
            }
            let relations = match reliquary_graph::clocked_graph_in_frame(payload)? {
                Some(publication) => publication.relations,
                None => reliquary_graph::relation_in_frame(payload)?
                    .into_iter()
                    .collect(),
            };
            if !relations.is_empty() {
                for endpoint in relations
                    .iter()
                    .flat_map(|relation| [relation.source, relation.target])
                {
                    let found = match endpoint {
                        NodeRef::Memory(id) => memories.contains(&id),
                        NodeRef::Entity(id) => entities.contains(&id),
                    };
                    if !found {
                        return Err(OwnerSessionError::Record(
                            "Graph endpoint is absent from its physical prefix".into(),
                        ));
                    }
                }
                graph_version = relations[0].graph_version;
            } else if let Some(group) =
                crate::records::dream_group::GroupedDreamRelation::decode(payload)
                    .map_err(OwnerSessionError::Record)?
            {
                if group.0.owner_id != container.owner_id()
                    || group.0.graph_version != graph_version
                    || [group.0.source, group.0.candidate].iter().any(|observed| {
                        !memory_store
                            .record_at_cut(observed.id, memory_head)
                            .is_some_and(|r| {
                                r.revision == observed.revision
                                    && r.body_id == observed.body_id
                                    && !r.archived
                            })
                    })
                {
                    return Err(OwnerSessionError::Record(
                        "grouped Dream relation lacks owner physical-prefix proof".into(),
                    ));
                }
                graph_version = group.relation().graph_version;
            } else if let Some(group) =
                crate::records::dream_supersession::GroupedDreamSupersession::decode(payload)
                    .map_err(OwnerSessionError::Record)?
            {
                if group.cut.owner_id != container.owner_id()
                    || group.cut.graph_version != graph_version
                    || [group.cut.source, group.cut.candidate]
                        .iter()
                        .any(|observed| {
                            !memory_store
                                .record_at_cut(observed.id, memory_head)
                                .is_some_and(|r| {
                                    r.revision == observed.revision
                                        && r.body_id == observed.body_id
                                        && !r.archived
                                })
                        })
                {
                    return Err(OwnerSessionError::Record(
                        "Dream supersession lacks owner physical-prefix proof".into(),
                    ));
                }
                graph_version = group.relation().graph_version;
                memory_head = reliquary_memory_core::history_change_in_frame(&group.delta)?
                    .ok_or(OwnerSessionError::StaleDreamCut)?
                    .1;
            }
            if reliquary_graph::rename_graph_version_in_frame(payload)?
                .is_some_and(|version| version != graph_version)
            {
                return Err(OwnerSessionError::Record(
                    "Community rename does not match its physical Graph prefix".into(),
                ));
            }
            if reliquary_graph::community_graph_version_in_frame(payload)?
                .is_some_and(|version| version > graph_version)
            {
                return Err(OwnerSessionError::Record(
                    "Community references a Graph cut outside its physical prefix".into(),
                ));
            }
        }
        if let Some(relation) = user_memory.as_ref().and_then(UserMemoryRecord::relation) {
            graph_version = relation.graph_version;
        }
    }
    Ok((receipts, separations, observations, phy_observations))
}

pub(super) fn validate_user_memory_prefix(
    container: &Container,
    memories: &MemoryStore,
    memory_head: u64,
    graph_version: u64,
    record: &UserMemoryRecord,
) -> Result<(), OwnerSessionError> {
    let invalid =
        || OwnerSessionError::Record("user Memory mutation differs from owner prefix".into());
    if record.owner != container.owner_id() || record.memory_cut != memory_head {
        return Err(invalid());
    }
    let previous = memories
        .record_at_cut(MemoryId(record.previous.id), memory_head)
        .filter(|previous| record.previous.matches(previous))
        .ok_or_else(invalid)?;
    match &record.effect {
        UserMemoryEffect::Correction {
            graph_cut,
            creation,
            ..
        } => {
            let replacement = reliquary_memory_core::decode_grouped_creation(creation)?;
            if *graph_cut != graph_version
                || previous.archived
                || previous.superseded_by.is_some()
                || replacement.body_id == previous.body_id
                || replacement.category != previous.category
                || replacement.memory_type != previous.memory_type
                || replacement.scope != previous.scope
            {
                return Err(invalid());
            }
        }
        UserMemoryEffect::Retirement { successor, .. } => {
            if !previous.archived
                || previous.authority_kind != "direct"
                || previous.source_ref.is_none()
                || previous.superseded_by != Some(MemoryId(successor.id))
                || !memories
                    .record_at_cut(MemoryId(successor.id), memory_head)
                    .is_some_and(|r| {
                        successor.matches(r) && !r.archived && r.superseded_by.is_none()
                    })
            {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

pub(super) fn validate_frontier_cut(
    owner: [u8; 16],
    memory_store: &MemoryStore,
    memory_head: u64,
    graph_version: u64,
    frontier: &crate::records::dream_frontier::FrontierRecord,
) -> Result<(), OwnerSessionError> {
    if frontier.owner != owner
        || frontier.graph_version != graph_version
        || frontier.memory_version != memory_head
        || std::iter::once(&frontier.source)
            .chain(frontier.candidates.iter())
            .any(|o| {
                !memory_store
                    .record_at_cut(MemoryId(o.id), memory_head)
                    .is_some_and(|r| {
                        r.revision == o.revision && r.body_id.0 == o.body && !r.archived
                    })
            })
    {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    Ok(())
}

pub(super) fn validate_batch_cut(
    owner: [u8; 16],
    memory_store: &MemoryStore,
    memory_head: u64,
    graph_version: u64,
    record: &crate::records::dream_batch::DreamBatchRecord,
) -> Result<(), OwnerSessionError> {
    if record.owner != owner
        || record.memory_version != memory_head
        || record.graph_version != graph_version
        || record.observed.iter().any(|o| {
            !memory_store
                .record_at_cut(MemoryId(o.id), memory_head)
                .is_some_and(|r| r.revision == o.revision && r.body_id.0 == o.body)
        })
        || record.pairs.iter().any(|p| {
            [p.source, p.candidate].iter().any(|id| {
                !memory_store
                    .record_at_cut(MemoryId(*id), memory_head)
                    .is_some_and(|r| !r.archived)
            })
        })
    {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    Ok(())
}
