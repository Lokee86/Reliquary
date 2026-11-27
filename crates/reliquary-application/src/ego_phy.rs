//! Prepare one verified PHY summary publication from the mounted audit capture.

use crate::perception_workflow::phy_audit_citation;
use chrono::{DateTime, Utc};
use reliquary_ego::{
    EgoCandidate,
    phy_summary::{
        self, ChangeKind, MeasuredDraft, MemoryChange, MemoryEvidence, PriorParagraph,
        PriorSummary, StructuredRequest, Synthesis, TokenBudget, VerifiedSummary,
    },
};
use reliquary_memory_audit::{SourceCitation, SourceId, TextField};
use reliquary_owner_session::{
    EgoCitation, EgoCompletion, EgoParagraph, EgoPhyPublication, EgoPhyText, EgoTextField,
    MAX_EGO_PHY_SUMMARY_TOKENS, MemoryId, OwnerSessionError, PhyAuditChange, PhyAuditCut,
    PhyWriter, model::MemoryBody,
};
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError, GeneralTool};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

const PASSAGE_BYTES: usize = 4 * 1024;

pub struct PhySummaryCapture {
    cut: PhyAuditCut,
    completion: EgoCompletion,
    expected_revision: u64,
    as_of: String,
    prior: Option<PriorSummary>,
    changes: Vec<MemoryChange>,
    evidence: Vec<MemoryEvidence>,
    input_revisions: BTreeMap<MemoryId, Option<u64>>,
}

#[derive(Debug)]
pub enum PhySummaryError {
    Owner(OwnerSessionError),
    Policy(phy_summary::Error),
    Endpoint(GeneralEndpointError),
    InvalidCapture,
    InputLimit,
}

impl fmt::Display for PhySummaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Owner(_) => "PHY summary source is unavailable",
            Self::Policy(_) => "PHY summary failed its structured stage",
            Self::Endpoint(_) => "PHY summary inference failed",
            Self::InvalidCapture => "invalid PHY summary capture binding",
            Self::InputLimit => "PHY summary evidence exceeded the route budget",
        })
    }
}

impl std::error::Error for PhySummaryError {}

impl From<OwnerSessionError> for PhySummaryError {
    fn from(error: OwnerSessionError) -> Self {
        Self::Owner(error)
    }
}

impl From<phy_summary::Error> for PhySummaryError {
    fn from(error: phy_summary::Error) -> Self {
        Self::Policy(error)
    }
}

impl From<GeneralEndpointError> for PhySummaryError {
    fn from(error: GeneralEndpointError) -> Self {
        Self::Endpoint(error)
    }
}

/// Capture exact evidence under the caller's active PHY lease. Inference runs
/// on the resulting values after the caller releases the lease.
pub fn capture_phy_summary(
    phy: &PhyWriter,
    cut: PhyAuditCut,
    completion: EgoCompletion,
    accepted: &[EgoCandidate],
    changes: &[PhyAuditChange],
    as_of_ns: i64,
) -> Result<PhySummaryCapture, PhySummaryError> {
    if !cut.is_at_or_before(phy.phy_audit_cut())
        || completion.cut.owner_id != cut.owner_id
        || completion.cut.history_generation != cut.history_generation
        || completion.cut.memory_version != cut.memory
    {
        return Err(PhySummaryError::InvalidCapture);
    }

    let mut changed_ids = BTreeMap::new();
    for change in changes {
        let kind = match (change.previous_revision, change.current_revision) {
            (None, Some(_)) => ChangeKind::Added,
            (Some(_), Some(_)) => ChangeKind::Revised,
            (Some(_), None) => ChangeKind::Withdrawn,
            _ => return Err(PhySummaryError::InvalidCapture),
        };
        if changed_ids
            .insert(change.memory, (kind, change.current_revision))
            .is_some()
        {
            return Err(PhySummaryError::InvalidCapture);
        }
    }

    let saved = phy.ego_phy_summary();
    let prior = saved
        .and_then(|summary| summary.text.as_ref())
        .map(|previous| PriorSummary {
            text: previous.text.clone(),
            paragraphs: previous
                .paragraphs
                .iter()
                .map(|paragraph| {
                    let citations: Vec<_> =
                        paragraph.citations.iter().map(audit_citation).collect();
                    let can_carry = citations
                        .iter()
                        .all(|citation| phy_audit_citation(phy, cut, *citation).is_ok());
                    PriorParagraph {
                        start_byte: paragraph.start_byte,
                        end_byte: paragraph.end_byte,
                        citations,
                        can_carry,
                    }
                })
                .collect(),
        });

    let mut reopen: BTreeSet<_> = changed_ids
        .iter()
        .filter_map(|(&id, (_, revision))| revision.map(|_| id))
        .collect();
    for paragraph in prior.iter().flat_map(|summary| &summary.paragraphs) {
        let ids = paragraph
            .citations
            .iter()
            .filter_map(|citation| match citation.source {
                SourceId::Memory(id) => Some(MemoryId(id)),
                SourceId::Observation(_) => None,
            });
        if !paragraph.can_carry || ids.clone().any(|id| changed_ids.contains_key(&id)) {
            reopen.extend(
                ids.filter(|id| !matches!(changed_ids.get(id), Some((ChangeKind::Withdrawn, _)))),
            );
        }
    }

    let mut input_revisions: BTreeMap<_, _> = changed_ids
        .iter()
        .map(|(&id, &(_, revision))| (id, revision))
        .collect();
    for paragraph in prior
        .iter()
        .flat_map(|summary| &summary.paragraphs)
        .filter(|p| p.can_carry)
    {
        for citation in &paragraph.citations {
            if let SourceId::Memory(id) = citation.source {
                input_revisions.insert(MemoryId(id), Some(citation.revision));
            }
        }
    }
    let mut evidence = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for citation in accepted.iter().flat_map(|topic| &topic.sources).copied() {
        if !seen.insert(citation) {
            continue;
        }
        let (record, text) = phy_audit_citation(phy, cut, citation)?;
        input_revisions.insert(record.id, Some(record.revision));
        let source_date = date(record.source_time_ns.unwrap_or(record.created_at_ns));
        append_passages(&mut evidence, citation, &source_date, &text);
    }
    for id in reopen {
        let current = phy.phy_audit_evidence(cut, id)?;
        if let Some((_, Some(expected))) = changed_ids.get(&id)
            && current
                .as_ref()
                .is_none_or(|(record, _)| record.revision != *expected)
        {
            return Err(PhySummaryError::InvalidCapture);
        }
        input_revisions.insert(id, current.as_ref().map(|(record, _)| record.revision));
        let Some((record, body)) = current else {
            continue;
        };
        let source_date = date(record.source_time_ns.unwrap_or(record.created_at_ns));
        for (field, text) in [
            (TextField::Title, &body.title),
            (TextField::Content, &body.content),
        ] {
            let citation = SourceCitation {
                owner_id: cut.owner_id,
                history_generation: cut.history_generation,
                source: SourceId::Memory(id.0),
                revision: record.revision,
                body_id: record.body_id.0,
                field,
                start_byte: 0,
                end_byte: text
                    .len()
                    .try_into()
                    .map_err(|_| PhySummaryError::InvalidCapture)?,
            };
            append_passages(&mut evidence, citation, &source_date, text);
        }
    }
    Ok(PhySummaryCapture {
        cut,
        completion,
        expected_revision: saved.map_or(0, |summary| summary.revision),
        as_of: date(as_of_ns),
        prior,
        changes: changed_ids
            .into_iter()
            .map(|(id, (kind, _))| MemoryChange {
                memory_id: id.0,
                kind,
            })
            .collect(),
        evidence: evidence.into_values().collect(),
        input_revisions,
    })
}

/// Recheck the source inputs under the final writer lease. Unrelated source
/// changes can advance the current owner head without invalidating this work.
pub fn revalidate_phy_summary(
    phy: &PhyWriter,
    capture: &PhySummaryCapture,
) -> Result<(), PhySummaryError> {
    let latest = phy.phy_audit_cut();
    if !capture.cut.is_at_or_before(latest)
        || phy.ego_phy_summary().map_or(0, |summary| summary.revision) != capture.expected_revision
    {
        return Err(PhySummaryError::Owner(OwnerSessionError::ManagementStale));
    }
    revalidate_inputs(capture, |id| {
        Ok(phy
            .phy_audit_evidence(latest, id)?
            .map(|(record, body)| SourceHead {
                revision: record.revision,
                body_id: record.body_id.0,
                body,
            }))
    })
}

struct SourceHead {
    revision: u64,
    body_id: [u8; 32],
    body: MemoryBody,
}

fn revalidate_inputs(
    capture: &PhySummaryCapture,
    mut read: impl FnMut(MemoryId) -> Result<Option<SourceHead>, OwnerSessionError>,
) -> Result<(), PhySummaryError> {
    let mut citations = BTreeMap::<_, Vec<_>>::new();
    let prior = capture
        .prior
        .iter()
        .flat_map(|summary| &summary.paragraphs)
        .filter(|paragraph| paragraph.can_carry)
        .flat_map(|paragraph| &paragraph.citations)
        .map(|citation| (citation, None));
    let supplied = capture
        .evidence
        .iter()
        .map(|evidence| (&evidence.citation, Some(evidence.text.as_str())));
    for (citation, text) in prior.chain(supplied) {
        let SourceId::Memory(id) = citation.source else {
            return Err(PhySummaryError::InvalidCapture);
        };
        citations
            .entry(MemoryId(id))
            .or_default()
            .push((citation, text));
    }
    for (&id, &expected) in &capture.input_revisions {
        let head = read(id)?;
        if head.as_ref().map(|head| head.revision) != expected {
            return Err(PhySummaryError::Owner(OwnerSessionError::ManagementStale));
        }
        if let Some(references) = citations.remove(&id) {
            let valid = head.is_some_and(|head| {
                references.iter().all(|(citation, exact)| {
                    let text = match citation.field {
                        TextField::Title => &head.body.title,
                        TextField::Content => &head.body.content,
                    };
                    head.body_id == citation.body_id
                        && head.revision == citation.revision
                        && text
                            .get(citation.start_byte as usize..citation.end_byte as usize)
                            .is_some_and(|text| {
                                !text.is_empty() && exact.is_none_or(|exact| exact == text)
                            })
                })
            });
            if !valid {
                return Err(PhySummaryError::Owner(OwnerSessionError::ManagementStale));
            }
        }
    }
    if !citations.is_empty() {
        return Err(PhySummaryError::InvalidCapture);
    }
    Ok(())
}

fn append_passages(
    evidence: &mut BTreeMap<SourceCitation, MemoryEvidence>,
    source: SourceCitation,
    source_date: &str,
    text: &str,
) {
    let mut start = 0;
    while start < text.len() {
        let limit = text.floor_char_boundary(start.saturating_add(PASSAGE_BYTES).min(text.len()));
        let end = if limit < text.len() {
            text[start..limit]
                .rfind(char::is_whitespace)
                .map(|offset| start + offset)
                .filter(|at| *at > start + PASSAGE_BYTES / 2)
                .unwrap_or(limit)
        } else {
            limit
        };
        let citation = SourceCitation {
            start_byte: source.start_byte + start as u32,
            end_byte: source.start_byte + end as u32,
            ..source
        };
        evidence.entry(citation).or_insert_with(|| MemoryEvidence {
            citation,
            source_date: source_date.to_owned(),
            text: text[start..end].to_owned(),
        });
        start = end;
    }
}

fn date(timestamp: i64) -> String {
    DateTime::<Utc>::from_timestamp_nanos(timestamp)
        .format("%Y-%m-%d")
        .to_string()
}

fn audit_citation(citation: &EgoCitation) -> SourceCitation {
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

fn owner_citation(citation: SourceCitation) -> Result<EgoCitation, PhySummaryError> {
    let SourceId::Memory(memory_id) = citation.source else {
        return Err(PhySummaryError::InvalidCapture);
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

pub fn prepare_phy_publication(
    capture: &PhySummaryCapture,
    endpoint: &dyn GeneralEndpoint,
    counter: &(dyn Fn(&str) -> u64 + Sync),
    max_input_bytes: usize,
) -> Result<Option<EgoPhyPublication>, PhySummaryError> {
    let synthesis = Synthesis::new(
        &capture.as_of,
        capture.prior.as_ref(),
        &capture.changes,
        &capture.evidence,
    )?;
    let budget = TokenBudget::new(u64::from(MAX_EGO_PHY_SUMMARY_TOKENS), counter)?;
    let verified = if request_fits(&synthesis.request(), max_input_bytes) {
        synthesize(&synthesis, endpoint, budget, max_input_bytes, &[])?
    } else {
        let Some((reduced, analyses)) =
            analyze_packets(capture, endpoint, budget, max_input_bytes)?
        else {
            return Ok(None);
        };
        let synthesis = Synthesis::new(
            &capture.as_of,
            capture.prior.as_ref(),
            &capture.changes,
            &reduced,
        )?;
        synthesize(&synthesis, endpoint, budget, max_input_bytes, &analyses)?
    };
    let Some(verified) = verified else {
        return Ok(None);
    };
    let paragraphs = verified
        .paragraphs
        .into_iter()
        .map(|paragraph| {
            let citations = paragraph
                .citations
                .into_iter()
                .map(owner_citation)
                .collect::<Result<_, _>>()?;
            Ok(EgoParagraph {
                start_byte: paragraph.start_byte,
                end_byte: paragraph.end_byte,
                citations,
            })
        })
        .collect::<Result<_, PhySummaryError>>()?;
    let replacement = EgoPhyText {
        text: verified.text,
        paragraphs,
        token_count: verified
            .token_count
            .try_into()
            .map_err(|_| PhySummaryError::InvalidCapture)?,
    };
    Ok(Some(EgoPhyPublication {
        expected_revision: capture.expected_revision,
        completion: capture.completion.clone(),
        replacement: Some(replacement),
    }))
}

fn request_fits(request: &StructuredRequest, max_input_bytes: usize) -> bool {
    request
        .instruction
        .len()
        .saturating_add(request.schema.to_string().len())
        .saturating_add(request.input.to_string().len())
        <= max_input_bytes
}

fn with_analysis(mut request: StructuredRequest, analyses: &[Value]) -> StructuredRequest {
    if !analyses.is_empty() {
        request.instruction.push_str(
            "\nPacket analyses assist selection. They are derived context. Cite the original \
            Memories and use the analyses to preserve distinctions across packets.",
        );
        request.input["packet_analyses"] = json!(analyses);
    }
    request
}

fn call(
    endpoint: &dyn GeneralEndpoint,
    request: StructuredRequest,
    name: &str,
    max_input_bytes: usize,
) -> Result<Value, PhySummaryError> {
    if !request_fits(&request, max_input_bytes) {
        return Err(PhySummaryError::InputLimit);
    }
    Ok(endpoint.complete_json(
        &request.instruction,
        &request.input.to_string(),
        name,
        &request.schema,
    )?)
}

fn synthesize(
    synthesis: &Synthesis<'_>,
    endpoint: &dyn GeneralEndpoint,
    budget: TokenBudget<'_>,
    max_input_bytes: usize,
    analyses: &[Value],
) -> Result<Option<VerifiedSummary>, PhySummaryError> {
    let draft = synthesis.parse(call(
        endpoint,
        with_analysis(synthesis.request(), analyses),
        "ego_phy_draft",
        max_input_bytes,
    )?)?;
    let candidate = match synthesis.measure(draft, budget) {
        MeasuredDraft::Fits(candidate) => candidate,
        MeasuredDraft::Compress(mut job) => {
            let job_request = job.request();
            let request = with_analysis(job_request.structured, analyses);
            let request_limit = max_input_bytes
                .checked_sub(job_request.tool_schema.to_string().len())
                .ok_or(PhySummaryError::InputLimit)?;
            if !request_fits(&request, request_limit) {
                return Err(PhySummaryError::InputLimit);
            }
            let terminal = endpoint.complete_json_with_tool(
                &request.instruction,
                &request.input.to_string(),
                "ego_phy_compress",
                &request.schema,
                GeneralTool {
                    name: job_request.tool_name,
                    schema: &job_request.tool_schema,
                    max_results: phy_summary::MAX_TOKENIZER_RESULTS,
                    max_input_bytes,
                },
                &mut |arguments| {
                    job.tokenize(arguments).map_err(|_| {
                        GeneralEndpointError::InvalidResponse("invalid PHY tokenizer continuation")
                    })
                },
            )?;
            let Some(candidate) = job.finish(terminal)? else {
                return Ok(None);
            };
            candidate
        }
    };
    let verification = candidate.verification();
    let reply = call(
        endpoint,
        with_analysis(verification.request(), analyses),
        "ego_phy_verify",
        max_input_bytes,
    )?;
    Ok(verification.accept(reply)?)
}

fn analyze_packets(
    capture: &PhySummaryCapture,
    endpoint: &dyn GeneralEndpoint,
    budget: TokenBudget<'_>,
    max_input_bytes: usize,
) -> Result<Option<(Vec<MemoryEvidence>, Vec<Value>)>, PhySummaryError> {
    let mut reduced = BTreeMap::new();
    let mut findings = Vec::new();
    let mut start = 0;
    // Reserve space for draft text and final checks when planning packet requests.
    let packet_limit = max_input_bytes / 2;
    while start < capture.evidence.len() {
        let mut end = start;
        while end < capture.evidence.len() {
            let synthesis =
                Synthesis::new(&capture.as_of, None, &[], &capture.evidence[start..=end])?;
            if !request_fits(&synthesis.request(), packet_limit) {
                break;
            }
            end += 1;
        }
        if end == start {
            return Err(PhySummaryError::InputLimit);
        }
        let packet = &capture.evidence[start..end];
        let synthesis = Synthesis::new(&capture.as_of, None, &[], packet)?;
        let Some(verified) = synthesize(&synthesis, endpoint, budget, max_input_bytes, &[])? else {
            return Ok(None);
        };
        for paragraph in verified.paragraphs {
            let mut refs = Vec::new();
            for cite in paragraph.citations {
                let source = packet
                    .iter()
                    .find(|source| {
                        source.citation.owner_id == cite.owner_id
                            && source.citation.history_generation == cite.history_generation
                            && source.citation.source == cite.source
                            && source.citation.revision == cite.revision
                            && source.citation.body_id == cite.body_id
                            && source.citation.field == cite.field
                            && source.citation.start_byte <= cite.start_byte
                            && source.citation.end_byte >= cite.end_byte
                    })
                    .ok_or(PhySummaryError::InvalidCapture)?;
                let offset = source.citation.start_byte;
                let text = source
                    .text
                    .get((cite.start_byte - offset) as usize..(cite.end_byte - offset) as usize)
                    .ok_or(PhySummaryError::InvalidCapture)?;
                reduced.entry(cite).or_insert_with(|| MemoryEvidence {
                    citation: cite,
                    source_date: source.source_date.clone(),
                    text: text.to_owned(),
                });
                refs.push(cite);
            }
            findings.push((
                verified.text[paragraph.start_byte as usize..paragraph.end_byte as usize]
                    .to_owned(),
                refs,
            ));
        }
        start = end;
    }
    let slots: BTreeMap<_, _> = reduced
        .keys()
        .enumerate()
        .map(|(slot, &cite)| (cite, slot))
        .collect();
    let analyses = findings
        .into_iter()
        .map(|(text, refs)| {
            let source_slots: Vec<_> = refs.iter().map(|reference| slots[reference]).collect();
            json!({"text": text, "source_slots": source_slots})
        })
        .collect();
    Ok(Some((reduced.into_values().collect(), analyses)))
}

#[cfg(test)]
#[path = "ego_phy_tests.rs"]
mod tests;
