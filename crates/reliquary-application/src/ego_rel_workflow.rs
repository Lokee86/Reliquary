//! Prepare a complete REL Ego publication from authorized, immutable evidence.

use crate::ego_pending::{AcceptedEgoBatch, EgoPendingError, EgoPendingWork, EgoRun, EgoRunKind};
use chrono::{DateTime, Utc};
use reliquary_ego::rel_summary::{
    self, Embedding, ExistingPoint, MemoryEvidence, SavedTopic, Status, Synthesis, TimeScope,
    TopicProposal, Verification, VerifiedPoint,
};
use reliquary_memory_audit::{SourceCitation, SourceId, TextField};
use reliquary_owner_session::{
    EgoCitation, EgoCompletion, EgoEmbedding, EgoItemChange, EgoPoint, EgoPointId, EgoPointStatus,
    EgoRelChange, EgoRelPublication, EgoRelSummary, EgoSourceCut, EgoTextField, EgoTimeScope,
    EgoTopic, EgoTopicId, MemoryId, OwnerSessionError, RelSession,
};
use reliquary_switchboard::{
    EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingMode, GeneralEndpoint, GeneralEndpointError,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const EMBEDDING_BATCH_SIZE: usize = 16;

pub struct RelEgoRoutes<'a> {
    pub general: &'a dyn GeneralEndpoint,
    pub embedding: &'a dyn EmbeddingEndpoint,
    pub identity: [u8; 32],
    pub embedding_profile: [u8; 32],
    pub max_request_bytes: usize,
}

pub struct RelEgoWork<'a> {
    pub run: EgoRun,
    pub batches: &'a [AcceptedEgoBatch],
    pub prior: Option<&'a EgoRelSummary>,
    pub evidence: &'a RelEgoEvidence,
    pub as_of: &'a str,
    pub verified_at_ns: i64,
}

pub struct RelEgoEvidence {
    run: EgoRun,
    prior_revision: u64,
    current: BTreeMap<SourceCitation, MemoryEvidence>,
    historical: BTreeMap<SourceCitation, MemoryEvidence>,
}

#[derive(Debug)]
pub enum RelEgoError {
    Binding,
    Budget,
    Owner(OwnerSessionError),
    Pending(EgoPendingError),
    Embedding(EmbeddingEndpointError),
    General(GeneralEndpointError),
    Contract(rel_summary::Error),
}

impl std::fmt::Display for RelEgoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Binding => "Ego REL work binding is stale or inconsistent",
            Self::Budget => "Ego REL evidence exceeds the captured route budget",
            Self::Owner(_) => "Ego REL evidence capture failed",
            Self::Pending(_) => "Ego REL stage receipt is unavailable",
            Self::Embedding(_) => "Ego REL embedding request failed",
            Self::General(_) => "Ego REL structured request failed",
            Self::Contract(_) => "Ego REL structured reply failed validation",
        })
    }
}

impl std::error::Error for RelEgoError {}

impl From<OwnerSessionError> for RelEgoError {
    fn from(error: OwnerSessionError) -> Self {
        Self::Owner(error)
    }
}

impl From<EgoPendingError> for RelEgoError {
    fn from(error: EgoPendingError) -> Self {
        Self::Pending(error)
    }
}

impl From<rel_summary::Error> for RelEgoError {
    fn from(error: rel_summary::Error) -> Self {
        Self::Contract(error)
    }
}

/// Capture source slices under the mounted REL read. A currently ineligible
/// discovery reference is excluded by the existing Ego source policy. Exact
/// historical predecessor passages can still be used for replacement judgment.
pub fn capture_rel_ego_evidence(
    rel: &RelSession,
    run: EgoRun,
    batches: &[AcceptedEgoBatch],
    prior: Option<&EgoRelSummary>,
    max_bytes: usize,
) -> Result<RelEgoEvidence, RelEgoError> {
    let cut = rel.perception_source_cut();
    if cut.owner_id != run.source.owner_id
        || cut.history_generation != run.source.history_generation
        || cut.memory < run.memory_version
    {
        return Err(RelEgoError::Binding);
    }
    validate_run(run, batches)?;
    let mut current = BTreeMap::new();
    let mut historical = BTreeMap::new();
    let mut bytes = 0;
    let memory = rel.management_read();
    for batch in batches {
        let mut retained = batch.candidates.clone();
        super::perception_workflow::retain_current_ego_evidence(rel, &mut retained)?;
        for reference in retained.iter().flat_map(|candidate| &candidate.sources) {
            if current.contains_key(reference) {
                continue;
            }
            let citation = owner_citation(*reference)?;
            let text = rel.ego_citation_text(&citation)?;
            let source = memory
                .memory_record(MemoryId(citation.memory_id), Some(citation.revision))
                .map_err(OwnerSessionError::from)?;
            if source.memory_version > run.memory_version {
                return Err(RelEgoError::Binding);
            }
            let time = source.source_time_ns.unwrap_or(source.created_at_ns);
            account_bytes(&mut bytes, text.len(), max_bytes)?;
            current.insert(*reference, evidence(*reference, time, text));
        }
    }
    if let Some(prior) = prior
        && !current.is_empty()
    {
        for item in prior
            .points
            .values()
            .filter(|item| item.tombstone.is_none())
        {
            for citation in &item.value.citations {
                if citation.owner_id != run.source.owner_id {
                    return Err(RelEgoError::Binding);
                }
                let reference = audit_citation(*citation);
                if citation.history_generation != run.source.history_generation
                    || historical.contains_key(&reference)
                    || current.contains_key(&reference)
                {
                    continue;
                }
                let Ok(text) = rel.historical_ego_citation_text(citation) else {
                    continue;
                };
                let source = memory
                    .memory_record(MemoryId(citation.memory_id), Some(citation.revision))
                    .map_err(OwnerSessionError::from)?;
                let time = source.source_time_ns.unwrap_or(source.created_at_ns);
                account_bytes(&mut bytes, text.len(), max_bytes)?;
                historical.insert(reference, evidence(reference, time, text));
            }
        }
    }
    Ok(RelEgoEvidence {
        run,
        prior_revision: prior.map_or(0, |summary| summary.revision),
        current,
        historical,
    })
}

fn account_bytes(bytes: &mut usize, len: usize, limit: usize) -> Result<(), RelEgoError> {
    *bytes = bytes
        .checked_add(len)
        .filter(|bytes| *bytes <= limit)
        .ok_or(RelEgoError::Budget)?;
    Ok(())
}

fn evidence(citation: SourceCitation, time: i64, text: String) -> MemoryEvidence {
    MemoryEvidence {
        citation,
        source_date: DateTime::<Utc>::from_timestamp_nanos(time)
            .format("%Y-%m-%d")
            .to_string(),
        text,
    }
}

fn validate_run(run: EgoRun, batches: &[AcceptedEgoBatch]) -> Result<(), RelEgoError> {
    let mut ids = BTreeSet::new();
    if run.phy == run.source.owner_id
        || !matches!(
            run.kind,
            EgoRunKind::RelBootstrap | EgoRunKind::RelSharedBootstrap | EgoRunKind::RelRecurring
        )
        || batches.iter().any(|batch| !ids.insert(batch.id))
        || batches
            .iter()
            .flat_map(|batch| &batch.candidates)
            .flat_map(|candidate| &candidate.sources)
            .any(|reference| {
                reference.owner_id != run.source.owner_id
                    || reference.history_generation != run.source.history_generation
                    || !matches!(reference.source, SourceId::Memory(_))
            })
    {
        return Err(RelEgoError::Binding);
    }
    Ok(())
}

/// Synthesis and verification use the configured General route outside owner
/// locks. The mounted publisher rechecks source eligibility and the expected
/// summary revision before committing this single publication.
pub fn prepare_rel_ego_publication(
    routes: &RelEgoRoutes<'_>,
    work: RelEgoWork<'_>,
    pending: &mut EgoPendingWork,
) -> Result<EgoRelPublication, RelEgoError> {
    validate_run(work.run, work.batches)?;
    if routes.identity != work.run.route
        || work.evidence.run != work.run
        || work.evidence.prior_revision != work.prior.map_or(0, |summary| summary.revision)
        || work.as_of.trim().is_empty()
    {
        return Err(RelEgoError::Binding);
    }
    let mut work_batches: Vec<_> = work.batches.iter().collect();
    work_batches.sort_by_key(|batch| batch.sequence);
    pending.require_rel_batches(work.run, &work_batches)?;
    let mut proposals = BTreeMap::<String, Vec<SourceCitation>>::new();
    for candidate in work_batches.iter().flat_map(|batch| &batch.candidates) {
        let references = candidate
            .sources
            .iter()
            .copied()
            .filter(|source| work.evidence.current.contains_key(source));
        proposals
            .entry(candidate.subject.clone())
            .or_default()
            .extend(references);
    }
    proposals.retain(|_, references| !references.is_empty());
    let mut session = RelPreparation {
        routes,
        run: work.run,
        pending,
        vectors: BTreeMap::new(),
    };
    session.embed(proposals.keys().map(String::as_str))?;
    let proposals = rel_summary::consolidate_topics(
        proposals
            .into_iter()
            .map(|(title, references)| TopicProposal {
                embedding: session.vectors.get(&title).cloned(),
                title,
                references,
            })
            .collect(),
    );
    let saved = if proposals.is_empty() {
        Vec::new()
    } else {
        session.saved_topics(work.prior)?
    };
    let mut by_section = BTreeMap::<_, TopicProposal>::new();
    let mut new_sections = Vec::new();
    for proposal in proposals {
        if let Some(planned) = rel_summary::plan_topic(proposal, &saved) {
            if let Some(matched) = planned.matched {
                by_section
                    .entry(matched.id)
                    .and_modify(|existing| {
                        existing.references.extend(&planned.proposal.references);
                        existing.references.sort_unstable();
                        existing.references.dedup();
                    })
                    .or_insert(planned.proposal);
            } else {
                new_sections.push(planned.proposal);
            }
        }
    }

    let mut changes = Vec::new();
    for proposal in by_section.into_values().chain(new_sections) {
        let Some(plan) = rel_summary::plan_topic(proposal, &saved) else {
            continue;
        };
        let topic_id = EgoTopicId(plan.matched.map_or_else(
            || stable_id(work.run, b"topic", &[], &plan.proposal.title),
            |target| target.id,
        ));
        if plan.matched.is_none()
            && work
                .prior
                .is_some_and(|prior| prior.topics.contains_key(&topic_id))
        {
            return Err(RelEgoError::Binding);
        }
        let evidence = plan
            .proposal
            .references
            .iter()
            .map(|reference| {
                work.evidence
                    .current
                    .get(reference)
                    .cloned()
                    .ok_or(RelEgoError::Binding)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let synthesis = Synthesis::new(&plan.proposal.title, work.as_of, &evidence)?;
        let stage_binding = serde_json::to_vec(&(
            topic_id,
            plan.matched.map(|target| target.revision),
            &plan.proposal.references,
        ))
        .map_err(|_| RelEgoError::Binding)?;
        let request = synthesis.request();
        let (key, reply) =
            session.request("reliquary_ego_rel_synthesize", &stage_binding, &request)?;
        let mut drafts = synthesis.parse(reply.clone())?;
        session.remember(key, &reply)?;

        let mut points = Vec::new();
        if !drafts.is_empty() {
            session.embed(drafts.iter().map(|draft| draft.statement()))?;
            for draft in &mut drafts {
                draft.embedding = session.vectors.get(draft.statement()).cloned();
            }
            let (live, suppressed) =
                session.existing_points(topic_id, work.prior, work.evidence)?;
            let verify = Verification::new(&synthesis, &plan, drafts, &live, &suppressed);
            let (key, reply) = session.request(
                "reliquary_ego_rel_verify",
                &stage_binding,
                &verify.request(),
            )?;
            points = verify.accept(reply.clone())?;
            session.remember(key, &reply)?;
        }

        let old = saved.iter().find(|topic| topic.id == topic_id.0);
        let (title, embedding, mut references, expected_revision) = match old {
            Some(topic) => (
                topic.title.clone(),
                topic.embedding.as_ref().map(owner_embedding),
                topic
                    .references
                    .iter()
                    .filter(|reference| {
                        reference.history_generation == work.run.source.history_generation
                    })
                    .map(|reference| owner_citation(*reference))
                    .collect::<Result<Vec<_>, _>>()?,
                topic.revision,
            ),
            None => (
                plan.proposal.title.clone(),
                plan.proposal.embedding.as_ref().map(owner_embedding),
                Vec::new(),
                0,
            ),
        };
        references.extend(
            plan.proposal
                .references
                .iter()
                .copied()
                .map(owner_citation)
                .collect::<Result<Vec<_>, _>>()?,
        );
        references.sort_unstable();
        references.dedup();
        changes.push(EgoRelChange::Topic {
            id: topic_id,
            expected_revision,
            change: EgoItemChange::Put(EgoTopic {
                title,
                embedding,
                references,
            }),
        });

        let mut updated = BTreeSet::new();
        for point in points {
            if point.citations.is_empty()
                || !point
                    .citations
                    .iter()
                    .all(|citation| captured_citation(&evidence, citation))
            {
                return Err(RelEgoError::Binding);
            }
            let id = EgoPointId(point.replacement.map_or_else(
                || stable_id(work.run, b"point", &topic_id.0, &point.statement),
                |target| target.id,
            ));
            if !updated.insert(id) {
                continue;
            }
            let expected_revision = point.replacement.map_or(0, |target| target.revision);
            changes.push(EgoRelChange::Point {
                id,
                expected_revision,
                change: EgoItemChange::Put(owner_point(point, topic_id, work.verified_at_ns)?),
            });
        }
    }

    Ok(EgoRelPublication {
        source_owner_id: work.run.source.owner_id,
        expected_revision: work.prior.map_or(0, |prior| prior.revision),
        completion: Some(EgoCompletion {
            id: work.run.id(),
            cut: EgoSourceCut {
                owner_id: work.run.source.owner_id,
                history_generation: work.run.source.history_generation,
                memory_version: work.run.memory_version,
                audit_frontier: work.run.source.frontier,
            },
            batch_ids: work_batches.into_iter().map(|batch| batch.id).collect(),
        }),
        changes,
    })
}

fn captured_citation(evidence: &[MemoryEvidence], citation: &SourceCitation) -> bool {
    evidence.iter().any(|memory| {
        let source = memory.citation;
        let Some(start) = citation.start_byte.checked_sub(source.start_byte) else {
            return false;
        };
        let Some(end) = citation.end_byte.checked_sub(source.start_byte) else {
            return false;
        };
        source
            == SourceCitation {
                start_byte: source.start_byte,
                end_byte: source.end_byte,
                ..*citation
            }
            && memory
                .text
                .get(start as usize..end as usize)
                .is_some_and(|text| !text.trim().is_empty())
    })
}

struct RelPreparation<'a, 'b> {
    routes: &'a RelEgoRoutes<'a>,
    run: EgoRun,
    pending: &'b mut EgoPendingWork,
    vectors: BTreeMap<String, Embedding>,
}

impl RelPreparation<'_, '_> {
    fn embed<'a>(&mut self, inputs: impl IntoIterator<Item = &'a str>) -> Result<(), RelEgoError> {
        let inputs: Vec<_> = inputs
            .into_iter()
            .filter(|text| !self.vectors.contains_key(*text))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(str::to_owned)
            .collect();
        for batch in inputs.chunks(EMBEDDING_BATCH_SIZE) {
            let input = serde_json::to_string(batch).map_err(|_| RelEgoError::Binding)?;
            let key = stage_key(self.routes.embedding_profile, "embed", &[], "", &input, "");
            let stored = self.pending.rel_stage(self.run, key)?;
            let response: Vec<Vec<f32>> = match &stored {
                Some(value) => {
                    serde_json::from_value(value.clone()).map_err(|_| RelEgoError::Binding)?
                }
                None => self
                    .routes
                    .embedding
                    .embed(EmbeddingMode::Document, batch)
                    .map_err(RelEgoError::Embedding)?,
            };
            if response.len() != batch.len()
                || response.iter().any(|vector| {
                    vector.len() != self.routes.embedding.dimensions() as usize
                        || !vector.iter().all(|value| value.is_finite())
                        || vector.iter().all(|value| *value == 0.0)
                })
            {
                return Err(RelEgoError::Binding);
            }
            if stored.is_none() {
                let value = serde_json::to_value(&response).map_err(|_| RelEgoError::Binding)?;
                self.remember(key, &value)?;
            }
            for (text, vector) in batch.iter().zip(response) {
                self.vectors.insert(
                    text.clone(),
                    Embedding {
                        profile_id: self.routes.embedding_profile,
                        vector,
                    },
                );
            }
        }
        Ok(())
    }

    fn saved_topics(
        &mut self,
        prior: Option<&EgoRelSummary>,
    ) -> Result<Vec<SavedTopic>, RelEgoError> {
        let Some(prior) = prior else {
            return Ok(Vec::new());
        };
        let included = prior
            .topics
            .iter()
            .filter(|(_, item)| item.tombstone.is_none_or(|tombstone| tombstone.suppressed))
            .collect::<Vec<_>>();
        let reembed = included
            .iter()
            .filter_map(|(_, item)| {
                (!self.usable_embedding(item.value.embedding.as_ref()))
                    .then_some(item.value.title.as_str())
            })
            .collect::<Vec<_>>();
        self.embed(reembed)?;
        Ok(included
            .into_iter()
            .map(|(id, item)| SavedTopic {
                id: id.0,
                revision: item.revision,
                title: item.value.title.clone(),
                embedding: self.stored_embedding(&item.value.title, item.value.embedding.as_ref()),
                references: item
                    .value
                    .references
                    .iter()
                    .copied()
                    .map(audit_citation)
                    .collect(),
                suppressed: item.tombstone.is_some(),
            })
            .collect())
    }

    fn existing_points(
        &mut self,
        topic: EgoTopicId,
        prior: Option<&EgoRelSummary>,
        evidence: &RelEgoEvidence,
    ) -> Result<(Vec<ExistingPoint>, Vec<ExistingPoint>), RelEgoError> {
        let Some(prior) = prior else {
            return Ok((Vec::new(), Vec::new()));
        };
        let included = prior
            .points
            .iter()
            .filter(|(_, item)| {
                item.tombstone.is_some_and(|tombstone| tombstone.suppressed)
                    || (item.value.topic_id == topic && item.tombstone.is_none())
            })
            .collect::<Vec<_>>();
        let reembed = included
            .iter()
            .filter_map(|(_, item)| {
                let current_profile = self.usable_embedding(item.value.embedding.as_ref());
                (item.tombstone.is_some() && !current_profile)
                    .then_some(item.value.statement.as_str())
            })
            .collect::<Vec<_>>();
        self.embed(reembed)?;
        let mut live = Vec::new();
        let mut suppressed = Vec::new();
        for (id, item) in included {
            let point = &item.value;
            let citations: Vec<_> = point
                .citations
                .iter()
                .copied()
                .map(audit_citation)
                .collect();
            let existing = ExistingPoint {
                id: id.0,
                revision: item.revision,
                statement: point.statement.clone(),
                status: match point.status {
                    EgoPointStatus::Observed => Status::Observed,
                    EgoPointStatus::Planned => Status::Planned,
                    EgoPointStatus::Requested => Status::Requested,
                    EgoPointStatus::Possible => Status::Possible,
                    EgoPointStatus::Expected => Status::Expected,
                },
                time_scope: match point.time_scope {
                    EgoTimeScope::Past => TimeScope::Past,
                    EgoTimeScope::Current => TimeScope::Current,
                    EgoTimeScope::Future => TimeScope::Future,
                    EgoTimeScope::Unspecified => TimeScope::Unspecified,
                },
                embedding: self.stored_embedding(&point.statement, point.embedding.as_ref()),
                evidence: citations
                    .iter()
                    .filter_map(|reference| {
                        evidence
                            .historical
                            .get(reference)
                            .or_else(|| evidence.current.get(reference))
                            .cloned()
                    })
                    .collect(),
                citations,
            };
            if item.tombstone.is_none() {
                live.push(existing);
            } else {
                suppressed.push(existing);
            }
        }
        Ok((live, suppressed))
    }

    fn usable_embedding(&self, embedding: Option<&EgoEmbedding>) -> bool {
        embedding.is_some_and(|value| {
            value.profile_id == self.routes.embedding_profile
                && value.components.len() == self.routes.embedding.dimensions() as usize
        })
    }

    fn stored_embedding(&self, text: &str, embedding: Option<&EgoEmbedding>) -> Option<Embedding> {
        embedding
            .filter(|value| self.usable_embedding(Some(value)))
            .map(|value| Embedding {
                profile_id: value.profile_id,
                vector: value.components().collect(),
            })
            .or_else(|| self.vectors.get(text).cloned())
    }

    fn request(
        &self,
        stage: &'static str,
        binding: &[u8],
        request: &rel_summary::StructuredRequest,
    ) -> Result<([u8; 32], Value), RelEgoError> {
        let payload = request.input.to_string();
        let schema = request.schema.to_string();
        let request_bytes = payload
            .len()
            .saturating_add(request.instruction.len())
            .saturating_add(schema.len());
        if request_bytes > self.routes.max_request_bytes {
            return Err(RelEgoError::Budget);
        }
        let key = stage_key(
            self.routes.embedding_profile,
            stage,
            binding,
            &request.instruction,
            &payload,
            &schema,
        );
        if let Some(reply) = self.pending.rel_stage(self.run, key)? {
            return Ok((key, reply));
        }
        let reply = self
            .routes
            .general
            .complete_json(&request.instruction, &payload, stage, &request.schema)
            .map_err(RelEgoError::General)?;
        Ok((key, reply))
    }

    fn remember(&mut self, key: [u8; 32], reply: &Value) -> Result<(), RelEgoError> {
        self.pending.accept_rel_stage(self.run, key, reply)?;
        Ok(())
    }
}

fn stable_id(run: EgoRun, kind: &[u8], parent: &[u8], statement: &str) -> [u8; 16] {
    let mut hash = Sha256::new();
    hash.update(b"Reliquary Ego REL item v1");
    hash.update(run.id());
    hash.update(kind);
    hash.update(parent);
    hash.update(statement.as_bytes());
    let digest: [u8; 32] = hash.finalize().into();
    digest[..16].try_into().expect("first half of the digest")
}

fn stage_key(
    profile: [u8; 32],
    stage: &str,
    binding: &[u8],
    instruction: &str,
    payload: &str,
    schema: &str,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"Reliquary Ego REL stage v1");
    hash.update(profile);
    hash.update((binding.len() as u64).to_le_bytes());
    hash.update(binding);
    for value in [stage, instruction, payload, schema] {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
    hash.finalize().into()
}

fn owner_embedding(value: &Embedding) -> EgoEmbedding {
    EgoEmbedding::from_components(value.profile_id, &value.vector)
}

fn owner_point(
    point: VerifiedPoint,
    topic_id: EgoTopicId,
    verified_at_ns: i64,
) -> Result<EgoPoint, RelEgoError> {
    Ok(EgoPoint {
        topic_id,
        statement: point.statement,
        status: match point.status {
            Status::Observed => EgoPointStatus::Observed,
            Status::Planned => EgoPointStatus::Planned,
            Status::Requested => EgoPointStatus::Requested,
            Status::Possible => EgoPointStatus::Possible,
            Status::Expected => EgoPointStatus::Expected,
        },
        time_scope: match point.time_scope {
            TimeScope::Past => EgoTimeScope::Past,
            TimeScope::Current => EgoTimeScope::Current,
            TimeScope::Future => EgoTimeScope::Future,
            TimeScope::Unspecified => EgoTimeScope::Unspecified,
        },
        verified_at_ns,
        citations: point
            .citations
            .into_iter()
            .map(owner_citation)
            .collect::<Result<Vec<_>, _>>()?,
        embedding: point.embedding.as_ref().map(owner_embedding),
    })
}

fn owner_citation(citation: SourceCitation) -> Result<EgoCitation, RelEgoError> {
    let SourceId::Memory(memory_id) = citation.source else {
        return Err(RelEgoError::Binding);
    };
    Ok(EgoCitation {
        owner_id: citation.owner_id,
        history_generation: citation.history_generation,
        memory_id,
        revision: citation.revision,
        body_id: citation.body_id,
        field: match citation.field {
            TextField::Title => EgoTextField::Title,
            TextField::Content => EgoTextField::Content,
        },
        start_byte: citation.start_byte,
        end_byte: citation.end_byte,
    })
}

fn audit_citation(citation: EgoCitation) -> SourceCitation {
    SourceCitation {
        owner_id: citation.owner_id,
        history_generation: citation.history_generation,
        source: SourceId::Memory(citation.memory_id),
        revision: citation.revision,
        body_id: citation.body_id,
        field: match citation.field {
            EgoTextField::Title => TextField::Title,
            EgoTextField::Content => TextField::Content,
        },
        start_byte: citation.start_byte,
        end_byte: citation.end_byte,
    }
}

#[cfg(test)]
mod tests;
