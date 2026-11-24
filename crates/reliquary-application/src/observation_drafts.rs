//! Read-only preparation of exact Observation work through the scoped owner boundary.

mod backcheck;
mod capture;
mod review;
#[cfg(test)]
mod tests;

use capture::{
    Capture, Excerpt, audit_citation, bind_participants, fit_context, observation_citation,
    participant,
};
use reliquary_memory_audit::{OwnerCut, Participant, Passage, Status, TimeScope};
use reliquary_owner_session::model::{CompatibilityProfileId, EntityId};
use reliquary_owner_session::observation::{
    ObservationCitation, ObservationDraft, ObservationId, ObservationOperation, ObservationPolicy,
    ObservationProfile, ObservationReceptor, ObservationReceptorKind, ObservationRevision,
    ObservationStanding, ObservationStatus, ObservationTimeScope, ObservationValidity,
};
use reliquary_owner_session::{OwnerSessionError, RelSession};
use reliquary_perception::{
    ClaimParticipant, EvidencePassage, ExistingObservation, ExistingObservationRef, HitVerdict,
    NativeDecisionError, ProposedClaim, ReceptorBinding, ReceptorCategory, RevisionContext,
    RevisionEvidence, StageBinding, StageError, StageLimits, SynthesisDecision, SynthesisDraft,
    SynthesisRequest, VERIFICATION_POLICY_VERSION, VerificationDecision, VerificationState,
    VerificationVerdict, generate_receptors, synthesize, verify,
};
use reliquary_switchboard::{DecisionEndpoint, EmbeddingEndpoint, EmbeddingMode, GeneralEndpoint};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ObservationWorkflowError as Error;
pub use review::evaluate_review;
type Result<T> = std::result::Result<T, Error>;

const MAX_DRAFT_SLICES: usize = 8;
const MAX_DRAFT_EVIDENCE_BYTES: usize = 24 * 1024;
const MAX_REVIEWED: usize = 11;
const MAX_EXISTING: usize = 16;
const MAX_EXISTING_BYTES: usize = 8 * 1024;

pub trait ScopedObservationRead {
    fn read<T>(&self, capture: impl FnOnce(&RelSession) -> Result<T>) -> Result<T>;
}

pub struct ObservationDraftRoutes<'a> {
    pub general: &'a dyn GeneralEndpoint,
    pub verification: &'a dyn DecisionEndpoint,
    pub embedding: &'a dyn EmbeddingEndpoint,
    pub route_identity: [u8; 32],
    pub embedding_profile: [u8; 32],
    pub memory_vector_profile: Option<CompatibilityProfileId>,
}

impl ObservationDraftRoutes<'_> {
    fn search_profile(&self, receptors: &[ObservationReceptor]) -> Option<CompatibilityProfileId> {
        self.memory_vector_profile.filter(|_| {
            receptors
                .iter()
                .all(|row| row.embedding_profile == self.embedding_profile)
        })
    }
}

#[derive(Debug)]
pub enum ObservationWorkflowError {
    Stale,
    PendingEvidence,
    EvidenceLimit,
    Embedding,
    InvalidDraft,
    Owner(OwnerSessionError),
    General(StageError),
    Native(NativeDecisionError),
}

impl fmt::Display for ObservationWorkflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Stale => "Observation capture changed",
            Self::PendingEvidence => "Observation evidence awaits an eligible source",
            Self::EvidenceLimit => "required Observation evidence exceeds the prepared batch",
            Self::Embedding => "Observation embedding route is unavailable or incompatible",
            Self::InvalidDraft => "Observation draft could not be validated",
            Self::Owner(_) => "Observation capture could not read owner evidence",
            Self::General(_) => "Observation structured inference could not complete",
            Self::Native(_) => "Observation native verification could not complete",
        })
    }
}

impl std::error::Error for ObservationWorkflowError {}

pub enum ProposalEvaluation {
    Abstained,
    Unresolved,
    Ready(PreparedObservation),
    Duplicate {
        target: ExistingObservationRef,
        evidence: Vec<ObservationCitation>,
    },
}

pub struct PreparedObservation {
    pub operation: ObservationOperation,
    pub input_fingerprint: [u8; 32],
    pub policy: ObservationPolicy,
    owner_id: [u8; 16],
    history_generation: [u8; 32],
    must_be_current: Vec<ObservationCitation>,
    embedding_profile: [u8; 32],
    memory_vector_profile: Option<CompatibilityProfileId>,
    review: Option<review::ReviewFence>,
}

impl PreparedObservation {
    /// Recheck the relevant owner cut under the writer before grouped publication.
    pub fn revalidate(&self, rel: &RelSession, route_identity: [u8; 32]) -> Result<()> {
        let mut capture = Capture::new(rel);
        if self.policy.general_route != route_identity
            || self.owner_id != capture.cut.owner_id
            || self.history_generation != capture.cut.history_generation
            || self
                .memory_vector_profile
                .is_some_and(|id| rel.active_compatibility_profile(id).is_none())
        {
            return Err(Error::Stale);
        }
        for &cite in &self.must_be_current {
            capture.citation(cite)?;
        }
        if let Some(fence) = &self.review {
            fence.revalidate(&mut capture)?;
        }
        if let ObservationOperation::Create { draft }
        | ObservationOperation::Successor { draft, .. } = &self.operation
        {
            let anchors = capture.anchors(&draft.citations)?;
            if draft.profile.participants().iter().any(|p| {
                !anchors.get(&p.entity_id).is_some_and(|values| {
                    values.iter().any(|entry| {
                        entry.entity_revision == p.revision
                            && entry.mention == p.mention.key
                            && entry.mention_revision == p.mention.revision
                            && entry.retained_ordinal == p.mention.retained_ordinal
                    })
                })
            }) || draft
                .receptors
                .iter()
                .any(|row| row.embedding_profile != self.embedding_profile)
            {
                return Err(Error::Stale);
            }
        }
        Ok(())
    }
}

struct SynthesisInput {
    binding: StageBinding,
    proposal: ProposedClaim,
    source_slots: Vec<Passage>,
    participants: Vec<Participant>,
    existing: Vec<ExistingObservation>,
}

impl SynthesisInput {
    fn run(
        &self,
        routes: &ObservationDraftRoutes<'_>,
        revision: Option<&RevisionContext>,
    ) -> Result<SynthesisDecision> {
        synthesize(
            routes.general,
            &SynthesisRequest {
                binding: self.binding,
                proposal: &self.proposal,
                sources: &self.source_slots,
                participants: &self.participants,
                existing: &self.existing,
                revision,
            },
            StageLimits::default(),
        )
        .map_err(Error::General)
    }

    fn add(&mut self, row: Passage) -> Result<()> {
        if self
            .source_slots
            .iter()
            .any(|prior| prior.citation == row.citation)
        {
            return Ok(());
        }
        if self.source_slots.len() == MAX_DRAFT_SLICES
            || self
                .source_slots
                .iter()
                .map(|row| row.text.len())
                .sum::<usize>()
                + row.text.len()
                > MAX_DRAFT_EVIDENCE_BYTES
        {
            return Err(Error::EvidenceLimit);
        }
        self.source_slots.push(row);
        Ok(())
    }
}

pub fn evaluate_proposal(
    read: &impl ScopedObservationRead,
    routes: &ObservationDraftRoutes<'_>,
    cut: OwnerCut,
    proposal: &ProposedClaim,
) -> Result<ProposalEvaluation> {
    let mut input = read.read(|rel| proposal_input(rel, routes, cut, proposal))?;
    let mut draft = match input.run(routes, None)? {
        SynthesisDecision::Abstained => return Ok(ProposalEvaluation::Abstained),
        SynthesisDecision::Duplicate { target, draft } => return Ok(duplicate(target, &draft)),
        SynthesisDecision::Draft(draft) => draft,
    };
    let mut required = Vec::new();
    let mut previous: Option<(SynthesisDraft, Vec<ObservationReceptor>)> = None;
    for attempt in 0..2 {
        let mut canonical =
            read.read(|rel| canonical_claim(&mut Capture::new(rel), &draft.claim))?;
        if let Some(target) = read.read(|rel| {
            Ok(rel
                .matching_observation(&canonical)
                .and_then(|id| rel.observation(id))
                .filter(|row| {
                    !matches!(
                        row.standing,
                        ObservationStanding::Withdrawn(_) | ObservationStanding::Superseded
                    )
                })
                .map(|row| existing_ref(row.revision)))
        })? {
            return Ok(duplicate(target, &draft));
        }
        canonical.receptors = receptors(
            routes,
            &draft,
            previous
                .as_ref()
                .map(|(draft, rows)| (draft, rows.as_slice(), routes.route_identity)),
        )?;
        let frozen =
            read.read(|rel| freeze(&mut Capture::new(rel), canonical, &required, routes))?;
        let decision = verify(routes.verification, &frozen.state).map_err(Error::Native)?;
        match decision.verdict {
            VerificationVerdict::StandsAsWritten => {
                return finish_create(&input, draft.id, frozen, routes);
            }
            VerificationVerdict::NeedsRevision if attempt == 0 => {
                required = material_refs(&frozen, &decision);
                previous = Some((draft.clone(), frozen.draft.receptors.clone()));
                let revision = revision_input(read, &mut input, draft, &frozen, &decision)?;
                draft = match input.run(routes, Some(&revision))? {
                    SynthesisDecision::Draft(draft) => draft,
                    SynthesisDecision::Duplicate { target, draft } => {
                        return Ok(duplicate(target, &draft));
                    }
                    SynthesisDecision::Abstained => return Ok(ProposalEvaluation::Abstained),
                };
            }
            _ => return Ok(ProposalEvaluation::Unresolved),
        }
    }
    unreachable!("a bounded verification attempt yields a disposition")
}

fn duplicate(target: ExistingObservationRef, draft: &SynthesisDraft) -> ProposalEvaluation {
    ProposalEvaluation::Duplicate {
        target,
        evidence: draft
            .claim
            .citations
            .iter()
            .copied()
            .map(observation_citation)
            .collect(),
    }
}

fn proposal_input(
    rel: &RelSession,
    routes: &ObservationDraftRoutes<'_>,
    cut: OwnerCut,
    proposal: &ProposedClaim,
) -> Result<SynthesisInput> {
    let mut capture = Capture::new(rel);
    if capture.cut.owner_id != cut.owner_id
        || capture.cut.history_generation != cut.history_generation
        || cut.frontier > capture.cut.memory
    {
        return Err(Error::Stale);
    }
    let citations: Vec<_> = proposal
        .citations
        .iter()
        .copied()
        .map(observation_citation)
        .collect();
    if !proposal.participants.is_empty() {
        capture.check_relational_sources(&citations)?;
        bind_participants(&proposal.participants, &capture.anchors(&citations)?)?;
    }
    let mut remaining = MAX_EXISTING_BYTES;
    let mut input = SynthesisInput {
        binding: StageBinding {
            cut,
            proposal_id: digest(json!([
                cut.owner_id,
                cut.history_generation,
                proposal.proposition,
                proposal.profile,
                proposal.status,
                proposal.time_scope,
                citations,
                proposal
                    .participants
                    .iter()
                    .map(|p| (p.participant.entity_id, p.participant.revision, &p.role))
                    .collect::<Vec<_>>()
            ])),
            route_identity: routes.route_identity,
        },
        proposal: proposal.clone(),
        source_slots: Vec::new(),
        participants: proposal
            .participants
            .iter()
            .map(|row| row.participant.clone())
            .collect(),
        existing: rel
            .observations_after(None, MAX_EXISTING)
            .into_iter()
            .filter(|row| row.standing == ObservationStanding::Current)
            .filter_map(|row| {
                let claim = review::semantic_claim(rel, row.revision);
                if claim.profile != proposal.profile {
                    return None;
                }
                let bytes = claim.proposition.len()
                    + claim
                        .participants
                        .iter()
                        .map(|p| p.participant.label.len() + p.role.as_ref().map_or(0, String::len))
                        .sum::<usize>();
                remaining = remaining.checked_sub(bytes)?;
                Some(ExistingObservation {
                    target: existing_ref(row.revision),
                    claim,
                })
            })
            .collect(),
    };
    for cite in citations {
        let (row, grade) = capture.citation(cite)?;
        input.add(row.audit(grade))?;
    }
    Ok(input)
}

fn existing_ref(row: &ObservationRevision) -> ExistingObservationRef {
    ExistingObservationRef {
        owner_id: row.owner_id,
        history_generation: row.history_generation,
        id: row.id.0,
        revision: row.revision,
        body_id: row.draft.body_id(),
    }
}

fn canonical_claim(capture: &mut Capture<'_>, claim: &ProposedClaim) -> Result<ObservationDraft> {
    let citations: Vec<_> = claim
        .citations
        .iter()
        .copied()
        .map(observation_citation)
        .collect();
    let profile = match claim.profile {
        reliquary_perception::ObservationProfile::Ordinary => ObservationProfile::Ordinary,
        reliquary_perception::ObservationProfile::Relational => {
            capture.check_relational_sources(&citations)?;
            ObservationProfile::Relational(bind_participants(
                &claim.participants,
                &capture.anchors(&citations)?,
            )?)
        }
    };
    let validity = grounded_validity(capture, &claim.proposition, &citations)?;
    Ok(ObservationDraft {
        proposition: claim.proposition.clone(),
        profile,
        status: match claim.status {
            Status::Observed => ObservationStatus::Observed,
            Status::Planned => ObservationStatus::Planned,
            Status::Requested => ObservationStatus::Requested,
            Status::Possible => ObservationStatus::Possible,
            Status::Expected => ObservationStatus::Expected,
        },
        time_scope: match claim.time_scope {
            TimeScope::Past => ObservationTimeScope::Past,
            TimeScope::Current => ObservationTimeScope::Current,
            TimeScope::Future => ObservationTimeScope::Future,
            TimeScope::Unspecified => ObservationTimeScope::Unspecified,
        },
        validity,
        citations,
        reviewed: Vec::new(),
        receptors: Vec::new(),
    })
}

fn grounded_validity(
    capture: &mut Capture<'_>,
    claim: &str,
    citations: &[ObservationCitation],
) -> Result<Option<ObservationValidity>> {
    use reliquary_chronos::{TemporalOrigin, analyze};
    let interpretation = analyze(claim, None);
    let [scope] = interpretation.intervals.as_slice() else {
        return Ok(None);
    };
    // Only explicit bounds attached to the exact wording can establish validity.
    if scope.origin != TemporalOrigin::Explicit
        || (scope.start_ns.is_none() && scope.end_ns.is_none())
    {
        return Ok(None);
    }
    for &cite in citations {
        let source = capture.citation(cite)?.0;
        if analyze(&source.text, None).intervals.iter().any(|other| {
            other.origin == TemporalOrigin::Explicit
                && other.start_ns == scope.start_ns
                && other.end_ns == scope.end_ns
        }) {
            return Ok(Some(ObservationValidity {
                start_ns: scope.start_ns,
                end_ns: scope.end_ns,
            }));
        }
    }
    Ok(None)
}

fn receptors(
    routes: &ObservationDraftRoutes<'_>,
    draft: &SynthesisDraft,
    prior: Option<(&SynthesisDraft, &[ObservationReceptor], [u8; 32])>,
) -> Result<Vec<ObservationReceptor>> {
    if let Some((before, rows, prior_route)) = prior {
        let equivalent = before.claim.proposition == draft.claim.proposition
            && before.claim.profile == draft.claim.profile
            && before.claim.status == draft.claim.status
            && before.claim.time_scope == draft.claim.time_scope
            && before.claim.participants == draft.claim.participants;
        if equivalent
            && prior_route == routes.route_identity
            && rows
                .iter()
                .all(|r| r.embedding_profile == routes.embedding_profile)
        {
            return Ok(rows.to_vec());
        }
    }
    if routes.embedding_profile == [0; 32] || !routes.embedding.supports_bounded_embedding() {
        return Err(Error::Embedding);
    }
    let set = generate_receptors(
        routes.general,
        draft,
        ReceptorBinding {
            route_identity: routes.route_identity,
            embedding_profile: routes.embedding_profile,
        },
        StageLimits::default(),
    )
    .map_err(Error::General)?;
    let inputs: Vec<_> = set
        .receptors
        .iter()
        .map(|row| row.description.clone())
        .collect();
    let rows = routes
        .embedding
        .embed_bounded(
            EmbeddingMode::Query,
            &inputs,
            crate::vector_workflow::MAX_EMBEDDING_RESPONSE_BYTES,
        )
        .map_err(|_| Error::Embedding)?;
    if rows.len() != inputs.len()
        || rows.iter().any(|row| {
            row.len() != routes.embedding.dimensions() as usize
                || row.iter().any(|v| !v.is_finite())
                || row.iter().all(|v| *v == 0.0)
        })
    {
        return Err(Error::Embedding);
    }
    Ok(set
        .receptors
        .into_iter()
        .zip(rows)
        .map(|(row, vector)| ObservationReceptor {
            kind: match row.category {
                ReceptorCategory::Support => ObservationReceptorKind::Support,
                ReceptorCategory::Challenge => ObservationReceptorKind::Challenge,
                ReceptorCategory::Qualification => ObservationReceptorKind::Qualification,
                ReceptorCategory::Other => ObservationReceptorKind::Change,
            },
            description: row.description,
            embedding_profile: routes.embedding_profile,
            vector,
        })
        .collect())
}

struct FrozenDraft {
    draft: ObservationDraft,
    state: VerificationState,
    hits: Vec<Excerpt>,
    current: Vec<ObservationCitation>,
}

fn freeze(
    capture: &mut Capture<'_>,
    mut draft: ObservationDraft,
    required: &[ObservationCitation],
    routes: &ObservationDraftRoutes<'_>,
) -> Result<FrozenDraft> {
    let mut cites = Vec::new();
    let mut seen = BTreeSet::new();
    for &cite in &draft.citations {
        cites.extend(
            capture
                .proof(cite)?
                .into_iter()
                .filter(|row| seen.insert(row.citation)),
        );
    }
    if cites.is_empty() || cites.len() > MAX_DRAFT_SLICES {
        return Err(Error::EvidenceLimit);
    }
    fit_context(&mut cites, MAX_DRAFT_EVIDENCE_BYTES)?;
    let mut hits = Vec::new();
    for &cite in required {
        hits.extend(
            capture
                .proof(cite)?
                .into_iter()
                .filter(|row| seen.insert(row.citation)),
        );
    }
    if hits.len() > backcheck::MAX_HITS {
        return Err(Error::EvidenceLimit);
    }
    fit_context(&mut hits, backcheck::MAX_HIT_BYTES)?;
    hits = backcheck::optional_hits(
        capture,
        &draft.proposition,
        &draft.receptors,
        routes.search_profile(&draft.receptors),
        &cites,
        hits,
    )?;
    draft.reviewed = reviewed(&[
        required,
        &hits.iter().map(|row| row.citation).collect::<Vec<_>>(),
    ])?;
    draft.validate().map_err(|_| Error::InvalidDraft)?;
    let mut current = draft.citations.clone();
    current.extend(required.iter().copied());
    current.extend(cites.iter().chain(&hits).map(|row| row.citation));
    current.sort_unstable();
    current.dedup();
    let state = verify_state(&draft.proposition, &cites, &hits);
    Ok(FrozenDraft {
        draft,
        state,
        hits,
        current,
    })
}

fn verify_state(claim: &str, cites: &[Excerpt], hits: &[Excerpt]) -> VerificationState {
    let mut lineages = BTreeMap::new();
    let mut encode = |prefix: &str, rows: &[Excerpt]| {
        rows.iter()
            .enumerate()
            .map(|(index, row)| {
                let count = lineages.len();
                let source_id = lineages
                    .entry(row.lineage)
                    .or_insert_with(|| format!("source-{count}"));
                EvidencePassage {
                    id: format!("{prefix}-{index}"),
                    source_id: source_id.clone(),
                    text: row.text.clone(),
                    before: row.before.clone(),
                    after: row.after.clone(),
                }
            })
            .collect()
    };
    VerificationState {
        claim: claim.to_owned(),
        citations: encode("citation", cites),
        backcheck_hits: encode("backcheck", hits),
    }
}

fn revision_input(
    read: &impl ScopedObservationRead,
    input: &mut SynthesisInput,
    draft: SynthesisDraft,
    frozen: &FrozenDraft,
    decision: &VerificationDecision,
) -> Result<RevisionContext> {
    let backcheck = read.read(|rel| {
        let mut capture = Capture::new(rel);
        frozen
            .hits
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let (source, grade) = capture.citation(row.citation)?;
                input.add(source.audit(grade))?;
                Ok(RevisionEvidence {
                    citation: audit_citation(row.citation),
                    disposition: decision.hits[&format!("backcheck-{index}")],
                })
            })
            .collect::<Result<Vec<_>>>()
    })?;
    Ok(RevisionContext {
        prior: draft,
        backcheck,
    })
}

fn material_refs(
    frozen: &FrozenDraft,
    decision: &VerificationDecision,
) -> Vec<ObservationCitation> {
    frozen
        .hits
        .iter()
        .enumerate()
        .filter_map(|(index, hit)| {
            matches!(
                decision.hits[&format!("backcheck-{index}")],
                HitVerdict::MaterialChallenge | HitVerdict::MaterialQualification
            )
            .then_some(hit.citation)
        })
        .collect()
}

fn finish_create(
    input: &SynthesisInput,
    draft_id: [u8; 32],
    frozen: FrozenDraft,
    routes: &ObservationDraftRoutes<'_>,
) -> Result<ProposalEvaluation> {
    let mut current = frozen.current;
    current.extend(
        input
            .source_slots
            .iter()
            .map(|source| observation_citation(source.citation)),
    );
    let cut = input.binding.cut;
    let operation = ObservationOperation::Create {
        draft: frozen.draft,
    };
    Ok(ProposalEvaluation::Ready(prepared(
        routes, cut, operation, draft_id, current, None,
    )))
}

fn prepared(
    routes: &ObservationDraftRoutes<'_>,
    cut: OwnerCut,
    operation: ObservationOperation,
    draft_id: [u8; 32],
    mut current: Vec<ObservationCitation>,
    review: Option<review::ReviewFence>,
) -> PreparedObservation {
    current.sort_unstable();
    current.dedup();
    let policy = ObservationPolicy {
        version: u32::from(VERIFICATION_POLICY_VERSION),
        general_route: routes.route_identity,
        decision_route: routes.route_identity,
    };
    let input_fingerprint = digest(json!([
        "reliquary-observation-prepared-v1",
        cut.owner_id,
        cut.history_generation,
        routes.embedding_profile,
        routes.memory_vector_profile,
        policy,
        draft_id,
        current,
        &operation,
    ]));
    PreparedObservation {
        operation,
        input_fingerprint,
        policy,
        owner_id: cut.owner_id,
        history_generation: cut.history_generation,
        must_be_current: current,
        embedding_profile: routes.embedding_profile,
        memory_vector_profile: routes.memory_vector_profile,
        review,
    }
}

fn reviewed(groups: &[&[ObservationCitation]]) -> Result<Vec<ObservationCitation>> {
    let mut result = BTreeSet::new();
    for group in groups {
        result.extend(group.iter().copied());
    }
    if result.len() > MAX_REVIEWED {
        return Err(Error::EvidenceLimit);
    }
    Ok(result.into_iter().collect())
}

fn digest(value: Value) -> [u8; 32] {
    Sha256::digest(value.to_string().as_bytes()).into()
}
