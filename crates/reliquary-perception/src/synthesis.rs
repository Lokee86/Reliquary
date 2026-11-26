use crate::claim::{
    ClaimFields, MAX_CITATIONS, bind_claim, claim_schema, model_claim, model_participants,
    nonblank_bounded, normalize, object, participant_identity, valid_claim,
};
use crate::general::{complete, fingerprint};
use crate::{
    HitVerdict, ObservationClaim, ObservationProposal, RECEPTOR_POLICY_VERSION, StageError,
    StageLimits, VERIFICATION_POLICY_VERSION,
};
use reliquary_memory_audit::{Feature, OwnerCut, Participant, Passage, SourceCitation, SourceId};
use reliquary_switchboard::GeneralEndpoint;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const SYNTHESIS_POLICY_VERSION: u16 = 2;
const MAX_SOURCE_BYTES: usize = 24 * 1024;
const MAX_PARTICIPANT_SLOTS: usize = 16;
const MAX_EXISTING_SLOTS: usize = 16;
const MAX_LABEL_BYTES: usize = 512;
const MAX_BACKCHECK_SLOTS: usize = 3;
const PROMPT: &str = concat!(
    "Draft one exact Observation from the supplied authorized passages. Use the proposed statement as ",
    "a lead and preserve its profile. Determine the claim from the passages and include its supported ",
    "status, applicable time, conditions and scope in the exact wording. A plan or request supports ",
    "its own status. Require evidence for an observed outcome. Use direct evidence or a bounded ",
    "conclusion with citations for all its premises. ",
    "Retain uncertainty left by the sources. Do not guess about cause or generalize beyond the passages. ",
    "Choose only provided source and participant slots. For an ordinary Observation leave participants empty. ",
    "For a relationship select two to four distinct resolved participants and ground the relationship ",
    "and each role in the selected Memory passages. Use null for an unspecified role. ",
    "When revising, consider the disposition and text of each supplied back-check passage. Select a ",
    "passage as a citation only when it grounds the exact new wording. An existing Observation slot may ",
    "be selected when its claim is equivalent in meaning, status, applicable time, scope and participant roles. ",
    "Use null for duplicate_slot when no such existing claim is supplied. Use null for both fields to abstain.",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StageBinding {
    pub cut: OwnerCut,
    pub proposal_id: [u8; 32],
    pub route_identity: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ExistingObservationRef {
    pub owner_id: [u8; 16],
    pub history_generation: [u8; 32],
    pub id: [u8; 32],
    pub revision: u64,
    pub body_id: [u8; 32],
}

#[derive(Clone, Eq, PartialEq)]
pub struct ExistingObservation {
    pub target: ExistingObservationRef,
    pub claim: ObservationClaim,
}

#[derive(Clone, Eq, PartialEq)]
pub struct SynthesisDraft {
    pub id: [u8; 32],
    pub claim: ObservationClaim,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RevisionEvidence {
    pub citation: SourceCitation,
    pub disposition: HitVerdict,
}

#[derive(Clone, Eq, PartialEq)]
pub struct RevisionContext {
    pub prior: SynthesisDraft,
    pub backcheck: Vec<RevisionEvidence>,
}

pub struct SynthesisRequest<'a> {
    pub binding: StageBinding,
    pub proposal: &'a ObservationProposal,
    pub sources: &'a [Passage],
    pub participants: &'a [Participant],
    pub existing: &'a [ExistingObservation],
    pub revision: Option<&'a RevisionContext>,
}

#[derive(Clone, Eq, PartialEq)]
pub enum SynthesisDecision {
    Abstained,
    Draft(SynthesisDraft),
    Duplicate {
        target: ExistingObservationRef,
        draft: SynthesisDraft,
    },
}

/// Synthesize one exact claim or abstain. Application owns resynthesis budgets and authorization.
pub fn synthesize(
    endpoint: &dyn GeneralEndpoint,
    request: &SynthesisRequest<'_>,
    limits: StageLimits,
) -> Result<SynthesisDecision, StageError> {
    if !request.valid() {
        return Err(StageError::InvalidInput);
    }
    let state = request.state();
    let schema = object(
        json!({
            "draft": {"anyOf":[claim_schema(request.proposal.profile), {"type":"null"}]},
            "duplicate_slot": {"type":["integer","null"], "minimum":0, "maximum":MAX_EXISTING_SLOTS - 1},
        }),
        &["draft", "duplicate_slot"],
    );
    let response = complete(
        endpoint,
        PROMPT,
        "observation_synthesis_v2",
        &schema,
        &state,
        limits,
    )?;
    let fields: SynthesisFields =
        serde_json::from_value(response).map_err(|_| StageError::InvalidReply)?;
    let Some(claim) = fields.draft else {
        return if fields.duplicate_slot.is_none() {
            Ok(SynthesisDecision::Abstained)
        } else {
            Err(StageError::InvalidReply)
        };
    };
    let claim = bind_claim(
        claim,
        |slot| request.sources.get(usize::from(slot)),
        |slot| request.participants.get(usize::from(slot)),
    )
    .filter(|claim| claim.profile == request.proposal.profile)
    .ok_or(StageError::InvalidReply)?;
    if request
        .revision
        .is_some_and(|revision| unchanged(&revision.prior.claim, &claim))
    {
        return Err(StageError::InvalidReply);
    }
    let target = match fields.duplicate_slot {
        Some(slot) => {
            let known = request
                .existing
                .get(usize::from(slot))
                .filter(|known| compatible_claim(&known.claim, &claim))
                .ok_or(StageError::InvalidReply)?;
            Some(known.target)
        }
        None => request
            .existing
            .iter()
            .filter(|known| {
                compatible_claim(&known.claim, &claim)
                    && normalize(&known.claim.proposition) == normalize(&claim.proposition)
            })
            .min_by_key(|known| known.target.id)
            .map(|known| known.target),
    };
    let draft = SynthesisDraft {
        id: request.draft_identity(endpoint.model(), &state, &claim),
        claim,
    };
    Ok(match target {
        Some(target) => SynthesisDecision::Duplicate { target, draft },
        None => SynthesisDecision::Draft(draft),
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SynthesisFields {
    #[serde(deserialize_with = "Deserialize::deserialize")]
    draft: Option<ClaimFields>,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    duplicate_slot: Option<u16>,
}

impl SynthesisRequest<'_> {
    fn valid(&self) -> bool {
        let cut = self.binding.cut;
        let mut citations = BTreeSet::new();
        let mut source_revisions = BTreeMap::new();
        let mut participants = BTreeSet::new();
        let mut existing = BTreeSet::new();
        self.proposal.valid()
            && (self.revision.is_some() || !self.proposal.sources.is_empty())
            && (1..=MAX_CITATIONS).contains(&self.sources.len())
            && self
                .sources
                .iter()
                .map(|source| source.text.len())
                .sum::<usize>()
                <= MAX_SOURCE_BYTES
            && self.sources.iter().all(|source| {
                let citation = source.citation;
                let revision = (citation.revision, citation.body_id);
                citation.owner_id == cut.owner_id
                    && citation.history_generation == cut.history_generation
                    && citation.revision > 0
                    && citation.end_byte.checked_sub(citation.start_byte)
                        == u32::try_from(source.text.len()).ok()
                    && !source.text.trim().is_empty()
                    && source.allows(Feature::Perception)
                    && citations.insert(citation)
                    && *source_revisions.entry(citation.source).or_insert(revision) == revision
            })
            && self
                .proposal
                .sources
                .iter()
                .all(|citation| citations.contains(citation))
            && self.participants.len() <= MAX_PARTICIPANT_SLOTS
            && self.participants.iter().all(|p| {
                p.owner_id == cut.owner_id
                    && p.revision > 0
                    && nonblank_bounded(&p.label, MAX_LABEL_BYTES)
                    && participants.insert(p.entity_id)
            })
            && self
                .proposal
                .participants
                .iter()
                .all(|p| self.participants.contains(p))
            && self.existing.len() <= MAX_EXISTING_SLOTS
            && self.existing.iter().all(|known| {
                known.target.owner_id == cut.owner_id
                    && known.target.history_generation == cut.history_generation
                    && known.target.revision > 0
                    && valid_claim(&known.claim)
                    && known.claim.participants.iter().all(|p| {
                        p.participant.owner_id == cut.owner_id
                            && nonblank_bounded(&p.participant.label, MAX_LABEL_BYTES)
                    })
                    && existing.insert(known.target.id)
            })
            && self.revision.is_none_or(|revision| {
                let mut backcheck = BTreeSet::new();
                valid_claim(&revision.prior.claim)
                    && revision.prior.claim.profile == self.proposal.profile
                    && revision.backcheck.len() <= MAX_BACKCHECK_SLOTS
                    && revision.backcheck.iter().all(|hit| {
                        citations.contains(&hit.citation) && backcheck.insert(hit.citation)
                    })
            })
    }

    fn source_slot(&self, citation: SourceCitation) -> usize {
        self.sources
            .iter()
            .position(|row| row.citation == citation)
            .expect("validated source slot")
    }

    fn state(&self) -> Value {
        let sources: Vec<_> = self
            .sources
            .iter()
            .enumerate()
            .map(|(slot, row)| {
                json!({
                    "slot": slot, "text": row.text,
                    "source_kind": match row.citation.source {
                        SourceId::Memory(_) => "memory",
                        SourceId::Observation(_) => "observation",
                    },
                })
            })
            .collect();
        let participants: Vec<_> = self
            .participants
            .iter()
            .enumerate()
            .map(|(slot, p)| {
                json!({
                    "slot": slot, "label": p.label,
                })
            })
            .collect();
        let proposal_participants: Vec<_> = self
            .proposal
            .participants
            .iter()
            .map(|p| {
                self.participants
                    .iter()
                    .position(|entry| entry == p)
                    .expect("validated participant slot")
            })
            .collect();
        let existing: Vec<_> = self
            .existing
            .iter()
            .enumerate()
            .map(|(slot, known)| {
                json!({
                    "slot": slot, "claim": model_claim(&known.claim),
                    "participants": model_participants(&known.claim),
                })
            })
            .collect();
        let revision = self.revision.map(|revision| {
            let backcheck: Vec<_> = revision.backcheck.iter().map(|hit| json!({
                "source_slot": self.source_slot(hit.citation), "disposition": hit.disposition,
            })).collect();
            json!({
                "prior_claim": model_claim(&revision.prior.claim),
                "prior_participants": model_participants(&revision.prior.claim),
                "backcheck": backcheck,
            })
        });
        json!({
            "proposal": {"proposition": self.proposal.proposition, "profile": self.proposal.profile},
            "proposal_sources": self.proposal.sources.iter().map(|c| self.source_slot(*c)).collect::<Vec<_>>(),
            "proposal_participants": proposal_participants,
            "source_slots": sources,
            "participant_slots": participants,
            "existing_observations": existing,
            "revision": revision,
        })
    }

    fn draft_identity(&self, model: &str, state: &Value, result: &ObservationClaim) -> [u8; 32] {
        let sources: Vec<_> = self.sources.iter().map(|row| row.citation).collect();
        let participants: Vec<_> = self
            .participants
            .iter()
            .map(|p| (p.owner_id, p.entity_id, p.revision))
            .collect();
        let existing: Vec<_> = self.existing.iter().map(|row| row.target).collect();
        fingerprint(json!([
            "reliquary-observation-draft",
            SYNTHESIS_POLICY_VERSION,
            RECEPTOR_POLICY_VERSION,
            VERIFICATION_POLICY_VERSION,
            model,
            self.binding.cut.owner_id,
            self.binding.cut.history_generation,
            self.binding.proposal_id,
            self.binding.route_identity,
            self.revision.map(|revision| revision.prior.id),
            state,
            sources,
            participants,
            existing,
            model_claim(result),
            result.citations,
            participant_identity(result, true),
        ]))
    }
}

fn compatible_claim(left: &ObservationClaim, right: &ObservationClaim) -> bool {
    left.profile == right.profile
        && left.status == right.status
        && left.time_scope == right.time_scope
        && participant_identity(left, false) == participant_identity(right, false)
}

fn unchanged(prior: &ObservationClaim, draft: &ObservationClaim) -> bool {
    compatible_claim(prior, draft)
        && normalize(&prior.proposition) == normalize(&draft.proposition)
        && prior.citations.len() == draft.citations.len()
        && prior
            .citations
            .iter()
            .all(|citation| draft.citations.contains(citation))
}
