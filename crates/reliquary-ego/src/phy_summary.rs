//! PHY summary policy over an authorized incremental capture.

use crate::summary_evidence::{Judgment, Support, bind_support, object_schema};
pub use crate::summary_evidence::{MemoryEvidence, StructuredRequest};
use reliquary_memory_audit::{SourceCitation, SourceId};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const DEFAULT_TOKEN_CAP: u64 = 4_000;
pub const MAX_TOKENIZER_RESULTS: usize = 4;
pub const TOKENIZER_TOOL: &str = "measure_summary_tokens";

const DRAFT_INSTRUCTION: &str = "Write the whole current PHY profile summary from the authorized capture. \
Start from the previous verified paragraphs on a refresh. Use the changes and supporting passages to \
correct affected claims, remove unsupported claims after source withdrawal, and retain applicable \
standing guidance and narrower exceptions. Only the evidence text and explicitly supplied source \
metadata establish new facts. Preserve effective dates, qualifiers, scope, and uncertainty. Order \
the most broadly useful current preferences and standing guidance first. Use independently meaningful \
paragraphs without leading or trailing whitespace and without internal blank lines. For each paragraph \
list short, exact, unique contiguous quotes and their source slots. Support may be empty only for a \
byte-identical previous paragraph marked can_carry. Return an empty paragraph list only when no \
supported useful material remains.";

const COMPRESS_INSTRUCTION: &str = "Compress the supplied draft into independently meaningful \
paragraphs with the most broadly useful current preferences and standing guidance first. Preserve \
its material facts, uncertainty, dates, scope, conditions, and narrower exceptions. Do not add \
claims. Bind each rewritten paragraph to short exact source quotes and their slots. Support may be \
empty only for a byte-identical previous paragraph marked can_carry. The final text is the exact \
paragraph texts joined by two line feeds with nothing before or after. The tokenizer tool measures \
that final text. You may receive at most four tokenizer results before submitting one terminal \
structured response. Fit within the supplied token cap and return only the structured paragraphs.";

const VERIFY_INSTRUCTION: &str = "Verify the exact final PHY summary against the authorized capture \
and return one check for every paragraph ID. Judge meaning, time, scope, and attribution independently. \
For new or rewritten paragraphs, ground every material detail in its cited Memories, using their \
surrounding captured text as needed. An uncited Memory can expose a conflict but cannot supply missing \
support. The as_of date supplies a reference for timing and no facts. For byte-identical carried \
paragraphs, the prior verified provenance is already bound and eligible in this capture. Still \
consider conflicts in the captured changes and evidence. Check summary continuity independently: \
the draft must retain applicable prior material, account for withdrawals and corrections within \
their scope, and preserve valid narrower exceptions. An empty summary is supported only if no \
supported useful material remains. Return supported, unsupported, or uncertain for each judgment. \
Give an empty hold_reason when a check passes and a short reason when it does not. Do not rewrite \
any text.";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriorSummary {
    pub text: String,
    pub paragraphs: Vec<PriorParagraph>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriorParagraph {
    pub start_byte: u32,
    pub end_byte: u32,
    pub citations: Vec<SourceCitation>,
    /// The caller has checked exact current source standing and access.
    pub can_carry: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Revised,
    Withdrawn,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct MemoryChange {
    pub memory_id: [u8; 32],
    pub kind: ChangeKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedParagraph {
    pub start_byte: u32,
    pub end_byte: u32,
    pub citations: Vec<SourceCitation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedSummary {
    pub text: String,
    pub paragraphs: Vec<VerifiedParagraph>,
    pub token_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidCapture,
    InvalidBudget,
    InvalidDraft,
    InvalidCitation,
    InvalidTokenizerInput,
    CompressionExhausted,
    InvalidVerification,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidCapture => "invalid PHY summary capture",
            Self::InvalidBudget => "invalid PHY summary token budget",
            Self::InvalidDraft => "invalid PHY summary draft",
            Self::InvalidCitation => "invalid PHY summary draft citation",
            Self::InvalidTokenizerInput => "invalid PHY summary tokenizer input",
            Self::CompressionExhausted => "PHY summary tokenizer result limit exhausted",
            Self::InvalidVerification => "invalid PHY summary verification reply",
        })
    }
}

impl std::error::Error for Error {}

pub struct Synthesis<'a> {
    as_of: &'a str,
    prior: Option<&'a PriorSummary>,
    changes: BTreeMap<[u8; 32], ChangeKind>,
    evidence: &'a [MemoryEvidence],
}

impl<'a> Synthesis<'a> {
    pub fn new(
        as_of: &'a str,
        prior: Option<&'a PriorSummary>,
        changes: &[MemoryChange],
        evidence: &'a [MemoryEvidence],
    ) -> Result<Self, Error> {
        let by_memory: BTreeMap<_, _> = changes.iter().map(|c| (c.memory_id, c.kind)).collect();
        if as_of.trim().is_empty()
            || by_memory.len() != changes.len()
            || prior.is_some_and(|summary| !valid_prior(summary))
            || !same_scope(prior, evidence)
            || evidence.iter().any(|memory| {
                !memory.valid()
                    || memory_id(&memory.citation)
                        .is_some_and(|id| by_memory.get(&id) == Some(&ChangeKind::Withdrawn))
            })
        {
            return Err(Error::InvalidCapture);
        }
        Ok(Self {
            as_of,
            prior,
            changes: by_memory,
            evidence,
        })
    }

    pub fn request(&self) -> StructuredRequest {
        StructuredRequest {
            instruction: DRAFT_INSTRUCTION.into(),
            input: self.packet(),
            schema: draft_schema(),
        }
    }

    pub fn parse(&self, reply: Value) -> Result<Draft, Error> {
        let wire: DraftWire = serde_json::from_value(reply).map_err(|_| Error::InvalidDraft)?;
        let mut text = String::new();
        let mut paragraphs = Vec::with_capacity(wire.paragraphs.len());
        for paragraph in wire.paragraphs {
            if !valid_paragraph(&paragraph.text) {
                return Err(Error::InvalidDraft);
            }
            let (citations, carry_from) = if paragraph.support.is_empty() {
                let (index, prior) = self.carry(&paragraph.text).ok_or(Error::InvalidCitation)?;
                (prior.citations.clone(), Some(index))
            } else {
                (
                    bind_support(self.evidence, &paragraph.support)
                        .ok_or(Error::InvalidCitation)?,
                    None,
                )
            };
            if !text.is_empty() {
                text.push_str("\n\n");
            }
            let start_byte = u32::try_from(text.len()).map_err(|_| Error::InvalidDraft)?;
            text.push_str(&paragraph.text);
            let end_byte = u32::try_from(text.len()).map_err(|_| Error::InvalidDraft)?;
            paragraphs.push(BoundParagraph {
                wire: paragraph,
                final_span: VerifiedParagraph {
                    start_byte,
                    end_byte,
                    citations,
                },
                carry_from,
            });
        }
        Ok(Draft { text, paragraphs })
    }

    pub fn measure<'b>(&'b self, draft: Draft, budget: TokenBudget<'b>) -> MeasuredDraft<'b> {
        if budget.count(&draft.text) <= budget.cap {
            MeasuredDraft::Fits(FinalDraft {
                synthesis: self,
                draft,
                uncompressed: None,
                budget,
            })
        } else {
            MeasuredDraft::Compress(Compression {
                synthesis: self,
                original: draft,
                budget,
                results: 0,
                exhausted: false,
            })
        }
    }

    fn can_carry(&self, prior: &PriorParagraph) -> bool {
        prior.can_carry
            && prior.citations.iter().all(|citation| {
                memory_id(citation).is_some_and(|id| {
                    !matches!(
                        self.changes.get(&id),
                        Some(ChangeKind::Revised | ChangeKind::Withdrawn)
                    )
                })
            })
    }

    fn carry(&self, text: &str) -> Option<(usize, &PriorParagraph)> {
        let prior = self.prior?;
        prior.paragraphs.iter().enumerate().find(|(_, paragraph)| {
            self.can_carry(paragraph)
                && paragraph_text(&prior.text, paragraph.start_byte, paragraph.end_byte)
                    == Some(text)
        })
    }

    fn packet(&self) -> Value {
        let mut memories = BTreeMap::new();
        let all_sources =
            self.evidence
                .iter()
                .map(|memory| &memory.citation)
                .chain(self.prior.iter().flat_map(|prior| {
                    prior
                        .paragraphs
                        .iter()
                        .flat_map(|paragraph| &paragraph.citations)
                }));
        for id in all_sources
            .filter_map(memory_id)
            .chain(self.changes.keys().copied())
        {
            let slot = memories.len();
            memories.entry(id).or_insert(slot);
        }
        let evidence: Vec<_> = self
            .evidence
            .iter()
            .enumerate()
            .map(|(slot, memory)| {
                json!({
                    "slot": slot, "memory": memory_id(&memory.citation).map(|id| memories[&id]),
                    "revision": memory.citation.revision, "source_date": memory.source_date,
                    "text": memory.text
                })
            })
            .collect();
        let prior = self.prior.map(|prior| {
            let paragraphs: Vec<_> = prior.paragraphs.iter().enumerate().map(|(id, paragraph)| {
                let references: Vec<_> = paragraph.citations.iter().map(|citation| {
                    json!({
                        "memory": memory_id(citation).map(|id| memories[&id]),
                        "revision": citation.revision, "field": citation.field,
                        "start_byte": citation.start_byte, "end_byte": citation.end_byte
                    })
                }).collect();
                json!({
                    "paragraph_id": id,
                    "text": paragraph_text(&prior.text, paragraph.start_byte, paragraph.end_byte),
                    "references": references, "can_carry": self.can_carry(paragraph)
                })
            }).collect();
            json!({"text": prior.text, "paragraphs": paragraphs})
        });
        let changes: Vec<_> = self
            .changes
            .iter()
            .map(|(id, kind)| json!({"memory": memories[id], "kind": kind}))
            .collect();
        json!({"as_of": self.as_of, "previous": prior, "changes": changes, "memories": evidence})
    }
}

fn valid_prior(prior: &PriorSummary) -> bool {
    let mut end = 0;
    for (i, span) in prior.paragraphs.iter().enumerate() {
        let start = span.start_byte as usize;
        if (i == 0 && start != 0)
            || (i > 0 && prior.text.get(end..start) != Some("\n\n"))
            || !paragraph_text(&prior.text, span.start_byte, span.end_byte)
                .is_some_and(valid_paragraph)
            || span.citations.is_empty()
            || span.citations.iter().any(|citation| {
                memory_id(citation).is_none() || citation.start_byte >= citation.end_byte
            })
        {
            return false;
        }
        end = span.end_byte as usize;
    }
    end == prior.text.len()
}

fn same_scope(prior: Option<&PriorSummary>, evidence: &[MemoryEvidence]) -> bool {
    let references = evidence
        .iter()
        .map(|memory| &memory.citation)
        .chain(prior.iter().flat_map(|summary| {
            summary
                .paragraphs
                .iter()
                .flat_map(|paragraph| &paragraph.citations)
        }));
    let mut scopes = references.map(|citation| (citation.owner_id, citation.history_generation));
    scopes
        .next()
        .is_none_or(|first| scopes.all(|scope| scope == first))
}

fn valid_paragraph(text: &str) -> bool {
    !text.is_empty()
        && text.trim() == text
        && !text.contains('\r')
        && text.lines().all(|line| !line.trim().is_empty())
}

fn paragraph_text(text: &str, start_byte: u32, end_byte: u32) -> Option<&str> {
    text.get(start_byte as usize..end_byte as usize)
}

fn memory_id(citation: &SourceCitation) -> Option<[u8; 32]> {
    match citation.source {
        SourceId::Memory(id) => Some(id),
        SourceId::Observation(_) => None,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DraftWire {
    paragraphs: Vec<ParagraphWire>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ParagraphWire {
    text: String,
    support: Vec<Support>,
}

struct BoundParagraph {
    wire: ParagraphWire,
    final_span: VerifiedParagraph,
    carry_from: Option<usize>,
}

pub struct Draft {
    text: String,
    paragraphs: Vec<BoundParagraph>,
}

impl Draft {
    fn packet(&self) -> Value {
        let paragraphs: Vec<_> = self.paragraphs.iter().enumerate().map(|(id, paragraph)| {
            json!({
                "paragraph_id": id, "text": paragraph.wire.text, "support": paragraph.wire.support,
                "carry_from": paragraph.carry_from
            })
        }).collect();
        json!({"text": self.text, "paragraphs": paragraphs})
    }
}

fn draft_schema() -> Value {
    let support = object_schema(json!({
        "slot": {"type": "integer", "minimum": 0},
        "quote": {"type": "string", "minLength": 1}
    }));
    let paragraph = object_schema(json!({
        "text": {"type": "string", "minLength": 1},
        "support": {"type": "array", "items": support}
    }));
    object_schema(json!({"paragraphs": {"type": "array", "items": paragraph}}))
}

#[derive(Clone, Copy)]
pub struct TokenBudget<'a> {
    cap: u64,
    count: &'a (dyn Fn(&str) -> u64 + Sync),
}

impl<'a> TokenBudget<'a> {
    pub fn new(cap: u64, count: &'a (dyn Fn(&str) -> u64 + Sync)) -> Result<Self, Error> {
        if cap == 0 {
            return Err(Error::InvalidBudget);
        }
        Ok(Self { cap, count })
    }

    fn count(&self, text: &str) -> u64 {
        (self.count)(text)
    }
}

pub enum MeasuredDraft<'a> {
    Fits(FinalDraft<'a>),
    Compress(Compression<'a>),
}

pub struct CompressionRequest {
    pub structured: StructuredRequest,
    pub tool_name: &'static str,
    pub tool_schema: Value,
}

pub struct Compression<'a> {
    synthesis: &'a Synthesis<'a>,
    original: Draft,
    budget: TokenBudget<'a>,
    results: usize,
    exhausted: bool,
}

impl<'a> Compression<'a> {
    pub fn request(&self) -> CompressionRequest {
        let mut input = self.synthesis.packet();
        input["draft"] = self.original.packet();
        input["token_cap"] = json!(self.budget.cap);
        CompressionRequest {
            structured: StructuredRequest {
                instruction: COMPRESS_INSTRUCTION.into(),
                input,
                schema: draft_schema(),
            },
            tool_name: TOKENIZER_TOOL,
            tool_schema: object_schema(json!({"text": {"type": "string"}})),
        }
    }

    pub fn tokenize(&mut self, arguments: Value) -> Result<Value, Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            text: String,
        }

        if self.results >= MAX_TOKENIZER_RESULTS {
            self.exhausted = true;
            return Err(Error::CompressionExhausted);
        }
        let input: Input =
            serde_json::from_value(arguments).map_err(|_| Error::InvalidTokenizerInput)?;
        self.results += 1;
        let tokens = self.budget.count(&input.text);
        Ok(json!({"tokens": tokens, "cap": self.budget.cap, "fits": tokens <= self.budget.cap}))
    }

    pub fn finish(self, reply: Value) -> Result<Option<FinalDraft<'a>>, Error> {
        if self.exhausted {
            return Ok(None);
        }
        let draft = self.synthesis.parse(reply)?;
        if self.budget.count(&draft.text) > self.budget.cap {
            return Ok(None);
        }
        Ok(Some(FinalDraft {
            synthesis: self.synthesis,
            draft,
            uncompressed: Some(self.original),
            budget: self.budget,
        }))
    }
}

pub struct FinalDraft<'a> {
    synthesis: &'a Synthesis<'a>,
    draft: Draft,
    uncompressed: Option<Draft>,
    budget: TokenBudget<'a>,
}

impl<'a> FinalDraft<'a> {
    pub fn verification(self) -> Verification<'a> {
        Verification { candidate: self }
    }
}

pub struct Verification<'a> {
    candidate: FinalDraft<'a>,
}

impl Verification<'_> {
    pub fn request(&self) -> StructuredRequest {
        let mut instruction = VERIFY_INSTRUCTION.to_owned();
        let mut input = self.candidate.synthesis.packet();
        input["final_summary"] = self.candidate.draft.packet();
        let judgment = json!({"type": "string", "enum": ["supported", "unsupported", "uncertain"]});
        let explanation = json!({"type": "string"});
        let paragraph = object_schema(json!({
            "paragraph_id": {"type": "integer", "minimum": 0},
            "meaning": judgment, "time": judgment, "scope": judgment, "attribution": judgment,
            "hold_reason": explanation
        }));
        let whole = object_schema(json!({"judgment": judgment, "hold_reason": explanation}));
        let count = self.candidate.draft.paragraphs.len();
        let mut properties = json!({
            "checks": {"type": "array", "minItems": count, "maxItems": count, "items": paragraph},
            "continuity": whole
        });
        if let Some(original) = &self.candidate.uncompressed {
            input["uncompressed_draft"] = original.packet();
            properties["preservation"] = whole;
            instruction.push_str(
                "\nAlso check preservation against the exact uncompressed draft. Mark it unsupported \
                or uncertain if compression dropped or changed any material information, including \
                effective dates, conditions, qualifiers, or scope. Do not penalize shorter wording \
                that preserves the same information.",
            );
        }
        StructuredRequest {
            instruction,
            input,
            schema: object_schema(properties),
        }
    }

    pub fn accept(self, reply: Value) -> Result<Option<VerifiedSummary>, Error> {
        let compressed = self.candidate.uncompressed.is_some();
        if reply.get("preservation").is_some() != compressed
            || reply.get("preservation").is_some_and(Value::is_null)
        {
            return Err(Error::InvalidVerification);
        }
        let reply: VerificationReply =
            serde_json::from_value(reply).map_err(|_| Error::InvalidVerification)?;
        let draft = self.candidate.draft;
        if reply.checks.len() != draft.paragraphs.len()
            || !reply.continuity.valid()
            || reply
                .preservation
                .as_ref()
                .is_some_and(|check| !check.valid())
        {
            return Err(Error::InvalidVerification);
        }
        let mut ids = BTreeSet::new();
        for check in &reply.checks {
            if check.paragraph_id >= draft.paragraphs.len()
                || !ids.insert(check.paragraph_id)
                || !valid_reason(check.passes(), &check.hold_reason)
            {
                return Err(Error::InvalidVerification);
            }
        }
        let token_count = self.candidate.budget.count(&draft.text);
        if token_count > self.candidate.budget.cap
            || !reply.continuity.passes()
            || reply
                .preservation
                .as_ref()
                .is_some_and(|check| !check.passes())
            || reply.checks.iter().any(|check| !check.passes())
        {
            return Ok(None);
        }
        Ok(Some(VerifiedSummary {
            text: draft.text,
            paragraphs: draft.paragraphs.into_iter().map(|p| p.final_span).collect(),
            token_count,
        }))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VerificationReply {
    checks: Vec<ParagraphCheck>,
    continuity: WholeCheck,
    preservation: Option<WholeCheck>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ParagraphCheck {
    paragraph_id: usize,
    meaning: Judgment,
    time: Judgment,
    scope: Judgment,
    attribution: Judgment,
    hold_reason: String,
}

impl ParagraphCheck {
    fn passes(&self) -> bool {
        [self.meaning, self.time, self.scope, self.attribution]
            .iter()
            .all(|&judgment| judgment == Judgment::Supported)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WholeCheck {
    judgment: Judgment,
    hold_reason: String,
}

impl WholeCheck {
    fn passes(&self) -> bool {
        self.judgment == Judgment::Supported
    }

    fn valid(&self) -> bool {
        valid_reason(self.passes(), &self.hold_reason)
    }
}

fn valid_reason(passes: bool, reason: &str) -> bool {
    if passes {
        reason.is_empty()
    } else {
        !reason.trim().is_empty()
    }
}

#[cfg(test)]
#[path = "phy_summary_tests.rs"]
mod tests;
