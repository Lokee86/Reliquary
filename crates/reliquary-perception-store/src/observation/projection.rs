use super::*;
use std::collections::BTreeMap;
use std::ops::Bound::{Excluded, Unbounded};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObservationRecord {
    pub batch: ObservationBatch,
    pub memory_cut: u64,
    pub perception_cut: u64,
    pub archive_cut: u64,
    pub rel_turn: u64,
    /// Derived once per operation and checked again against its publication prefix.
    pub lineages: Vec<Vec<ObservationMemoryLineage>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_vectors: Option<ObservationClaimVectors>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Pending {
    reason: ObservationPendingReason,
    evidence: BTreeSet<ObservationCitation>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
struct ClaimKey {
    proposition: String,
    participants: Vec<(EntityId, Option<String>)>,
    status: ObservationStatus,
    time_scope: ObservationTimeScope,
    validity: Option<ObservationValidity>,
}

impl ClaimKey {
    fn new(draft: &ObservationDraft) -> Self {
        let mut participants = draft
            .profile
            .participants()
            .iter()
            .map(|p| (p.entity_id, p.role.as_deref().map(normalize)))
            .collect::<Vec<_>>();
        participants.sort_unstable();
        Self {
            proposition: normalize(&draft.proposition),
            participants,
            status: draft.status,
            time_scope: draft.time_scope,
            validity: draft.validity,
        }
    }

    fn id(&self, batch: &ObservationBatch) -> ObservationId {
        let body = serde_json::to_vec(self).expect("Observation claim key is serializable");
        ObservationId(digest(
            b"reliquary-observation-id-v1",
            &[&batch.owner_id, &batch.history_generation, &body],
        ))
    }
}

fn normalize(value: &str) -> String {
    value
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

impl ObservationDraft {
    /// Identity used to coalesce simultaneous proposals for an equivalent claim.
    pub fn claim_digest(&self) -> [u8; 32] {
        let key = serde_json::to_vec(&ClaimKey::new(self))
            .expect("Observation claim key is serializable");
        digest(b"reliquary-observation-claim-v1", &[&key])
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct ObservationStore {
    records: Vec<(ObservationBatchReceipt, ObservationRecord)>,
    semantic_frontier: u64,
    receipts: BTreeMap<[u8; 32], usize>,
    revisions: BTreeMap<(ObservationId, u64), ObservationRevision>,
    heads: BTreeMap<ObservationId, u64>,
    pending: BTreeMap<ObservationId, Pending>,
    withdrawals: BTreeMap<ObservationId, ObservationWithdrawalReason>,
    claims: BTreeMap<ClaimKey, ObservationId>,
    used_claims: BTreeMap<ClaimKey, ObservationId>,
    source_dependents: BTreeMap<ObservationSource, BTreeSet<ObservationId>>,
    participant_dependents: BTreeMap<EntityId, BTreeSet<ObservationId>>,
    historical_dependencies: BTreeMap<ObservationId, BTreeSet<ObservationId>>,
    comparisons:
        BTreeMap<(ObservationId, u64, ObservationCitation, u32), ObservationComparisonOutcome>,
    awards: BTreeMap<(ObservationId, u64), u8>,
    semantic_matches: BTreeMap<(ObservationId, ObservationActivity), u8>,
    progress: Option<ObservationAuditProgress>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    claim_vectors: BTreeMap<[u8; 32], BTreeMap<ObservationCitation, (usize, usize)>>,
}

impl ObservationStore {
    pub fn frontier(&self) -> u64 {
        self.records
            .last()
            .map_or(0, |(receipt, _)| receipt.publication_version)
    }

    pub fn semantic_frontier(&self) -> u64 {
        self.semantic_frontier
    }

    pub fn records(&self) -> &[(ObservationBatchReceipt, ObservationRecord)] {
        &self.records
    }

    pub fn lookup_receipt(
        &self,
        batch: &ObservationBatch,
    ) -> Result<Option<&ObservationBatchReceipt>, PerceptionError> {
        let Some(&index) = self.receipts.get(&batch.input_fingerprint) else {
            return Ok(None);
        };
        let (receipt, accepted) = &self.records[index];
        if &accepted.batch != batch {
            return Err(invalid("Observation receipt input reused"));
        }
        Ok(Some(receipt))
    }

    pub fn progress(&self) -> Option<&ObservationAuditProgress> {
        self.progress.as_ref()
    }

    pub fn claim_vector_profiles(&self) -> impl Iterator<Item = [u8; 32]> + '_ {
        self.claim_vectors.keys().copied()
    }

    pub fn claim_vector(
        &self,
        profile: [u8; 32],
        citation: &ObservationCitation,
    ) -> Option<&[f32]> {
        let &(record, row) = self.claim_vectors.get(&profile)?.get(citation)?;
        Some(
            &self
                .records
                .get(record)?
                .1
                .claim_vectors
                .as_ref()?
                .rows
                .get(row)?
                .vector,
        )
    }

    pub fn head(&self, id: ObservationId) -> Option<&ObservationRevision> {
        self.revision(id, *self.heads.get(&id)?)
    }

    pub fn head_at(&self, id: ObservationId, version: u64) -> Option<&ObservationRevision> {
        self.revisions
            .range((id, 0)..=(id, u64::MAX))
            .rev()
            .map(|(_, row)| row)
            .find(|row| row.publication_version <= version)
    }

    pub fn revision(&self, id: ObservationId, revision: u64) -> Option<&ObservationRevision> {
        self.revisions.get(&(id, revision))
    }

    pub fn revision_page(
        &self,
        id: ObservationId,
        after: u64,
        limit: usize,
    ) -> impl Iterator<Item = &ObservationRevision> {
        self.revisions
            .range((Excluded((id, after)), Unbounded))
            .take_while(move |((item, _), _)| *item == id)
            .take(limit)
            .map(|(_, row)| row)
    }

    pub fn heads_after(
        &self,
        after: Option<ObservationId>,
        limit: usize,
    ) -> impl Iterator<Item = ObservationId> {
        self.heads
            .range((after.map_or(Unbounded, Excluded), Unbounded))
            .take(limit)
            .map(|(&id, _)| id)
    }

    pub fn source_dependents(
        &self,
        source: ObservationSource,
    ) -> impl Iterator<Item = ObservationId> {
        self.source_dependents
            .get(&source)
            .into_iter()
            .flatten()
            .copied()
    }

    pub fn participant_dependents(&self, entity: EntityId) -> impl Iterator<Item = ObservationId> {
        self.participant_dependents
            .get(&entity)
            .into_iter()
            .flatten()
            .copied()
    }

    pub fn matching_claim(&self, draft: &ObservationDraft) -> Option<ObservationId> {
        self.claims.get(&ClaimKey::new(draft)).copied()
    }

    pub fn withdrawal(&self, id: ObservationId) -> Option<ObservationWithdrawalReason> {
        self.withdrawals.get(&id).copied()
    }

    pub fn pending(
        &self,
        id: ObservationId,
    ) -> Option<(ObservationPendingReason, Vec<ObservationCitation>)> {
        self.pending
            .get(&id)
            .map(|pending| (pending.reason, pending.evidence.iter().copied().collect()))
    }

    pub fn comparison(
        &self,
        comparison: &ObservationComparison,
    ) -> Option<ObservationComparisonOutcome> {
        self.comparison_at_policy(comparison, 1)
    }

    pub fn comparison_at_policy(
        &self,
        comparison: &ObservationComparison,
        policy: u32,
    ) -> Option<ObservationComparisonOutcome> {
        self.comparisons
            .get(&comparison_key(comparison, policy))
            .copied()
    }

    pub fn explicitly_pending_after(
        &self,
        after: Option<ObservationId>,
        limit: usize,
    ) -> impl Iterator<Item = ObservationId> {
        self.pending
            .range((after.map_or(Unbounded, Excluded), Unbounded))
            .take(limit)
            .map(|(&id, _)| id)
    }

    pub fn source_dependents_after(
        &self,
        source: ObservationSource,
        after: Option<ObservationId>,
        limit: usize,
    ) -> impl Iterator<Item = ObservationId> {
        self.source_dependents
            .get(&source)
            .into_iter()
            .flat_map(move |ids| ids.range((after.map_or(Unbounded, Excluded), Unbounded)))
            .take(limit)
            .copied()
    }

    pub fn participant_dependents_after(
        &self,
        entity: EntityId,
        after: Option<ObservationId>,
        limit: usize,
    ) -> impl Iterator<Item = ObservationId> {
        self.participant_dependents
            .get(&entity)
            .into_iter()
            .flat_map(move |ids| ids.range((after.map_or(Unbounded, Excluded), Unbounded)))
            .take(limit)
            .copied()
    }

    pub fn requires_memory_body(&self, bodies: &BTreeSet<[u8; 32]>) -> bool {
        self.records.iter().any(|(_, record)| {
            record
                .batch
                .operations
                .iter()
                .flat_map(ObservationOperation::citations)
                .any(|cite| {
                    matches!(cite.source, ObservationSource::Memory(_))
                        && bodies.contains(&cite.body_id)
                })
        })
    }

    pub fn award_total(&self, id: ObservationId, activity: ObservationActivity) -> Option<u8> {
        self.awards.get(&(id, activity.turn)).copied()
    }

    pub fn semantic_match(&self, id: ObservationId, activity: ObservationActivity) -> Option<u8> {
        self.semantic_matches.get(&(id, activity)).copied()
    }

    /// Plan a bounded transaction without cloning the full owner projection.
    pub fn prepare(
        &self,
        record: &ObservationRecord,
        publication_version: u64,
        mut requires_review: impl FnMut(ObservationId) -> bool,
    ) -> Result<ObservationBatchReceipt, PerceptionError> {
        let batch = &record.batch;
        if batch.input_fingerprint == [0; 32]
            || batch.policy.version == 0
            || batch.policy.general_route == [0; 32]
            || publication_version <= self.frontier()
            || record.lineages.len() != batch.operations.len()
            || batch.operations.len() > MAX_OBSERVATION_OPERATIONS
            || (batch.operations.is_empty()
                && batch.progress.is_none()
                && record.claim_vectors.is_none())
            || self.receipts.contains_key(&batch.input_fingerprint)
        {
            return Err(invalid("Observation publication"));
        }
        if let Some(progress) = &batch.progress
            && (progress.previous != self.progress().map(|value| value.fingerprint)
                || progress.fingerprint != <[u8; 32]>::from(Sha256::digest(&progress.bytes))
                || progress.previous == Some(progress.fingerprint)
                || progress.bytes.is_empty()
                || progress.bytes.len() > 512 * 1024)
        {
            return Err(invalid("Observation audit progress predecessor or bounds"));
        }

        if let Some(vectors) = &record.claim_vectors
            && (!batch.operations.is_empty()
                || batch.progress.is_some()
                || vectors.profile == [0; 32]
                || vectors.rows.is_empty()
                || vectors.rows.len() > MAX_OBSERVATION_CLAIM_VECTOR_ROWS
                || vectors
                    .rows
                    .windows(2)
                    .any(|rows| rows[0].citation >= rows[1].citation)
                || vectors
                    .rows
                    .iter()
                    .any(|row| self.claim_vector(vectors.profile, &row.citation).is_some()))
        {
            return Err(invalid("Observation claim vector publication"));
        }

        let mut receipt = ObservationBatchReceipt {
            publication_version,
            input_fingerprint: batch.input_fingerprint,
            revisions: Vec::new(),
            awards: Vec::new(),
        };
        let mut lifecycles = BTreeSet::new();
        let mut claims = BTreeSet::new();
        let mut comparisons = BTreeSet::new();
        let mut awards = BTreeMap::new();
        for (operation, lineage) in batch.operations.iter().zip(&record.lineages) {
            match operation {
                ObservationOperation::Create { draft } => {
                    let claim = ClaimKey::new(draft);
                    let id = claim.id(batch);
                    self.check_draft(id, draft, batch, lineage)?;
                    if self.head(id).is_some() || !lifecycles.insert(id) || !claims.insert(claim) {
                        return Err(invalid("duplicate Observation claim"));
                    }
                    receipt.revisions.push((id, 1));
                }
                ObservationOperation::Pending {
                    id,
                    expected_revision,
                    evidence,
                    ..
                } => {
                    self.require_head(*id, *expected_revision)?;
                    if !lifecycles.insert(*id) || !valid_reviewed(evidence) {
                        return Err(invalid("Observation pending review"));
                    }
                    let merged = self
                        .pending
                        .get(id)
                        .into_iter()
                        .flat_map(|p| p.evidence.iter())
                        .chain(evidence)
                        .collect::<BTreeSet<_>>();
                    if merged.len() > MAX_REVIEWED {
                        return Err(invalid("Observation pending evidence limit"));
                    }
                }
                ObservationOperation::Reaffirm {
                    id,
                    expected_revision,
                    reviewed,
                } => {
                    self.require_head(*id, *expected_revision)?;
                    if !lifecycles.insert(*id)
                        || !valid_reviewed(reviewed)
                        || !self.pending.contains_key(id)
                        || !self.retains_pending_evidence(*id, reviewed.iter())
                    {
                        return Err(invalid("Observation reaffirmation"));
                    }
                    receipt.revisions.push((*id, *expected_revision));
                }
                ObservationOperation::Successor {
                    id,
                    expected_revision,
                    draft,
                    change,
                    credited_activity,
                } => {
                    let head = self.require_head(*id, *expected_revision)?;
                    self.check_draft(*id, draft, batch, lineage)?;
                    if !lifecycles.insert(*id)
                        || !claims.insert(ClaimKey::new(draft))
                        || !requires_review(*id)
                        || head.draft.same_verified_claim(draft)
                        || !self.retains_pending_evidence(
                            *id,
                            draft.citations.iter().chain(&draft.reviewed),
                        )
                    {
                        return Err(invalid("Observation successor"));
                    }
                    let revision = expected_revision
                        .checked_add(1)
                        .ok_or(PerceptionError::VersionExhausted)?;
                    receipt.revisions.push((*id, revision));
                    if let Some(activity) = credited_activity {
                        let total = match change {
                            ObservationChange::Qualification => 25,
                            ObservationChange::Correction => 50,
                            ObservationChange::Equivalent => {
                                return Err(invalid("Observation review award"));
                            }
                        };
                        if !self.semantic_matches.contains_key(&(*id, *activity)) {
                            return Err(invalid(
                                "Observation review activity missing semantic match",
                            ));
                        }
                        self.plan_award(&mut awards, *id, *activity, total);
                    }
                }
                ObservationOperation::Withdraw {
                    id,
                    expected_revision,
                    reviewed,
                    ..
                } => {
                    self.require_head(*id, *expected_revision)?;
                    if !lifecycles.insert(*id)
                        || !valid_reviewed(reviewed)
                        || !requires_review(*id)
                        || !self.retains_pending_evidence(*id, reviewed.iter())
                    {
                        return Err(invalid("Observation withdrawal"));
                    }
                    receipt.revisions.push((*id, *expected_revision));
                }
                ObservationOperation::Comparison { comparison } => {
                    self.require_head(comparison.id, comparison.target_revision)?;
                    if self
                        .comparison_at_policy(comparison, batch.policy.version)
                        .is_some()
                        || !comparisons.insert(comparison_key(comparison, batch.policy.version))
                    {
                        return Err(invalid("duplicate Observation comparison"));
                    }
                    if matches!(comparison.outcome, ObservationComparisonOutcome::MaterialChallenge | ObservationComparisonOutcome::MaterialQualification)
                        && !self.pending.get(&comparison.id).is_some_and(|p| p.evidence.contains(&comparison.citation))
                        && !batch.operations.iter().any(|operation| matches!(operation,
                            ObservationOperation::Pending { id, expected_revision, reason: ObservationPendingReason::NewEvidence, evidence }
                            if *id == comparison.id && *expected_revision == comparison.target_revision && evidence.contains(&comparison.citation)))
                    {
                        return Err(invalid("material comparison lacks pending marker and evidence"));
                    }
                }
                ObservationOperation::SemanticAward {
                    id,
                    expected_revision,
                    activity,
                    amount,
                    ..
                } => {
                    self.require_head(*id, *expected_revision)?;
                    if !(1..=10).contains(amount) {
                        return Err(invalid("Observation semantic award"));
                    }
                    self.plan_award(&mut awards, *id, *activity, *amount);
                }
            }
        }
        receipt.awards.extend(awards.into_values());
        Ok(receipt)
    }

    fn retains_pending_evidence<'a>(
        &self,
        id: ObservationId,
        supplied: impl Iterator<Item = &'a ObservationCitation>,
    ) -> bool {
        self.pending.get(&id).is_none_or(|pending| {
            let supplied = supplied.collect::<BTreeSet<_>>();
            pending
                .evidence
                .iter()
                .all(|citation| supplied.contains(citation))
        })
    }

    fn plan_award(
        &self,
        awards: &mut BTreeMap<(ObservationId, u64), ObservationAwardReceipt>,
        id: ObservationId,
        activity: ObservationActivity,
        total: u8,
    ) {
        let previous = self.award_total(id, activity).unwrap_or(0);
        if total <= previous {
            return;
        }
        let key = (id, activity.turn);
        if awards.get(&key).is_none_or(|old| {
            total > old.total || (total == old.total && activity.source < old.activity.source)
        }) {
            awards.insert(
                key,
                ObservationAwardReceipt {
                    id,
                    activity,
                    previous,
                    total,
                },
            );
        }
    }

    fn require_head(
        &self,
        id: ObservationId,
        expected: u64,
    ) -> Result<&ObservationRevision, PerceptionError> {
        self.head(id)
            .filter(|row| row.revision == expected && !self.withdrawals.contains_key(&id))
            .ok_or_else(|| invalid("Observation head changed"))
    }

    fn check_draft(
        &self,
        id: ObservationId,
        draft: &ObservationDraft,
        batch: &ObservationBatch,
        lineage: &[ObservationMemoryLineage],
    ) -> Result<(), PerceptionError> {
        draft.validate()?;
        let claim = ClaimKey::new(draft);
        if claim.proposition.is_empty()
            || batch.policy.decision_route == [0; 32]
            || lineage.is_empty()
            || lineage.len() > MAX_OBSERVATION_LINEAGE
            || lineage.windows(2).any(|rows| rows[0] >= rows[1])
            || self
                .used_claims
                .get(&claim)
                .is_some_and(|owner| *owner != id)
        {
            return Err(invalid("Observation draft binding"));
        }
        let mut dependencies = direct_observations(draft).collect::<Vec<_>>();
        let mut visited = BTreeSet::new();
        while let Some(source) = dependencies.pop() {
            if source == id {
                return Err(invalid("Observation cycle or dependency limit"));
            }
            if visited.insert(source) {
                if visited.len() > MAX_OBSERVATION_DEPENDENCIES {
                    return Err(invalid("Observation cycle or dependency limit"));
                }
                dependencies.extend(
                    self.historical_dependencies
                        .get(&source)
                        .into_iter()
                        .flatten(),
                );
            }
        }
        Ok(())
    }

    /// Install an already validated transaction after its native frame is durable.
    pub fn apply(&mut self, record: ObservationRecord, receipt: ObservationBatchReceipt) {
        if let Some(vectors) = &record.claim_vectors {
            let index = self.claim_vectors.entry(vectors.profile).or_default();
            for (row, vector) in vectors.rows.iter().enumerate() {
                index.insert(vector.citation, (self.records.len(), row));
            }
        }
        for (operation, lineage) in record.batch.operations.iter().zip(&record.lineages) {
            let batch = &record.batch;
            if matches!(
                operation,
                ObservationOperation::Create { .. }
                    | ObservationOperation::Pending { .. }
                    | ObservationOperation::Reaffirm { .. }
                    | ObservationOperation::Successor { .. }
                    | ObservationOperation::Withdraw { .. }
            ) {
                self.semantic_frontier = receipt.publication_version;
            }
            match operation {
                ObservationOperation::Create { draft } => {
                    self.publish_revision(
                        ClaimKey::new(draft).id(batch),
                        1,
                        draft,
                        lineage,
                        batch,
                        receipt.publication_version,
                    );
                }
                ObservationOperation::Pending {
                    id,
                    reason,
                    evidence,
                    ..
                } => {
                    let pending = self.pending.entry(*id).or_insert_with(|| Pending {
                        reason: *reason,
                        evidence: BTreeSet::new(),
                    });
                    pending.evidence.extend(evidence);
                }
                ObservationOperation::Reaffirm { id, .. } => {
                    self.pending.remove(id);
                }
                ObservationOperation::Successor {
                    id,
                    expected_revision,
                    draft,
                    ..
                } => {
                    self.remove_live_indexes(*id);
                    self.pending.remove(id);
                    self.publish_revision(
                        *id,
                        expected_revision + 1,
                        draft,
                        lineage,
                        batch,
                        receipt.publication_version,
                    );
                }
                ObservationOperation::Withdraw { id, reason, .. } => {
                    self.remove_live_indexes(*id);
                    self.pending.remove(id);
                    self.withdrawals.insert(*id, *reason);
                }
                ObservationOperation::Comparison { comparison } => {
                    self.comparisons.insert(
                        comparison_key(comparison, batch.policy.version),
                        comparison.outcome,
                    );
                }
                ObservationOperation::SemanticAward {
                    id,
                    activity,
                    amount,
                    ..
                } => {
                    let entry = self
                        .semantic_matches
                        .entry((*id, *activity))
                        .or_insert(*amount);
                    *entry = (*entry).max(*amount);
                }
            }
        }
        for award in &receipt.awards {
            self.awards
                .insert((award.id, award.activity.turn), award.total);
        }
        if let Some(progress) = &record.batch.progress {
            self.progress = Some(progress.clone());
        }
        self.receipts
            .insert(record.batch.input_fingerprint, self.records.len());
        self.records.push((receipt, record));
    }

    fn publish_revision(
        &mut self,
        id: ObservationId,
        revision: u64,
        draft: &ObservationDraft,
        lineage: &[ObservationMemoryLineage],
        batch: &ObservationBatch,
        publication_version: u64,
    ) {
        let key = ClaimKey::new(draft);
        self.claims.insert(key.clone(), id);
        self.used_claims.insert(key, id);
        for citation in &draft.citations {
            self.source_dependents
                .entry(citation.source)
                .or_default()
                .insert(id);
        }
        for participant in draft.profile.participants() {
            self.participant_dependents
                .entry(participant.entity_id)
                .or_default()
                .insert(id);
            self.source_dependents
                .entry(ObservationSource::Memory(participant.mention.key.memory_id))
                .or_default()
                .insert(id);
        }
        self.historical_dependencies
            .entry(id)
            .or_default()
            .extend(direct_observations(draft));
        self.heads.insert(id, revision);
        self.revisions.insert(
            (id, revision),
            ObservationRevision {
                owner_id: batch.owner_id,
                history_generation: batch.history_generation,
                id,
                revision,
                predecessor: revision.checked_sub(1).filter(|revision| *revision != 0),
                draft: draft.clone(),
                lineage: lineage.to_vec(),
                policy: batch.policy,
                input_fingerprint: batch.input_fingerprint,
                publication_version,
            },
        );
    }

    fn remove_live_indexes(&mut self, id: ObservationId) {
        let revision = self.heads[&id];
        let draft = &self.revisions[&(id, revision)].draft;
        self.claims.remove(&ClaimKey::new(draft));
        for citation in &draft.citations {
            remove_dependent(&mut self.source_dependents, citation.source, id);
        }
        for participant in draft.profile.participants() {
            remove_dependent(&mut self.participant_dependents, participant.entity_id, id);
            remove_dependent(
                &mut self.source_dependents,
                ObservationSource::Memory(participant.mention.key.memory_id),
                id,
            );
        }
    }
}

fn remove_dependent<K: Ord>(
    index: &mut BTreeMap<K, BTreeSet<ObservationId>>,
    key: K,
    id: ObservationId,
) {
    if let std::collections::btree_map::Entry::Occupied(mut entry) = index.entry(key) {
        entry.get_mut().remove(&id);
        if entry.get().is_empty() {
            entry.remove();
        }
    }
}

fn comparison_key(
    comparison: &ObservationComparison,
    policy: u32,
) -> (ObservationId, u64, ObservationCitation, u32) {
    (
        comparison.id,
        comparison.target_revision,
        comparison.citation,
        policy,
    )
}

fn direct_observations(draft: &ObservationDraft) -> impl Iterator<Item = ObservationId> {
    draft
        .citations
        .iter()
        .filter_map(|citation| match citation.source {
            ObservationSource::Observation(id) => Some(id),
            ObservationSource::Memory(_) => None,
        })
}
