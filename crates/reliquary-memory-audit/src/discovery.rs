use crate::{
    AuditError, Batch, CandidateContract, Eligibility, Feature, MAX_CANDIDATES_PER_LIST,
    MAX_LOOKUPS, MAX_OPTIONAL_PASSAGE_BYTES, MAX_PASSAGES_PER_LOOKUP, OwnerCut, Passage, Slots,
    SourceCitation, WorkItem,
    batch::{MAX_QUERY_BYTES, RequestContract, SCHEMA_NAME, encode_request},
};
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::io::{self, Write};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LookupError {
    Unavailable,
    Invalidated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListFailure {
    Malformed,
    Lookup(LookupError),
    FollowUp(AuditError),
}

pub enum ListResult<T> {
    Inactive,
    Incomplete(ListFailure),
    Accepted {
        candidates: Vec<T>,
        exposure: Vec<SourceCitation>,
        rejected: usize,
    },
}

pub struct BatchResult<E, P> {
    pub ego: ListResult<E>,
    pub perception: ListResult<P>,
}

/// The callback must verify the grant and the returned body identities at the captured cut.
/// Pass only current, authorized lexical passages with their requesting-list eligibility.
pub fn run_batch<E, P, L>(
    batch: &Batch,
    endpoint: &dyn GeneralEndpoint,
    ego: &E,
    perception: &P,
    lookup: &mut L,
) -> Result<BatchResult<E::Candidate, P::Candidate>, AuditError>
where
    E: CandidateContract,
    P: CandidateContract,
    L: FnMut(&OwnerCut, Feature, &str) -> Result<Vec<Passage>, LookupError> + ?Sized,
{
    if *batch.contract != RequestContract::new(ego, perception) {
        return Err(AuditError::ContractChanged);
    }
    let active = batch.active_features();
    let mut response = dispatch(batch, endpoint, &batch.sources, active, false)?;
    let mut ego_reply = ListReply::extract(&mut response, Feature::Ego, active);
    let mut perception_reply = ListReply::extract(&mut response, Feature::Perception, active);
    if ego_reply.wants_lookup() || perception_reply.wants_lookup() {
        let mut sources = batch.sources.clone();
        let mut budget = LookupBudget::new(batch);
        // Give each requesting list one turn before using any remaining search slot.
        for index in 0..MAX_LOOKUPS {
            budget.search(&mut sources, &mut ego_reply, Feature::Ego, index, lookup);
            budget.search(
                &mut sources,
                &mut perception_reply,
                Feature::Perception,
                index,
                lookup,
            );
        }
        let follow_up = Eligibility {
            ego: ego_reply.wants_lookup(),
            perception: perception_reply.wants_lookup(),
        };
        if follow_up != Eligibility::NONE {
            match dispatch(batch, endpoint, &sources, follow_up, true) {
                Ok(mut response) => {
                    ego_reply.update(&mut response, Feature::Ego, follow_up);
                    perception_reply.update(&mut response, Feature::Perception, follow_up);
                }
                Err(error) => {
                    ego_reply.fail_follow_up(error);
                    perception_reply.fail_follow_up(error);
                }
            }
        }
        return Ok(results(
            batch,
            &sources,
            ego_reply,
            perception_reply,
            ego,
            perception,
        ));
    }
    Ok(results(
        batch,
        &batch.sources,
        ego_reply,
        perception_reply,
        ego,
        perception,
    ))
}

fn results<E: CandidateContract, P: CandidateContract>(
    batch: &Batch,
    sources: &[WorkItem],
    ego_reply: ListReply,
    perception_reply: ListReply,
    ego: &E,
    perception: &P,
) -> BatchResult<E::Candidate, P::Candidate> {
    BatchResult {
        ego: ego_reply.finish(batch, sources, Feature::Ego, ego),
        perception: perception_reply.finish(batch, sources, Feature::Perception, perception),
    }
}

fn dispatch(
    batch: &Batch,
    endpoint: &dyn GeneralEndpoint,
    sources: &[WorkItem],
    active: Eligibility,
    follow_up: bool,
) -> Result<Map<String, Value>, AuditError> {
    if !batch.fits(sources, active) {
        return Err(AuditError::InputLimit);
    }
    let packet = encode_request(sources, &batch.participants, active, follow_up);
    let response = endpoint
        .complete_json(
            &batch.contract.prompt,
            &packet,
            SCHEMA_NAME,
            &batch.contract.schema,
        )
        .map_err(|error| match error {
            GeneralEndpointError::InvalidResponse(_) => AuditError::UnreadableResponse,
            GeneralEndpointError::Backpressure { retry_after, .. } => {
                AuditError::Endpoint { retry_after }
            }
            _ => AuditError::Endpoint { retry_after: None },
        })?;
    serde_json::to_writer(ResponseLimit(batch.limits.max_response_bytes), &response)
        .map_err(|_| AuditError::UnreadableResponse)?;
    match response {
        Value::Object(map) => Ok(map),
        _ => Err(AuditError::UnreadableResponse),
    }
}

struct ResponseLimit(usize);

impl Write for ResponseLimit {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self
            .0
            .checked_sub(bytes.len())
            .ok_or_else(|| io::Error::other("audit response limit"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplyFields {
    candidates: Vec<Value>,
    lookups: Vec<String>,
}

enum ListReply {
    Inactive,
    Failed(ListFailure),
    Present(ReplyFields),
}

impl ListReply {
    fn extract(response: &mut Map<String, Value>, feature: Feature, active: Eligibility) -> Self {
        if !active.allows(feature) {
            return Self::Inactive;
        }
        let key = match feature {
            Feature::Ego => "ego",
            Feature::Perception => "perception",
        };
        let fields = response
            .remove(key)
            .and_then(|value| serde_json::from_value::<ReplyFields>(value).ok());
        match fields {
            Some(fields)
                if fields.candidates.len() <= MAX_CANDIDATES_PER_LIST
                    && fields.lookups.len() <= MAX_LOOKUPS
                    && fields
                        .lookups
                        .iter()
                        .all(|term| term.len() <= MAX_QUERY_BYTES && !term.trim().is_empty()) =>
            {
                Self::Present(fields)
            }
            _ => Self::Failed(ListFailure::Malformed),
        }
    }

    fn wants_lookup(&self) -> bool {
        matches!(self, Self::Present(fields) if !fields.lookups.is_empty())
    }

    fn fail_follow_up(&mut self, error: AuditError) {
        if self.wants_lookup() {
            *self = Self::Failed(ListFailure::FollowUp(error));
        }
    }

    fn update(&mut self, response: &mut Map<String, Value>, feature: Feature, active: Eligibility) {
        if self.wants_lookup() {
            *self = Self::extract(response, feature, active);
            if self.wants_lookup() {
                *self = Self::Failed(ListFailure::Malformed);
            }
        }
    }

    fn finish<C: CandidateContract>(
        self,
        batch: &Batch,
        sources: &[WorkItem],
        feature: Feature,
        contract: &C,
    ) -> ListResult<C::Candidate> {
        match self {
            Self::Inactive => ListResult::Inactive,
            Self::Failed(reason) => ListResult::Incomplete(reason),
            Self::Present(fields) => {
                let slots = Slots {
                    sources,
                    participants: &batch.participants,
                    feature,
                };
                let count = fields.candidates.len();
                let candidates: Vec<_> = fields
                    .candidates
                    .into_iter()
                    .filter_map(|value| contract.parse_candidate(value, &slots))
                    .collect();
                ListResult::Accepted {
                    rejected: count - candidates.len(),
                    candidates,
                    exposure: batch.mandatory_exposure(feature).collect(),
                }
            }
        }
    }
}

struct LookupBudget<'a> {
    batch: &'a Batch,
    searches: usize,
    passage_bytes: usize,
}

impl<'a> LookupBudget<'a> {
    fn new(batch: &'a Batch) -> Self {
        Self {
            batch,
            searches: MAX_LOOKUPS,
            passage_bytes: MAX_OPTIONAL_PASSAGE_BYTES,
        }
    }

    fn search<L>(
        &mut self,
        sources: &mut Vec<WorkItem>,
        reply: &mut ListReply,
        feature: Feature,
        index: usize,
        lookup: &mut L,
    ) where
        L: FnMut(&OwnerCut, Feature, &str) -> Result<Vec<Passage>, LookupError> + ?Sized,
    {
        if self.searches == 0 || self.passage_bytes == 0 {
            return;
        }
        let ListReply::Present(fields) = reply else {
            return;
        };
        let Some(query) = fields.lookups.get(index) else {
            return;
        };
        if fields.lookups[..index]
            .iter()
            .any(|prior| prior.trim() == query.trim())
        {
            return;
        }
        self.searches -= 1;
        let passages = match lookup(&self.batch.cut, feature, query.trim()) {
            Ok(passages) => passages,
            Err(error) => {
                *reply = ListReply::Failed(ListFailure::Lookup(error));
                return;
            }
        };
        for mut passage in passages.into_iter().take(MAX_PASSAGES_PER_LOOKUP) {
            if !passage.valid_at(self.batch.cut)
                || !passage.allows(feature)
                || passage.text.len() > self.passage_bytes
                || already_offered(sources, &passage, feature)
            {
                continue;
            }
            passage.eligible = match feature {
                Feature::Ego => Eligibility::EGO,
                Feature::Perception => Eligibility::PERCEPTION,
            };
            let bytes = passage.text.len();
            sources.push(WorkItem {
                passage,
                mandatory: Eligibility::NONE,
            });
            if self.batch.fits(sources, self.batch.active_features()) {
                self.passage_bytes -= bytes;
            } else {
                sources.pop();
            }
        }
    }
}

fn already_offered(sources: &[WorkItem], passage: &Passage, feature: Feature) -> bool {
    sources.iter().any(|item| {
        let existing = item.passage.citation;
        let incoming = passage.citation;
        existing.source == incoming.source
            && (existing.revision != incoming.revision
                || existing.body_id != incoming.body_id
                || (existing == incoming && item.passage.allows(feature)))
    })
}
