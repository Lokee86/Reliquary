use super::perception_lifecycle::support::{has_direct_source, owner_hex, text_field};
use super::source_eligibility::SourceEligibility;
use super::*;
use reliquary_perception_store::RetainedResolutionRecord;

mod phy;
mod reads;
pub use phy::{PhyObservationBatch, PhyObservationCut};
pub(super) use phy::{PhyObservationHistory, PhyObservationPublicationProof};

pub(super) const PHY_PREFIX: &[u8] = b"PHYOBS";
pub(crate) use phy::PHY_MARKER;
use reliquary_perception_store::observation::*;
use sha2::{Digest, Sha256};

pub(crate) const PREFIX: &[u8] = b"RELOBS";
pub(crate) const MARKER: &[u8; 8] = b"RELOBS01";
const CLAIM_VECTOR_MARKER: &[u8; 8] = b"RELOBS02";
const MAX_RECORD_BYTES: usize = 4 * 1024 * 1024;
const MAX_STANDING_CHECK: usize = MAX_OBSERVATION_DEPENDENCIES + 1;
const MAX_PAGE: usize = 128;

fn is_observation(prefix: &[u8]) -> bool {
    match reliquary_container::decode_publication(prefix) {
        Ok(Some(envelope)) => envelope.body.starts_with(PREFIX),
        Ok(None) => prefix.starts_with(PREFIX),
        Err(_) => true,
    }
}

#[derive(Default)]
pub(super) struct ArchiveReplayCuts {
    pub observations: BTreeMap<u64, (u64, u64)>,
    pub grouped_publications: BTreeMap<u64, (u64, u64)>,
    pub import_receipts: Vec<(u64, [u8; 32])>,
}

pub(super) fn open_archive(
    container: &Container,
    leased: bool,
) -> Result<(Archive, ArchiveReplayCuts), OwnerSessionError> {
    let mut cuts = ArchiveReplayCuts::default();
    // Find the grouped carrier positions without reading their bodies. The
    // Archive replay can supply each preceding cut as it advances over them.
    let mut grouped = container.matching_physical_cursor(|prefix| {
        prefix.starts_with(b"RELINSC") || prefix.starts_with(b"RELDRC01")
    })?;
    while let Some(frame) = grouped.next_frame()? {
        cuts.grouped_publications
            .insert(frame.object().offset, (0, 0));
    }
    drop(grouped);
    let mut grouped = cuts.grouped_publications.iter_mut().peekable();
    let archive = Archive::open_observing_frames(
        container,
        leased,
        |prefix| is_observation(prefix) || reliquary_archive::archive_replay_frame(prefix),
        |frame| {
            let cut = (frame.archive_version, frame.activity);
            while let Some((_, grouped_cut)) =
                grouped.next_if(|(offset, _)| **offset < frame.object.offset)
            {
                *grouped_cut = cut;
            }
            if let Some(key) = frame.import_receipt_key {
                cuts.import_receipts.push((frame.object.offset, key));
            }
            if let Ok(Some(publication)) = reliquary_container::decode_publication(frame.payload)
                && publication.body.starts_with(PREFIX)
            {
                cuts.observations.insert(publication.version, cut);
            }
        },
    )?;
    for (_, cut) in grouped {
        *cut = (archive.archive_version(), archive.rel_turn_count());
    }
    // A naked Observation marker has no clock and must never be ignored.
    if cuts.observations.is_empty() {
        let mut frames = container.matching_physical_cursor(|prefix| prefix.starts_with(PREFIX))?;
        if frames.next_frame()?.is_some() {
            return Err(invalid("unclocked Observation carrier"));
        }
    }
    Ok((archive, cuts))
}

pub(super) fn decode(frame: &[u8]) -> Result<Option<(u64, ObservationRecord)>, OwnerSessionError> {
    let envelope = reliquary_container::decode_publication(frame)?;
    let body = envelope.as_ref().map_or(frame, |entry| entry.body);
    if !body.starts_with(PREFIX) {
        return Ok(None);
    }
    let envelope = envelope
        .filter(|entry| entry.transaction_time_ns.is_none() && !entry.bound_reservation)
        .ok_or_else(|| invalid("expected an untimed REL publication envelope"))?;
    if (!body.starts_with(MARKER) && !body.starts_with(CLAIM_VECTOR_MARKER))
        || body.len() > MAX_RECORD_BYTES
    {
        return Err(invalid("unsupported Observation publication"));
    }
    let record = rmp_serde::from_slice(&body[MARKER.len()..]).map_err(|_| invalid("encoding"))?;
    if encode(&record)? != body {
        return Err(invalid("noncanonical encoding"));
    }
    Ok(Some((envelope.version, record)))
}

fn encode(record: &ObservationRecord) -> Result<Vec<u8>, OwnerSessionError> {
    let marker = if record.claim_vectors.is_some() {
        CLAIM_VECTOR_MARKER
    } else {
        MARKER
    };
    let mut body = marker.to_vec();
    body.extend(rmp_serde::to_vec_named(record).map_err(|_| invalid("encoding"))?);
    if body.len() > MAX_RECORD_BYTES {
        return Err(invalid("record size"));
    }
    Ok(body)
}

#[derive(Clone, Copy)]
pub(super) struct ObservationPrefix<'a> {
    pub owner: [u8; 16],
    pub generation: [u8; 32],
    pub memories: &'a MemoryStore,
    pub profiles: &'a CompatibilityProfileStore,
    pub memory_cut: u64,
    pub perception: &'a PerceptionStore,
    pub perception_cut: u64,
    pub retained_ordinal_cut: Option<u64>,
    pub observed_bodies: Option<&'a BTreeSet<MemoryBodyId>>,
    pub unobserved_suffix_bodies: Option<&'a BTreeSet<MemoryBodyId>>,
    pub source_eligibility: ObservationAuthority<'a>,
}

#[derive(Clone, Copy)]
pub(super) enum ObservationAuthority<'a> {
    Rel(SourceEligibility<'a>),
    Phy(PhySettlements<'a>),
}

#[derive(Clone, Copy)]
pub(super) enum PhySettlements<'a> {
    Admitted(&'a DreamEvidenceStore, u64),
    Physical(&'a BTreeMap<MemoryId, u64>),
}

impl PhySettlements<'_> {
    fn position(self, id: MemoryId) -> Option<u64> {
        match self {
            Self::Admitted(dreams, through) => dreams
                .settlement_position(id)
                .filter(|position| *position <= through),
            Self::Physical(settled) => settled.get(&id).copied(),
        }
    }

    pub(super) fn allows(self, memories: &MemoryStore, id: MemoryId, cut: u64) -> bool {
        self.position(id).is_some() || memories.has_retained_phy_import_authority_at(id, cut)
    }
}

impl ObservationPrefix<'_> {
    fn check_owner(&self, cite: &ObservationCitation) -> Result<(), OwnerSessionError> {
        if cite.owner_id != self.owner
            || cite.history_generation != self.generation
            || cite.revision == 0
            || cite.start_byte >= cite.end_byte
        {
            return Err(invalid("citation scope or coordinates"));
        }
        Ok(())
    }

    fn body_in_prefix(&self, id: MemoryBodyId) -> bool {
        self.observed_bodies
            .is_none_or(|bodies| bodies.contains(&id))
            && self
                .unobserved_suffix_bodies
                .is_none_or(|bodies| !bodies.contains(&id))
    }

    fn allows_memory_source(&self, record: &reliquary_memory_core::MemoryRecord) -> bool {
        match self.source_eligibility {
            ObservationAuthority::Rel(eligibility) => eligibility.allows(record),
            ObservationAuthority::Phy(_) => self
                .memories
                .terminal_at_cut(record.id, self.memory_cut)
                .is_none(),
        }
    }

    fn allows_entity_source(&self, entity: &Entity) -> bool {
        match self.source_eligibility {
            ObservationAuthority::Rel(eligibility) => {
                eligibility.allows_entity(entity, self.memories, self.memory_cut)
            }
            ObservationAuthority::Phy(_) => true,
        }
    }

    fn eligible_memory(&self, id: [u8; 32]) -> Option<&reliquary_memory_core::MemoryRecord> {
        self.memories
            .record_at_cut(MemoryId(id), self.memory_cut)
            .filter(|record| {
                !record.archived
                    && self.allows_memory_source(record)
                    && record.superseded_by.is_none()
                    && matches!(record.lifecycle_state.as_str(), "knowledge" | "canonical")
                    && self.body_in_prefix(record.body_id)
                    && match self.source_eligibility {
                        ObservationAuthority::Rel(_) => {
                            has_direct_source(record)
                                && record
                                    .source_ref
                                    .as_ref()
                                    .is_some_and(|source| source.owner_id == owner_hex(&self.owner))
                        }
                        ObservationAuthority::Phy(settled) => {
                            settled.allows(self.memories, record.id, self.memory_cut)
                        }
                    }
            })
    }

    fn current_memory_text(
        &self,
        cite: &ObservationCitation,
        id: [u8; 32],
    ) -> Result<String, OwnerSessionError> {
        if !self.eligible_memory(id).is_some_and(|record| {
            record.revision == cite.revision && record.body_id.0 == cite.body_id
        }) {
            return Err(invalid("citation Memory changed or is ineligible"));
        }
        self.memory_text(cite, id)
    }

    fn memory_text(
        &self,
        cite: &ObservationCitation,
        id: [u8; 32],
    ) -> Result<String, OwnerSessionError> {
        let record = self
            .memories
            .revision_record(MemoryId(id), Some(cite.revision))?;
        if !self.allows_memory_source(record) {
            return Err(invalid("citation Memory source was removed"));
        }
        let (record, body) = self
            .memories
            .revision_projection(MemoryId(id), Some(cite.revision))?;
        let body = body
            .filter(|_| {
                record.memory_version <= self.memory_cut
                    && record.body_id.0 == cite.body_id
                    && (matches!(self.source_eligibility, ObservationAuthority::Phy(_))
                        || has_direct_source(record))
                    && self.body_in_prefix(record.body_id)
            })
            .ok_or_else(|| invalid("citation body revision unavailable"))?;
        valid_excerpt(text_field(&body, cite.field), cite).map(str::to_owned)
    }

    fn check_participant(
        &self,
        participant: &ObservationParticipant,
        citations: &[ObservationCitation],
    ) -> Result<(), OwnerSessionError> {
        let mention = participant.mention;
        let cite = citations
            .iter()
            .find(|cite| cite.source == ObservationSource::Memory(mention.key.memory_id))
            .ok_or_else(|| invalid("participant lacks exact source evidence"))?;
        let surface = self.current_memory_text(
            &ObservationCitation {
                field: mention.key.field,
                start_byte: mention.key.start_byte,
                end_byte: mention.key.end_byte,
                ..*cite
            },
            mention.key.memory_id,
        )?;
        let entity = self
            .perception
            .entity_at_cut(participant.entity_id, self.perception_cut)
            .filter(|entity| {
                entity.revision == participant.revision
                    && entity.kind != reliquary_perception_store::PRINCIPAL_ENTITY_KIND
                    && !entity.kind.eq_ignore_ascii_case("generic")
                    && self.allows_entity_source(entity)
            })
            .ok_or_else(|| invalid("participant Entity changed or is ineligible"))?;
        if !super::identity_evidence::quote_names_entity(
            &surface,
            &entity.canonical_name,
            &entity.aliases,
        ) {
            return Err(invalid(
                "participant mention does not name the cited Entity",
            ));
        }
        let matches = match self
            .perception
            .mention_at_cut(mention.key, self.perception_cut)
        {
            Some(value) => {
                mention.retained_ordinal.is_none()
                    && value.revision == mention.revision
                    && matches!(value.status, reliquary_perception_store::MemoryEntityResolutionStatus::Resolved { entity_id, .. }
                    if entity_id == participant.entity_id)
            }
            None => self
                .retained_ordinal_cut
                .and_then(|cut| self.perception.retained_resolution_at_cut(mention.key, cut))
                .filter(|record| Some(record.source_ordinal) == mention.retained_ordinal)
                .and_then(RetainedResolutionRecord::resolved)
                .is_some_and(|(value, entity)| {
                    value.revision == mention.revision && entity == participant.entity_id
                }),
        };
        if !matches {
            return Err(invalid("participant mention changed or is unresolved"));
        }
        Ok(())
    }

    fn check_record(
        &self,
        store: &ObservationStore,
        record: &ObservationRecord,
        version: u64,
    ) -> Result<ObservationBatchReceipt, OwnerSessionError> {
        if record.batch.owner_id != self.owner
            || record.batch.history_generation != self.generation
            || record.memory_cut != self.memory_cut
            || record.perception_cut != self.perception_cut
            || self.lineages(store, &record.batch)? != record.lineages
        {
            return Err(invalid("record differs from owner physical prefix"));
        }
        if let Some(progress) = &record.batch.progress
            && let ObservationAuthority::Rel(eligibility) = self.source_eligibility
        {
            super::perception_audit::PerceptionAuditProgress::verify_prefix(
                &progress.bytes,
                self.owner,
                self.generation,
                self.memory_cut,
                self.perception_cut,
                store.frontier(),
                record.rel_turn,
                eligibility.cut(),
            )?;
        }
        let mut checker = StandingCheck::new(*self, store);
        if let Some(vectors) = &record.claim_vectors {
            let profile = self
                .profiles
                .get(CompatibilityProfileId(vectors.profile))
                .ok_or_else(|| invalid("unknown claim vector profile"))?;
            for row in &vectors.rows {
                let ObservationSource::Observation(id) = row.citation.source else {
                    return Err(invalid("claim vector source is not an Observation"));
                };
                let head = store
                    .head(id)
                    .ok_or_else(|| invalid("claim vector source is missing"))?;
                if head.full_citation() != row.citation
                    || !super::vector_publication::valid_profile_vector(profile, &row.vector)
                {
                    return Err(invalid("claim vector source or profile differs"));
                }
                checker.current_lineage(&row.citation)?;
            }
        }
        let changes = record
            .batch
            .operations
            .iter()
            .filter_map(|operation| match operation {
                ObservationOperation::Pending { id, .. }
                | ObservationOperation::Successor { id, .. }
                | ObservationOperation::Withdraw { id, .. } => Some(*id),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        // A transaction may publish independent claims and comparisons. Its
        // proposed revisions must not rely on another head it is invalidating.
        if !changes.is_empty() {
            for operation in &record.batch.operations {
                if let ObservationOperation::Create { draft }
                | ObservationOperation::Successor { draft, .. } = operation
                    && invalidated_by_batch(store, &draft.citations, &changes)
                {
                    return Err(invalid("batch invalidates one of its own citations"));
                }
            }
        }
        Ok(store.prepare(record, version, |id| checker.requires_own_review(id))?)
    }

    fn lineages(
        &self,
        store: &ObservationStore,
        batch: &ObservationBatch,
    ) -> Result<Vec<Vec<ObservationMemoryLineage>>, OwnerSessionError> {
        if batch.operations.len() > MAX_OBSERVATION_OPERATIONS {
            return Err(invalid("operation count"));
        }
        let mut checker = StandingCheck::new(*self, store);
        batch
            .operations
            .iter()
            .map(|operation| {
                let mut lineage = BTreeSet::new();
                match operation {
                    ObservationOperation::Create { draft }
                    | ObservationOperation::Successor { draft, .. } => {
                        draft.validate()?;
                        for cite in &draft.citations {
                            if let ObservationProfile::Relational(_) = &draft.profile
                                && let ObservationSource::Observation(id) = cite.source
                                && store.head(id).is_some_and(|row| {
                                    matches!(row.draft.profile, ObservationProfile::Relational(_))
                                })
                            {
                                return Err(invalid(
                                    "relational target cites a relational Observation",
                                ));
                            }
                            lineage.extend(checker.current_lineage(cite)?);
                            if lineage.len() > MAX_OBSERVATION_LINEAGE {
                                return Err(invalid("lineage limit"));
                            }
                        }
                        for cite in &draft.reviewed {
                            match operation {
                                ObservationOperation::Successor { id, .. } => {
                                    checker.review_evidence(*id, cite)?
                                }
                                _ => {
                                    checker.current_lineage(cite)?;
                                }
                            }
                        }
                        for participant in draft.profile.participants() {
                            self.check_participant(participant, &draft.citations)?;
                        }
                    }
                    ObservationOperation::Reaffirm { id, reviewed, .. } => {
                        let row = store.head(*id).ok_or_else(|| invalid("review target"))?;
                        if checker.has_invalid_support(row)
                            || row.draft.citations.iter().any(|cite| {
                                matches!(cite.source, ObservationSource::Observation(source)
                                    if checker.standing(source) != ObservationStanding::Current)
                            })
                        {
                            return Err(invalid("reaffirmation evidence no longer eligible"));
                        }
                        for cite in reviewed {
                            checker.review_evidence(*id, cite)?;
                        }
                    }
                    ObservationOperation::Withdraw { id, reviewed, .. }
                    | ObservationOperation::Pending {
                        id,
                        evidence: reviewed,
                        ..
                    } => {
                        for cite in reviewed {
                            checker.review_evidence(*id, cite)?;
                        }
                    }
                    ObservationOperation::Comparison { comparison } => {
                        checker.current_lineage(&comparison.citation)?;
                    }
                    ObservationOperation::SemanticAward {
                        evidence, activity, ..
                    } => {
                        if evidence.source != ObservationSource::Memory(activity.source)
                            || activity.turn == 0
                        {
                            return Err(invalid("semantic award activity"));
                        }
                        checker.current_lineage(evidence)?;
                    }
                }
                Ok(lineage.into_iter().collect())
            })
            .collect()
    }

    pub(super) fn admit(
        &self,
        store: &mut ObservationStore,
        frame: &[u8],
        archive_cuts: &BTreeMap<u64, (u64, u64)>,
        active_profiles: &BTreeSet<[u8; 32]>,
    ) -> Result<bool, OwnerSessionError> {
        let Some((version, record)) = decode(frame)? else {
            return Ok(false);
        };
        if archive_cuts.get(&version).copied() != Some((record.archive_cut, record.rel_turn)) {
            return Err(invalid("physical Archive cut differs"));
        }
        if record
            .claim_vectors
            .as_ref()
            .is_some_and(|vectors| !active_profiles.contains(&vectors.profile))
        {
            return Err(invalid(
                "claim vector profile is inactive at its publication",
            ));
        }
        let receipt = self.check_record(store, &record, version)?;
        store.apply(record, receipt);
        Ok(true)
    }
}

fn invalidated_by_batch(
    store: &ObservationStore,
    citations: &[ObservationCitation],
    changes: &BTreeSet<ObservationId>,
) -> bool {
    let sources = |citations: &[ObservationCitation]| {
        citations
            .iter()
            .filter_map(|citation| match citation.source {
                ObservationSource::Observation(id) => Some(id),
                ObservationSource::Memory(_) => None,
            })
            .collect::<Vec<_>>()
    };
    let mut queue = sources(citations);
    let mut visited = BTreeSet::new();
    while let Some(id) = queue.pop() {
        if changes.contains(&id) {
            return true;
        }
        if visited.insert(id)
            && let Some(row) = store.head(id)
        {
            if visited.len() > MAX_STANDING_CHECK {
                return true;
            }
            queue.extend(sources(&row.draft.citations));
        }
    }
    false
}

struct StandingCheck<'a> {
    prefix: ObservationPrefix<'a>,
    store: &'a ObservationStore,
    memo: BTreeMap<ObservationId, ObservationStanding>,
}

impl<'a> StandingCheck<'a> {
    fn new(prefix: ObservationPrefix<'a>, store: &'a ObservationStore) -> Self {
        Self {
            prefix,
            store,
            memo: BTreeMap::new(),
        }
    }

    fn standing(&mut self, id: ObservationId) -> ObservationStanding {
        // Evaluate dependencies before their children using a stack. The
        // bound matches publication admission and applies to one claim.
        let mut stack = vec![(id, false)];
        let mut visited = BTreeSet::new();
        while let Some((candidate, expanded)) = stack.pop() {
            if self.memo.contains_key(&candidate) {
                continue;
            }
            if expanded {
                let inherited = self
                    .dependencies(candidate)
                    .any(|source| self.memo.get(&source) != Some(&ObservationStanding::Current));
                let standing = if inherited {
                    ObservationStanding::Pending(ObservationPendingReason::UpstreamReview)
                } else {
                    ObservationStanding::Current
                };
                self.memo.insert(candidate, standing);
                continue;
            }
            if !visited.insert(candidate) || visited.len() > MAX_STANDING_CHECK {
                // Admitted histories cannot contain this cycle or exceed the
                // bound. A read of an unavailable proof fails closed.
                return ObservationStanding::Pending(ObservationPendingReason::UpstreamReview);
            }
            if let Some(standing) = self.local_standing(candidate) {
                self.memo.insert(candidate, standing);
            } else {
                stack.push((candidate, true));
                stack.extend(self.dependencies(candidate).map(|source| (source, false)));
            }
        }
        self.memo[&id]
    }

    fn dependencies(&self, id: ObservationId) -> impl Iterator<Item = ObservationId> + '_ {
        self.store
            .head(id)
            .into_iter()
            .flat_map(|row| &row.draft.citations)
            .filter_map(|cite| match cite.source {
                ObservationSource::Observation(source) => Some(source),
                ObservationSource::Memory(_) => None,
            })
    }

    fn local_standing(&self, id: ObservationId) -> Option<ObservationStanding> {
        if let Some(reason) = self.store.withdrawal(id) {
            return Some(ObservationStanding::Withdrawn(reason));
        }
        if self
            .store
            .head(id)
            .is_none_or(|head| self.has_invalid_support(head))
        {
            return Some(ObservationStanding::Pending(
                ObservationPendingReason::ChangedSupport,
            ));
        }
        self.store
            .pending(id)
            .map(|(reason, _)| ObservationStanding::Pending(reason))
    }

    fn requires_own_review(&mut self, id: ObservationId) -> bool {
        matches!(
            self.standing(id),
            ObservationStanding::Pending(
                ObservationPendingReason::NewEvidence | ObservationPendingReason::ChangedSupport
            )
        ) || self.store.pending(id).is_some()
    }

    fn has_invalid_support(&self, row: &ObservationRevision) -> bool {
        for cite in &row.draft.citations {
            match cite.source {
                ObservationSource::Memory(id) => {
                    if self.prefix.current_memory_text(cite, id).is_err() {
                        return true;
                    }
                }
                ObservationSource::Observation(id) => {
                    if !self
                        .store
                        .head(id)
                        .is_some_and(|source| source.revision == cite.revision)
                        || self.store.withdrawal(id).is_some()
                    {
                        return true;
                    }
                }
            }
        }
        row.draft.profile.participants().iter().any(|p| {
            self.prefix
                .check_participant(p, &row.draft.citations)
                .is_err()
        })
    }

    fn current_lineage(
        &mut self,
        cite: &ObservationCitation,
    ) -> Result<Vec<ObservationMemoryLineage>, OwnerSessionError> {
        self.prefix.check_owner(cite)?;
        match cite.source {
            ObservationSource::Memory(id) => {
                self.prefix.current_memory_text(cite, id)?;
                Ok(vec![ObservationMemoryLineage {
                    memory_id: id,
                    revision: cite.revision,
                    body_id: cite.body_id,
                }])
            }
            ObservationSource::Observation(id) => {
                let revision = self
                    .store
                    .head(id)
                    .filter(|revision| cite.revision == revision.revision)
                    .ok_or_else(|| invalid("cited Observation revision changed"))?;
                observation_text(revision, cite)?;
                if self.standing(id) != ObservationStanding::Current {
                    return Err(invalid("cited Observation is pending or withdrawn"));
                }
                Ok(revision.lineage.clone())
            }
        }
    }

    fn review_evidence(
        &mut self,
        id: ObservationId,
        cite: &ObservationCitation,
    ) -> Result<(), OwnerSessionError> {
        self.prefix.check_owner(cite)?;
        if matches!(cite.source, ObservationSource::Observation(source) if source == id) {
            return Err(invalid("Observation review self-citation"));
        }
        if self
            .store
            .head(id)
            .is_some_and(|head| head.draft.citations.contains(cite))
            || self
                .store
                .pending(id)
                .is_some_and(|(_, previous)| previous.contains(cite))
        {
            // The exact coordinates were previously admitted, including when
            // the source was later invalidated. Keep that reference in history.
            return Ok(());
        }
        self.current_lineage(cite).map(|_| ())
    }
}

fn valid_excerpt<'a>(
    body: &'a str,
    cite: &ObservationCitation,
) -> Result<&'a str, OwnerSessionError> {
    body.get(cite.start_byte as usize..cite.end_byte as usize)
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| invalid("citation outside UTF-8 body"))
}

fn observation_text<'a>(
    revision: &'a ObservationRevision,
    cite: &ObservationCitation,
) -> Result<&'a str, OwnerSessionError> {
    if cite.field != reliquary_perception_store::MemoryTextField::Content
        || revision.draft.body_id() != cite.body_id
    {
        return Err(invalid("citation Observation body identity"));
    }
    valid_excerpt(&revision.draft.proposition, cite)
}

fn invalid(reason: &str) -> OwnerSessionError {
    OwnerSessionError::Record(format!("invalid Observation publication: {reason}"))
}

impl RelSession {
    fn observation_store(&self) -> &ObservationStore {
        &self.observations
    }

    fn observation_prefix(&self) -> ObservationPrefix<'_> {
        ObservationPrefix {
            owner: self.owner_id,
            generation: self.history_generation,
            memories: &self.common.memories,
            profiles: &self.common.profiles,
            memory_cut: self.common.memories.memory_version(),
            perception: &self.common.perception,
            perception_cut: self.common.perception.version(),
            retained_ordinal_cut: self.common.perception.retained_resolution_ordinal(),
            observed_bodies: None,
            unobserved_suffix_bodies: None,
            source_eligibility: ObservationAuthority::Rel(self.source_eligibility()),
        }
    }
}

impl RelWriter {
    pub fn publish_observation_batch(
        &mut self,
        batch: ObservationBatch,
    ) -> Result<ObservationBatchReceipt, OwnerSessionError> {
        self.publish_observation_record(batch, None)
    }

    pub fn publish_observation_claim_vectors(
        &mut self,
        profile: CompatibilityProfileId,
        mut rows: Vec<ObservationClaimVector>,
    ) -> Result<(), OwnerSessionError> {
        self.ensure_write_ready()?;
        let rel = &self.session;
        if let Some((batch, vectors)) = prepare_claim_vectors(
            &rel.observations,
            rel.active_compatibility_profile(profile).is_some(),
            rel.owner_id,
            rel.history_generation,
            profile,
            &mut rows,
        )? {
            self.publish_observation_record(batch, Some(vectors))?;
        }
        Ok(())
    }

    fn publish_observation_record(
        &mut self,
        batch: ObservationBatch,
        claim_vectors: Option<ObservationClaimVectors>,
    ) -> Result<ObservationBatchReceipt, OwnerSessionError> {
        self.ensure_write_ready()?;
        let before = self.container.writer_lease_proof();
        if before.is_none() || before != self.session.writer_lease_proof {
            return Err(invalid("owner writer requires reopen"));
        }
        let store = &self.session.observations;
        if let Some(receipt) = store.lookup_receipt(&batch)? {
            return Ok(receipt.clone());
        }
        let prefix = self.session.observation_prefix();
        let version = self
            .container
            .latest_version()
            .checked_add(1)
            .ok_or_else(|| invalid("publication version exhausted"))?;
        let record = ObservationRecord {
            lineages: prefix.lineages(store, &batch)?,
            batch,
            memory_cut: prefix.memory_cut,
            perception_cut: prefix.perception_cut,
            archive_cut: self.session.archive.archive_version(),
            rel_turn: self.session.archive.rel_turn_count(),
            claim_vectors,
        };
        let receipt = prefix.check_record(store, &record, version)?;
        let scores = self.session.prepare_observation_scores(&record, &receipt)?;
        let payload = encode(&record)?;
        let publication = self
            .container
            .append_publication_envelope(&payload, None)
            .and_then(|(published, _)| self.container.sync().map(|()| published));
        match publication {
            Ok(published) if version == published => {
                self.session.observation_scores.extend(scores);
                self.session.observations.apply(record, receipt.clone());
                self.session.writer_lease_proof = self.container.writer_lease_proof();
                Ok(receipt)
            }
            Ok(_) => {
                self.session.write_poisoned = true;
                Err(invalid("unexpected publication version"))
            }
            Err(error) => {
                self.session.write_poisoned = true;
                Err(error.into())
            }
        }
    }
}

fn prepare_claim_vectors(
    store: &ObservationStore,
    active_profile: bool,
    owner_id: [u8; 16],
    history_generation: [u8; 32],
    profile: CompatibilityProfileId,
    rows: &mut Vec<ObservationClaimVector>,
) -> Result<Option<(ObservationBatch, ObservationClaimVectors)>, OwnerSessionError> {
    if rows.is_empty() || rows.len() > MAX_OBSERVATION_CLAIM_VECTOR_ROWS || !active_profile {
        return Err(invalid("claim vector batch or profile is unavailable"));
    }
    rows.sort_by_key(|row| row.citation);
    if rows
        .windows(2)
        .any(|pair| pair[0].citation == pair[1].citation)
    {
        return Err(invalid("claim vector batch repeats a citation"));
    }
    rows.retain(|row| store.claim_vector(profile.0, &row.citation).is_none());
    let Some(first) = rows.first() else {
        return Ok(None);
    };
    let ObservationSource::Observation(id) = first.citation.source else {
        return Err(invalid("claim vector source is not an Observation"));
    };
    let policy = store
        .head(id)
        .ok_or_else(|| invalid("claim vector source is missing"))?
        .policy;
    let mut fingerprint = Sha256::new();
    fingerprint.update(b"reliquary-observation-claim-vector-batch-v1");
    fingerprint.update(profile.0);
    for row in rows.iter() {
        fingerprint.update(
            rmp_serde::to_vec_named(&row.citation)
                .map_err(|_| invalid("claim vector citation encoding"))?,
        );
    }
    let batch = ObservationBatch {
        owner_id,
        history_generation,
        input_fingerprint: fingerprint.finalize().into(),
        policy,
        operations: Vec::new(),
        progress: None,
    };
    Ok(Some((
        batch,
        ObservationClaimVectors {
            profile: profile.0,
            rows: std::mem::take(rows),
        },
    )))
}

impl RelSession {
    pub fn observation_freshness(&self, id: ObservationId) -> Option<i16> {
        self.observation_scores
            .get(&id)?
            .score_at_with_policy(self.archive.rel_turn_count(), &self.freshness.policy())
            .ok()
    }

    pub fn observation_award(
        &self,
        id: ObservationId,
        activity: ObservationActivity,
    ) -> Option<u8> {
        self.observations.award_total(id, activity)
    }

    pub fn observation_semantic_match(
        &self,
        id: ObservationId,
        activity: ObservationActivity,
    ) -> Option<u8> {
        self.observations.semantic_match(id, activity)
    }

    fn prepare_observation_scores(
        &self,
        record: &ObservationRecord,
        receipt: &ObservationBatchReceipt,
    ) -> Result<BTreeMap<ObservationId, FreshnessRecord>, OwnerSessionError> {
        prepare_scores(
            &self.archive,
            &self.common.memories,
            self.freshness.policy(),
            &self.observation_scores,
            record,
            receipt,
        )
    }
}

pub(super) fn restore_scores(
    archive: &Archive,
    memories: &MemoryStore,
    freshness: &FreshnessStore,
    observations: &ObservationStore,
) -> Result<BTreeMap<ObservationId, FreshnessRecord>, OwnerSessionError> {
    let mut scores = BTreeMap::new();
    for (receipt, record) in observations.records() {
        let updates = prepare_scores(
            archive,
            memories,
            freshness.policy(),
            &scores,
            record,
            receipt,
        )?;
        scores.extend(updates);
    }
    Ok(scores)
}

fn prepare_scores(
    archive: &Archive,
    memories: &MemoryStore,
    policy: reliquary_freshness_store::FreshnessPolicy,
    existing: &BTreeMap<ObservationId, FreshnessRecord>,
    record: &ObservationRecord,
    receipt: &ObservationBatchReceipt,
) -> Result<BTreeMap<ObservationId, FreshnessRecord>, OwnerSessionError> {
    let mut scores = BTreeMap::new();
    if record.archive_cut > archive.archive_version() || record.rel_turn > archive.rel_turn_count()
    {
        return Err(invalid("Freshness clock exceeds Archive"));
    }
    for operation in &record.batch.operations {
        match operation {
            ObservationOperation::SemanticAward {
                activity, evidence, ..
            } => {
                let (memory, _) = memories
                    .revision_projection(MemoryId(activity.source), Some(evidence.revision))?;
                let source = memory
                    .source_ref
                    .as_ref()
                    .ok_or_else(|| invalid("semantic award lacks source"))?;
                let conversation = source
                    .content_source_conversation_id
                    .as_deref()
                    .or(source.grounding_source_conversation_id.as_deref())
                    .ok_or_else(|| invalid("semantic award lacks accepted conversation"))?;
                let node = source
                    .content_source_node_id
                    .as_deref()
                    .unwrap_or(&source.source_node_id);
                if activity.turn > record.rel_turn
                    || archive.activity_position_for_turn(conversation, node) != Some(activity.turn)
                {
                    return Err(invalid(
                        "semantic award lacks originating accepted activity",
                    ));
                }
            }
            ObservationOperation::Successor {
                draft,
                credited_activity: Some(activity),
                ..
            } => {
                if !draft
                    .citations
                    .iter()
                    .chain(&draft.reviewed)
                    .any(|cite| cite.source == ObservationSource::Memory(activity.source))
                {
                    return Err(invalid("review award lacks credited evidence"));
                }
            }
            _ => {}
        }
    }
    for &(id, revision) in &receipt.revisions {
        if revision == 1 && !existing.contains_key(&id) {
            let mut score = FreshnessRecord::created();
            score
                .admit(record.rel_turn)
                .map_err(|_| invalid("Freshness enrollment"))?;
            scores.insert(id, score);
        }
    }
    for award in &receipt.awards {
        let score = scores.entry(award.id).or_insert(
            *existing
                .get(&award.id)
                .ok_or_else(|| invalid("semantic award target was never enrolled"))?,
        );
        score
            .reinforce_with_policy(
                i16::from(award.total - award.previous),
                record.rel_turn,
                &policy,
            )
            .map_err(|_| invalid("Freshness award"))?;
    }
    Ok(scores)
}

pub(super) fn admit_suffix(
    session: &mut RelSession,
    frame: &[u8],
    memory_cut: u64,
    unobserved_suffix_bodies: &BTreeSet<MemoryBodyId>,
) -> Result<bool, OwnerSessionError> {
    let Some((version, record)) = decode(frame)? else {
        return Ok(false);
    };
    let prefix = ObservationPrefix {
        memory_cut,
        unobserved_suffix_bodies: Some(unobserved_suffix_bodies),
        ..session.observation_prefix()
    };
    if record.archive_cut != session.archive.archive_version()
        || record.rel_turn != session.archive.rel_turn_count()
    {
        return Err(invalid("checkpoint suffix Archive cut differs"));
    }
    if record.claim_vectors.as_ref().is_some_and(|vectors| {
        session
            .active_compatibility_profile(CompatibilityProfileId(vectors.profile))
            .is_none()
    }) {
        return Err(invalid(
            "claim vector profile is inactive at its publication",
        ));
    }
    let receipt = prefix.check_record(&session.observations, &record, version)?;
    let scores = session.prepare_observation_scores(&record, &receipt)?;
    session.observation_scores.extend(scores);
    session.observations.apply(record, receipt);
    Ok(true)
}
