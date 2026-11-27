use crate::{
    AuditError, Batch, CandidateContract, Eligibility, Feature, MAX_CANDIDATES_PER_LIST,
    MAX_LOOKUPS, MAX_OPTIONAL_PASSAGE_BYTES, MAX_PASSAGES_PER_LOOKUP, OwnerCut, Passage, Slots,
    SourceCitation, SourceId, WorkItem,
    batch::{MAX_QUERY_BYTES, OfferedLink, RequestContract, SCHEMA_NAME, encode_request},
};
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use serde::{Deserialize, Serialize};
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LookupRequest<'a> {
    Lexical(&'a str),
    Graph { anchor: SourceCitation },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GraphLink {
    pub kind: String,
    pub outgoing: bool,
}

pub struct LookupPassage {
    pub passage: Passage,
    pub link: Option<GraphLink>,
}

/// Run discovery with a lexical-only resolver.
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
    run_batch_with_lookup(
        batch,
        endpoint,
        ego,
        perception,
        &mut |cut: &OwnerCut, feature, request| {
            let LookupRequest::Lexical(query) = request else {
                return Err(LookupError::Unavailable);
            };
            Ok(lookup(cut, feature, query)?
                .into_iter()
                .map(|passage| LookupPassage {
                    passage,
                    link: None,
                })
                .collect())
        },
    )
}

/// The resolver rechecks grants and exact source standing at the captured cut.
/// Both lists spend from one lookup allowance and bind citations only to slots offered to them.
pub fn run_batch_with_lookup<E, P, L>(
    batch: &Batch,
    endpoint: &dyn GeneralEndpoint,
    ego: &E,
    perception: &P,
    lookup: &mut L,
) -> Result<BatchResult<E::Candidate, P::Candidate>, AuditError>
where
    E: CandidateContract,
    P: CandidateContract,
    L: FnMut(&OwnerCut, Feature, LookupRequest<'_>) -> Result<Vec<LookupPassage>, LookupError>
        + ?Sized,
{
    if *batch.contract != RequestContract::new(ego, perception) {
        return Err(AuditError::ContractChanged);
    }
    let active = batch.active_features();
    let mut response = dispatch(
        batch,
        endpoint,
        &batch.sources,
        active,
        &[],
        false,
        MAX_LOOKUPS,
    )?;
    let initial_slots = batch.sources.len();
    let mut ego_reply = ListReply::extract(&mut response, Feature::Ego, active, initial_slots);
    let mut perception_reply =
        ListReply::extract(&mut response, Feature::Perception, active, initial_slots);
    if !ego_reply.wants_lookup() && !perception_reply.wants_lookup() {
        return Ok(results(
            batch,
            &batch.sources,
            ego_reply,
            perception_reply,
            ego,
            perception,
        ));
    }

    let mut sources = batch.sources.clone();
    let mut links = Vec::new();
    let mut budget = LookupBudget::new(batch);
    for _ in 0..MAX_LOOKUPS {
        // Give each requesting list one turn before using any remaining slot.
        for index in 0..MAX_LOOKUPS {
            budget.search(
                &mut sources,
                &mut links,
                &mut ego_reply,
                Feature::Ego,
                index,
                lookup,
            );
            budget.search(
                &mut sources,
                &mut links,
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
        if follow_up == Eligibility::NONE {
            break;
        }
        let remaining = if budget.passage_bytes == 0 {
            0
        } else {
            budget.searches
        };
        match dispatch(
            batch, endpoint, &sources, follow_up, &links, true, remaining,
        ) {
            Ok(mut response) => {
                ego_reply.update(
                    &mut response,
                    Feature::Ego,
                    follow_up,
                    sources.len(),
                    remaining,
                );
                perception_reply.update(
                    &mut response,
                    Feature::Perception,
                    follow_up,
                    sources.len(),
                    remaining,
                );
            }
            Err(error) => {
                ego_reply.fail_follow_up(error);
                perception_reply.fail_follow_up(error);
                break;
            }
        }
    }
    ego_reply.reject_pending_lookup();
    perception_reply.reject_pending_lookup();
    Ok(results(
        batch,
        &sources,
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
    links: &[OfferedLink],
    follow_up: bool,
    remaining: usize,
) -> Result<Map<String, Value>, AuditError> {
    if !batch.fits_with_links(sources, active, links) {
        return Err(AuditError::InputLimit);
    }
    let packet = encode_request(batch, sources, active, links, follow_up, remaining);
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
    Present { fields: ReplyFields, offered: usize },
}

impl ListReply {
    fn extract(
        response: &mut Map<String, Value>,
        feature: Feature,
        active: Eligibility,
        offered: usize,
    ) -> Self {
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
                Self::Present { fields, offered }
            }
            _ => Self::Failed(ListFailure::Malformed),
        }
    }

    fn wants_lookup(&self) -> bool {
        matches!(self, Self::Present { fields, .. } if !fields.lookups.is_empty())
    }

    fn fail_follow_up(&mut self, error: AuditError) {
        if self.wants_lookup() {
            *self = Self::Failed(ListFailure::FollowUp(error));
        }
    }

    fn update(
        &mut self,
        response: &mut Map<String, Value>,
        feature: Feature,
        active: Eligibility,
        offered: usize,
        remaining: usize,
    ) {
        if self.wants_lookup() {
            *self = Self::extract(response, feature, active, offered);
            if remaining == 0 {
                self.reject_pending_lookup();
            }
        }
    }

    fn reject_pending_lookup(&mut self) {
        if self.wants_lookup() {
            *self = Self::Failed(ListFailure::Malformed);
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
            Self::Present { fields, offered } => {
                let slots = Slots {
                    sources: &sources[..offered],
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
    seen: Vec<(Feature, String)>,
}

impl<'a> LookupBudget<'a> {
    fn new(batch: &'a Batch) -> Self {
        Self {
            batch,
            searches: MAX_LOOKUPS,
            passage_bytes: MAX_OPTIONAL_PASSAGE_BYTES,
            seen: Vec::new(),
        }
    }

    fn search<L>(
        &mut self,
        sources: &mut Vec<WorkItem>,
        links: &mut Vec<OfferedLink>,
        reply: &mut ListReply,
        feature: Feature,
        index: usize,
        lookup: &mut L,
    ) where
        L: FnMut(&OwnerCut, Feature, LookupRequest<'_>) -> Result<Vec<LookupPassage>, LookupError>
            + ?Sized,
    {
        if self.searches == 0 || self.passage_bytes == 0 {
            return;
        }
        let ListReply::Present { fields, offered } = reply else {
            return;
        };
        let Some(query) = fields.lookups.get(index).map(|query| query.trim()) else {
            return;
        };
        if self
            .seen
            .iter()
            .any(|(list, prior)| *list == feature && prior == query)
        {
            return;
        }
        let (request, anchor_slot) = if let Some(slot) = query.strip_prefix("graph:") {
            let Some((slot, anchor)) = graph_anchor(&sources[..*offered], feature, slot) else {
                *reply = ListReply::Failed(ListFailure::Malformed);
                return;
            };
            (LookupRequest::Graph { anchor }, Some(slot))
        } else {
            (LookupRequest::Lexical(query), None)
        };
        self.searches -= 1;
        self.seen.push((feature, query.to_owned()));
        let passages = match lookup(&self.batch.cut, feature, request) {
            Ok(passages) => passages,
            Err(error) => {
                *reply = ListReply::Failed(ListFailure::Lookup(error));
                return;
            }
        };
        for hit in passages.into_iter().take(MAX_PASSAGES_PER_LOOKUP) {
            self.offer(sources, links, feature, anchor_slot, hit);
        }
    }

    fn offer(
        &mut self,
        sources: &mut Vec<WorkItem>,
        links: &mut Vec<OfferedLink>,
        feature: Feature,
        anchor_slot: Option<u16>,
        hit: LookupPassage,
    ) {
        let LookupPassage { mut passage, link } = hit;
        if !passage.valid_at(self.batch.cut)
            || !self.batch.mode.accepts(&passage)
            || !passage.allows(feature)
            || passage.text.len() > self.passage_bytes
        {
            return;
        }
        let incoming = passage.citation;
        let mut existing_slot = None;
        for (slot, item) in sources.iter().enumerate() {
            let existing = item.passage.citation;
            if existing.source != incoming.source {
                continue;
            }
            if existing.revision != incoming.revision || existing.body_id != incoming.body_id {
                return;
            }
            if existing == incoming {
                if item.passage.text != passage.text || item.passage.grade != passage.grade {
                    return;
                }
                if item.passage.allows(feature) || slot >= self.batch.sources.len() {
                    existing_slot = Some(slot);
                }
            }
        }
        let Ok(target_slot) = u16::try_from(existing_slot.unwrap_or(sources.len())) else {
            return;
        };
        let link_count = links.len();
        if let (Some(anchor_slot), Some(link)) = (anchor_slot, link) {
            if anchor_slot != target_slot {
                let offered = OfferedLink {
                    anchor_slot,
                    target_slot,
                    link,
                    feature,
                };
                if !links.contains(&offered) {
                    links.push(offered);
                }
            }
        }
        let eligible = match feature {
            Feature::Ego => Eligibility::EGO,
            Feature::Perception => Eligibility::PERCEPTION,
        };
        let mut bytes = 0;
        let prior_eligibility = if let Some(slot) = existing_slot {
            let current = &mut sources[slot].passage.eligible;
            let prior = *current;
            *current = prior.union(eligible);
            Some(prior)
        } else {
            passage.eligible = eligible;
            bytes = passage.text.len();
            sources.push(WorkItem {
                passage,
                mandatory: Eligibility::NONE,
            });
            None
        };
        if self
            .batch
            .fits_with_links(sources, self.batch.active_features(), links)
        {
            self.passage_bytes -= bytes;
        } else {
            links.truncate(link_count);
            if let Some(prior) = prior_eligibility {
                sources[usize::from(target_slot)].passage.eligible = prior;
            } else {
                sources.pop();
            }
        }
    }
}

fn graph_anchor(
    sources: &[WorkItem],
    feature: Feature,
    slot: &str,
) -> Option<(u16, SourceCitation)> {
    if slot.is_empty() || !slot.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let slot = slot.parse::<u16>().ok()?;
    let passage = &sources.get(usize::from(slot))?.passage;
    (passage.allows(feature) && matches!(passage.citation.source, SourceId::Memory(_)))
        .then_some((slot, passage.citation))
}
