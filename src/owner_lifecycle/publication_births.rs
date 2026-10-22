use crate::freshness_dream_journal::DirectBirthPrefix;
use crate::owner_lifecycle::OwnerError;
use crate::{Archive, Container, MemoryId};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AcceptedInsomniaBirthMember {
    pub(crate) id: MemoryId,
    pub(crate) memory_version: u64,
    pub(crate) global_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AcceptedInsomniaBirthCohort {
    pub(crate) turn: u64,
    pub(crate) policy_version: u64,
    pub(crate) global_version_start: u64,
    pub(crate) members: Vec<AcceptedInsomniaBirthMember>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AcceptedExternalBirthMember {
    pub(crate) id: MemoryId,
    pub(crate) memory_version: u64,
    pub(crate) global_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AcceptedExternalBirthCohort {
    pub(crate) turn: u64,
    pub(crate) global_version_start: u64,
    pub(crate) members: Vec<AcceptedExternalBirthMember>,
}

/// Accepted publication evidence seen before one physical Dream Begin frame.
/// The state is intentionally usable while a stream is still being scanned:
/// it does not finish Memory replay or require the container's EOF.
pub(crate) struct DreamPopulationPrefix {
    owner: [u8; 16],
    memory: crate::memory_rebuild::MemoryOpenState,
    direct: DirectBirthPrefix,
    insomnia: Vec<AcceptedInsomniaBirthCohort>,
    external: Vec<AcceptedExternalBirthCohort>,
}

impl DreamPopulationPrefix {
    pub(crate) fn new(owner: [u8; 16]) -> Self {
        Self {
            owner,
            memory: crate::memory_rebuild::MemoryOpenState::new(),
            direct: DirectBirthPrefix::new(owner),
            insomnia: Vec::new(),
            external: Vec::new(),
        }
    }

    pub(crate) fn memory_head(&self) -> u64 {
        self.memory.memory_version()
    }

    /// Replay the Memory projection and capture accepted birth facts for a
    /// single physical payload. `global_frontier` is the container clock after
    /// this frame, as supplied by the bounded versioned visitor.
    pub(crate) fn ingest(
        &mut self,
        archive: &Archive,
        memories: &crate::memory_store::MemoryStore,
        chunk: crate::ObjectRef,
        payload: &[u8],
        global_frontier: u64,
    ) -> Result<(), OwnerError> {
        let body = if let Some(envelope) = crate::container::publication_envelope::decode(payload)?
        {
            let _ = self
                .memory
                .ingest_publication_envelope(envelope, global_frontier)?;
            envelope.body
        } else {
            crate::owner_lifecycle::memory_frames::ingest(
                &mut self.memory,
                chunk,
                payload,
                global_frontier,
            )?;
            payload
        };

        self.direct
            .ingest(payload)
            .map_err(|message| OwnerError::Freshness(message.into()))?;

        if let Some(completion) = crate::insomnia::completion::decode_completion(body)
            .map_err(|message| OwnerError::Freshness(message.into()))?
            && let Some(cohort) = validate_insomnia_birth_cohort(archive, memories, &completion)?
        {
            self.insomnia.push(cohort);
        }
        if let Some(publication) = crate::memory_external_publication::decode(body)?
            && let Some(cohort) = validate_external_birth_cohort(archive, memories, &publication)?
        {
            self.external.push(cohort);
        }
        Ok(())
    }

    /// Materialize the initial Dream population at the exact physical-prefix
    /// Memory cut. A source outside that cut, or a cut that disagrees with the
    /// replayed prefix, is rejected rather than inferred from current state.
    pub(crate) fn materialize(
        &self,
        memories: &crate::memory_store::MemoryStore,
        owner: Option<[u8; 16]>,
        memory_cut: u64,
        source_id: MemoryId,
    ) -> Result<BTreeSet<MemoryId>, OwnerError> {
        if owner != Some(self.owner) || self.direct.owner_uuid() != owner {
            return Err(OwnerError::Freshness(
                "Dream population prefix belongs to another owner".into(),
            ));
        }
        if memory_cut != self.memory_head() {
            return Err(OwnerError::Freshness(
                "Dream population Memory cut differs from physical prefix".into(),
            ));
        }
        let first = memories
            .first_publication_prefix(memory_cut)?
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        let source_global = *first.get(&source_id).ok_or_else(|| {
            OwnerError::Freshness("Dream source is outside its physical Memory prefix".into())
        })?;
        let mut birth_ids = direct_birth_ids(&self.direct, memories, &first);
        let mut excluded_cohorts = BTreeSet::new();
        for cohort in &self.insomnia {
            if cohort.members.iter().any(|member| member.id == source_id) {
                excluded_cohorts.extend(cohort.members.iter().map(|member| member.id));
            }
            birth_ids.extend(cohort.members.iter().map(|member| member.id));
        }
        for cohort in &self.external {
            if cohort.members.iter().any(|member| member.id == source_id) {
                excluded_cohorts.extend(cohort.members.iter().map(|member| member.id));
            }
            birth_ids.extend(cohort.members.iter().map(|member| member.id));
        }
        Ok(birth_ids
            .into_iter()
            .filter(|id| !excluded_cohorts.contains(id))
            .filter(|id| first.get(id).is_some_and(|global| *global < source_global))
            .collect())
    }

    /// Scan through a previously accepted physical boundary. Later bytes are
    /// not visited, and the container's live clock remains untouched.
    pub(crate) fn scan_through(
        container: &Container,
        archive: &Archive,
        memories: &crate::memory_store::MemoryStore,
        owner: [u8; 16],
        prefix_end: u64,
    ) -> Result<Self, OwnerError> {
        let mut state = Self::new(owner);
        {
            let mut ingest_frame =
                |frame: crate::container::PhysicalFrame| -> Result<(), OwnerError> {
                    state.ingest(
                        archive,
                        memories,
                        frame.object,
                        &frame.payload,
                        frame.latest_global,
                    )
                };
            let mut cursor = container.physical_cursor_through(prefix_end)?;
            while let Some(frame) = cursor.next_frame()? {
                ingest_frame(frame)?;
            }
        }
        Ok(state)
    }
}

pub(crate) fn direct_birth_ids(
    prefix: &DirectBirthPrefix,
    memories: &crate::memory_store::MemoryStore,
    first: &BTreeMap<MemoryId, u64>,
) -> BTreeSet<MemoryId> {
    let mut births = BTreeSet::new();
    for (id, mutation) in prefix.pending_intents() {
        if first.contains_key(id) && memories.first_mutation_id(*id) == Some(mutation.as_str()) {
            births.insert(*id);
        }
    }
    for (id, mutations) in prefix.consumed_intents() {
        if first.contains_key(id)
            && memories
                .first_mutation_id(*id)
                .is_some_and(|mutation| mutations.contains(mutation))
        {
            births.insert(*id);
        }
    }
    births.extend(
        prefix
            .replay_births()
            .iter()
            .filter(|id| first.contains_key(id))
            .copied(),
    );
    births
}

/// Validate an external-source birth receipt against the accepted Archive and
/// Memory projections. Publications without a birth turn remain non-births.
pub(crate) fn validate_external_birth_cohort(
    archive: &Archive,
    memories: &crate::memory_store::MemoryStore,
    publication: &crate::memory_external_publication::Publication,
) -> Result<Option<AcceptedExternalBirthCohort>, OwnerError> {
    let Some(turn) = publication.birth_turn else {
        return Ok(None);
    };
    let archive_version = archive
        .record_versions()
        .iter()
        .filter(|record| record.global_version < publication.global_start)
        .map(|record| record.archive_version)
        .max()
        .unwrap_or(0);
    if turn != archive.activity_cut_at_archive_version(archive_version)? {
        return Err(OwnerError::Freshness(
            "external publication birth activity cut differs".into(),
        ));
    }
    let mut members = Vec::with_capacity(publication.records.len());
    for record in &publication.records {
        if memories.first_mutation_id(record.id) != Some(record.mutation_id.as_str()) {
            return Err(OwnerError::Freshness(
                "external publication birth identity differs".into(),
            ));
        }
        members.push(AcceptedExternalBirthMember {
            id: record.id,
            memory_version: record.memory_version,
            global_version: record.global_version,
        });
    }
    Ok(Some(AcceptedExternalBirthCohort {
        turn,
        global_version_start: publication.global_start,
        members,
    }))
}

pub(crate) fn validate_insomnia_birth_cohort(
    archive: &Archive,
    memories: &crate::memory_store::MemoryStore,
    completion: &crate::insomnia::completion::InsomniaCompletion,
) -> Result<Option<AcceptedInsomniaBirthCohort>, OwnerError> {
    let Some(turn) = completion.freshness_birth_turn else {
        return Ok(None);
    };
    let policy_version = completion.freshness_policy_version;
    if policy_version != Some(crate::freshness::FRESHNESS_POLICY_VERSION as u64)
        || turn > archive.rel_turn_count()
    {
        return Err(OwnerError::Freshness(
            "grouped birth proof has invalid policy or activity cut".into(),
        ));
    }
    if !completion.freshness_birth_memory_ids.is_empty() {
        let archive_version = archive
            .record_versions()
            .iter()
            .filter(|record| record.global_version < completion.global_version_start)
            .map(|record| record.archive_version)
            .max()
            .unwrap_or(0);
        if turn != archive.activity_cut_at_archive_version(archive_version)? {
            return Err(OwnerError::Freshness(
                "grouped birth differs from accepted completion activity cut".into(),
            ));
        }
    }
    let mut members = Vec::with_capacity(completion.freshness_birth_memory_ids.len());
    for id in &completion.freshness_birth_memory_ids {
        let record = completion
            .records
            .iter()
            .find(|record| record.id == *id && record.revision == 1)
            .ok_or_else(|| {
                OwnerError::Freshness(
                    "grouped birth proof lacks accepted first Memory record".into(),
                )
            })?;
        if memories.first_mutation_id(*id) != Some(record.mutation_id.as_str()) {
            return Err(OwnerError::Freshness(
                "grouped birth differs from accepted first Memory identity".into(),
            ));
        }
        members.push(AcceptedInsomniaBirthMember {
            id: *id,
            memory_version: record.memory_version,
            global_version: record.global_version,
        });
    }
    Ok(Some(AcceptedInsomniaBirthCohort {
        turn,
        policy_version: policy_version.unwrap(),
        global_version_start: completion.global_version_start,
        members,
    }))
}
