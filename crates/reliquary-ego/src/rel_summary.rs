//! REL topic planning and independently verified point proposals over authorized evidence.

use crate::summary_evidence::{
    Judgment, Support, bind_support, object_schema, same_memory, same_revision,
};
pub use crate::summary_evidence::{MemoryEvidence, StructuredRequest};
use reliquary_memory_audit::SourceCitation;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fmt;

pub const TOPIC_SIMILARITY: f64 = 0.85;
const POINT_SHORTLIST_SIMILARITY: f64 = 0.85;
const POINT_SUPPRESSION_SIMILARITY: f64 = 0.92;
const MAX_POINTS: usize = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct Embedding {
    pub profile_id: [u8; 32],
    pub vector: Vec<f32>,
}

impl Embedding {
    fn similarity(&self, other: &Self) -> Option<f64> {
        if self.profile_id != other.profile_id || self.vector.len() != other.vector.len() {
            return None;
        }
        let (mut dot, mut left, mut right) = (0.0, 0.0, 0.0);
        for (&a, &b) in self.vector.iter().zip(&other.vector) {
            let (a, b) = (f64::from(a), f64::from(b));
            dot += a * b;
            left += a * a;
            right += b * b;
        }
        let score = dot / (left.sqrt() * right.sqrt());
        score.is_finite().then_some(score.clamp(-1.0, 1.0))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TopicProposal {
    pub title: String,
    pub embedding: Option<Embedding>,
    pub references: Vec<SourceCitation>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SavedTopic {
    pub id: [u8; 16],
    pub revision: u64,
    pub title: String,
    pub embedding: Option<Embedding>,
    pub references: Vec<SourceCitation>,
    pub suppressed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TopicTarget {
    pub id: [u8; 16],
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TopicWork {
    pub proposal: TopicProposal,
    pub matched: Option<TopicTarget>,
    pub new_references: Vec<SourceCitation>,
}

fn match_score(
    title: &str,
    embedding: Option<&Embedding>,
    other_title: &str,
    other_embedding: Option<&Embedding>,
) -> Option<f64> {
    if title == other_title {
        return Some(1.0);
    }
    embedding?
        .similarity(other_embedding?)
        .filter(|score| *score >= TOPIC_SIMILARITY)
}

pub fn consolidate_topics(proposals: Vec<TopicProposal>) -> Vec<TopicProposal> {
    let mut retained: Vec<TopicProposal> = Vec::new();
    for mut proposal in proposals {
        let matched = retained.iter_mut().find(|prior| {
            match_score(
                &proposal.title,
                proposal.embedding.as_ref(),
                &prior.title,
                prior.embedding.as_ref(),
            )
            .is_some()
        });
        let references = match matched {
            Some(prior) => {
                prior.references.append(&mut proposal.references);
                &mut prior.references
            }
            None => {
                retained.push(proposal);
                &mut retained.last_mut().expect("just inserted").references
            }
        };
        references.sort_unstable();
        references.dedup();
    }
    retained
}

/// Saved topics belong to the same authorized REL. Plain tombstones are omitted.
pub fn plan_topic(proposal: TopicProposal, saved: &[SavedTopic]) -> Option<TopicWork> {
    let mut best = None;
    for topic in saved {
        let Some(score) = match_score(
            &proposal.title,
            proposal.embedding.as_ref(),
            &topic.title,
            topic.embedding.as_ref(),
        ) else {
            continue;
        };
        if topic.suppressed {
            return None;
        }
        let score = (proposal.title == topic.title, score);
        if best.is_none_or(|(prior_score, prior): ((bool, f64), &SavedTopic)| {
            score > prior_score || (score == prior_score && topic.id < prior.id)
        }) {
            best = Some((score, topic));
        }
    }
    let prior = best.map(|(_, topic)| topic);
    let new_references: Vec<_> = proposal
        .references
        .iter()
        .copied()
        .filter(|reference| {
            prior.is_none_or(|prior| {
                !prior
                    .references
                    .iter()
                    .any(|old| same_revision(old, reference))
            })
        })
        .collect();
    (!new_references.is_empty()).then(|| TopicWork {
        proposal,
        matched: prior.map(|topic| TopicTarget {
            id: topic.id,
            revision: topic.revision,
        }),
        new_references,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Observed,
    Planned,
    Requested,
    Possible,
    Expected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeScope {
    Past,
    Current,
    Future,
    Unspecified,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExistingPoint {
    pub id: [u8; 16],
    pub revision: u64,
    pub statement: String,
    pub status: Status,
    pub time_scope: TimeScope,
    pub embedding: Option<Embedding>,
    pub citations: Vec<SourceCitation>,
    pub evidence: Vec<MemoryEvidence>,
}

impl ExistingPoint {
    fn support(&self) -> impl Iterator<Item = &MemoryEvidence> {
        self.evidence.iter().filter(|memory| {
            self.citations
                .iter()
                .any(|citation| memory.covers(citation))
        })
    }

    fn has_complete_support(&self) -> bool {
        !self.citations.is_empty()
            && self
                .citations
                .iter()
                .all(|citation| self.evidence.iter().any(|memory| memory.covers(citation)))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointTarget {
    pub id: [u8; 16],
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VerifiedPoint {
    pub statement: String,
    pub status: Status,
    pub time_scope: TimeScope,
    pub citations: Vec<SourceCitation>,
    pub embedding: Option<Embedding>,
    pub replacement: Option<PointTarget>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidEvidence,
    InvalidSynthesis,
    InvalidVerification,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidEvidence => "invalid REL summary evidence packet",
            Self::InvalidSynthesis => "invalid REL summary synthesis reply",
            Self::InvalidVerification => "invalid REL summary verification reply",
        })
    }
}

impl std::error::Error for Error {}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Point {
    statement: String,
    status: Status,
    time_scope: TimeScope,
    support: Vec<Support>,
}

pub struct DraftPoint {
    id: usize,
    point: Point,
    citations: Option<Vec<SourceCitation>>,
    pub embedding: Option<Embedding>,
}

impl DraftPoint {
    pub fn statement(&self) -> &str {
        &self.point.statement
    }
}

pub struct Synthesis<'a> {
    topic: &'a str,
    as_of: &'a str,
    evidence: &'a [MemoryEvidence],
}

impl<'a> Synthesis<'a> {
    pub fn new(
        topic: &'a str,
        as_of: &'a str,
        evidence: &'a [MemoryEvidence],
    ) -> Result<Self, Error> {
        if topic.trim().is_empty()
            || as_of.trim().is_empty()
            || evidence.is_empty()
            || evidence.iter().any(|memory| !memory.valid())
        {
            return Err(Error::InvalidEvidence);
        }
        Ok(Self {
            topic,
            as_of,
            evidence,
        })
    }

    fn packet(&self) -> Value {
        let memories: Vec<_> = self
            .evidence
            .iter()
            .enumerate()
            .map(|(slot, memory)| {
                json!({ "slot": slot, "source_date": memory.source_date, "text": memory.text })
            })
            .collect();
        json!({ "topic": self.topic, "as_of": self.as_of, "memories": memories })
    }

    pub fn request(&self) -> StructuredRequest {
        let support = object_schema(json!({
            "slot": {"type": "integer", "minimum": 0, "maximum": self.evidence.len() - 1},
            "quote": {"type": "string", "minLength": 1}
        }));
        let item = object_schema(json!({
            "statement": {"type": "string", "minLength": 1},
            "status": {"type": "string", "enum": ["observed","planned","requested","possible","expected"]},
            "time_scope": {"type": "string", "enum": ["past","current","future","unspecified"]},
            "support": {"type": "array", "minItems": 1, "items": support}
        }));
        StructuredRequest {
            instruction: include_str!("rel_synthesis_prompt.txt").into(),
            input: self.packet(),
            schema: object_schema(json!({
                "points": {"type": "array", "maxItems": MAX_POINTS, "items": item}
            })),
        }
    }

    pub fn parse(&self, reply: Value) -> Result<Vec<DraftPoint>, Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Reply {
            points: Vec<Point>,
        }

        let reply: Reply = serde_json::from_value(reply).map_err(|_| Error::InvalidSynthesis)?;
        if reply.points.len() > MAX_POINTS
            || reply
                .points
                .iter()
                .any(|point| point.statement.trim().is_empty())
        {
            return Err(Error::InvalidSynthesis);
        }
        Ok(reply
            .points
            .into_iter()
            .enumerate()
            .map(|(id, point)| DraftPoint {
                id,
                citations: bind_support(self.evidence, &point.support),
                point,
                embedding: None,
            })
            .collect())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Check {
    point_id: usize,
    meaning: Judgment,
    status: Judgment,
    time_scope: Judgment,
    attribution: Judgment,
    hold_reason: String,
}

impl Check {
    fn passes(&self) -> bool {
        [self.meaning, self.status, self.time_scope, self.attribution]
            .iter()
            .all(|judgment| *judgment == Judgment::Supported)
    }

    fn valid(&self) -> bool {
        if self.passes() {
            self.hold_reason.is_empty()
        } else {
            !self.hold_reason.trim().is_empty()
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Relationship {
    Replaces,
    Independent,
    Uncertain,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplacementCheck {
    point_id: usize,
    candidate_id: usize,
    predecessor_id: [u8; 16],
    predecessor_revision: u64,
    relationship: Relationship,
}

struct Candidate<'a> {
    point: &'a ExistingPoint,
    vector: bool,
}

pub struct Verification<'a> {
    synthesis: &'a Synthesis<'a>,
    work: &'a TopicWork,
    drafts: Vec<DraftPoint>,
    candidate_lists: Vec<Vec<Candidate<'a>>>,
    suppressed: &'a [ExistingPoint],
}

impl<'a> Verification<'a> {
    pub fn new(
        synthesis: &'a Synthesis<'a>,
        work: &'a TopicWork,
        drafts: Vec<DraftPoint>,
        live: &'a [ExistingPoint],
        suppressed: &'a [ExistingPoint],
    ) -> Self {
        let candidate_lists = drafts
            .iter()
            .map(|draft| {
                if work.matched.is_none() || !has_new_support(draft, work) {
                    return Vec::new();
                }
                shortlist(draft, live)
            })
            .collect();
        Self {
            synthesis,
            work,
            drafts,
            candidate_lists,
            suppressed,
        }
    }

    pub fn request(&self) -> StructuredRequest {
        let mut input = self.synthesis.packet();
        input["points"] = Value::Array(
            self.drafts
                .iter()
                .map(|draft| {
                    let mut point = json!(draft.point);
                    point["point_id"] = json!(draft.id);
                    point
                })
                .collect(),
        );
        let judgment = json!({"type": "string", "enum": ["supported","unsupported","uncertain"]});
        let check = object_schema(json!({
            "point_id": {"type": "integer", "minimum": 0},
            "meaning": judgment, "status": judgment, "time_scope": judgment, "attribution": judgment,
            "hold_reason": {"type": "string"}
        }));
        let mut properties = json!({
            "checks": {"type": "array", "items": check,
                "minItems": self.drafts.len(), "maxItems": self.drafts.len()}
        });
        let mut instruction = include_str!("rel_verification_prompt.txt").to_owned();
        let candidates = self.replacement_packet();
        if !candidates.is_empty() {
            let replacement_check = object_schema(json!({
                "point_id": {"type": "integer", "minimum": 0},
                "candidate_id": {"type": "integer", "minimum": 0},
                "predecessor_id": {"type": "array", "minItems": 16, "maxItems": 16,
                    "items": {"type": "integer", "minimum": 0, "maximum": 255}},
                "predecessor_revision": {"type": "integer", "minimum": 1},
                "relationship": {"type": "string", "enum": ["replaces","independent","uncertain"]}
            }));
            properties["replacement_checks"] = json!({"type": "array", "items": replacement_check,
                "minItems": candidates.len(), "maxItems": candidates.len()});
            input["replacement_candidates"] = json!(candidates);
            instruction.push_str(
                "\n\nSeparately return one replacement check per supplied candidate, copying its point_id, \
                candidate_id, predecessor_id and predecessor_revision exactly. Judge whether the new \
                point replaces the same assertion using its cited new evidence, states an independently \
                applicable claim, or leaves that relationship uncertain. Use replaces only when its \
                new evidence updates or corrects the predecessor for the same subject and applicability. \
                A narrower exception or different scope is independent. Similar wording and shared \
                Memory identity alone do not establish a replacement. The predecessor and passages are \
                available only for this comparison. Continue to judge the four point dimensions using \
                the supplied Memories and its cited support."
            );
        }
        StructuredRequest {
            instruction,
            input,
            schema: object_schema(properties),
        }
    }

    fn replacement_packet(&self) -> Vec<Value> {
        self.drafts
            .iter()
            .zip(&self.candidate_lists)
            .flat_map(|(draft, candidates)| {
                candidates.iter().enumerate().map(move |(candidate_id, candidate)| {
                    let predecessor = candidate.point;
                    let passages: Vec<_> = predecessor
                        .support()
                        .map(|memory| json!({"source_date": memory.source_date, "text": memory.text}))
                        .collect();
                    let new_support: Vec<_> = draft
                        .point
                        .support
                        .iter()
                        .filter(|item| {
                            self.synthesis.evidence.get(item.slot).is_some_and(|memory| {
                                self.work.new_references.iter().any(|reference| {
                                    same_revision(reference, &memory.citation)
                                })
                            })
                        })
                        .map(|item| item.slot)
                        .collect();
                    json!({
                        "point_id": draft.id, "candidate_id": candidate_id,
                        "predecessor_id": predecessor.id, "predecessor_revision": predecessor.revision,
                        "statement": predecessor.statement, "status": predecessor.status,
                        "time_scope": predecessor.time_scope, "passages": passages,
                        "new_support_slots": new_support
                    })
                })
            })
            .collect()
    }

    pub fn accept(self, reply: Value) -> Result<Vec<VerifiedPoint>, Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Reply {
            checks: Vec<Check>,
            #[serde(default)]
            replacement_checks: Option<Vec<ReplacementCheck>>,
        }

        if reply.get("replacement_checks").is_some_and(Value::is_null) {
            return Err(Error::InvalidVerification);
        }
        let reply: Reply = serde_json::from_value(reply).map_err(|_| Error::InvalidVerification)?;
        if reply.checks.len() != self.drafts.len() {
            return Err(Error::InvalidVerification);
        }
        let mut checks = BTreeMap::new();
        for check in reply.checks {
            if check.point_id >= self.drafts.len()
                || !check.valid()
                || checks.insert(check.point_id, check).is_some()
            {
                return Err(Error::InvalidVerification);
            }
        }
        let relationships = validate_replacements(&self.candidate_lists, reply.replacement_checks)?;
        let mut accepted = Vec::new();
        for ((draft, candidates), relationships) in self
            .drafts
            .into_iter()
            .zip(self.candidate_lists)
            .zip(relationships)
        {
            if !checks[&draft.id].passes()
                || !has_new_support(&draft, self.work)
                || self
                    .suppressed
                    .iter()
                    .any(|point| suppresses(&draft, point))
            {
                continue;
            }
            let Some(replacement) = choose_replacement(&candidates, &relationships) else {
                continue;
            };
            if let Some(citations) = draft.citations {
                accepted.push(VerifiedPoint {
                    statement: draft.point.statement,
                    status: draft.point.status,
                    time_scope: draft.point.time_scope,
                    citations,
                    embedding: draft.embedding,
                    replacement,
                });
            }
        }
        let mut targets = BTreeMap::new();
        for target in accepted.iter().filter_map(|point| point.replacement) {
            *targets.entry(target.id).or_insert(0) += 1;
        }
        // Two drafts may not independently advance the same predecessor.
        accepted.retain(|point| {
            point
                .replacement
                .is_none_or(|target| targets[&target.id] == 1)
        });
        Ok(accepted)
    }
}

fn has_new_support(draft: &DraftPoint, work: &TopicWork) -> bool {
    draft.citations.as_ref().is_some_and(|citations| {
        work.matched.is_none()
            || citations.iter().any(|citation| {
                work.new_references
                    .iter()
                    .any(|reference| same_revision(citation, reference))
            })
    })
}

fn shortlist<'a>(draft: &DraftPoint, live: &'a [ExistingPoint]) -> Vec<Candidate<'a>> {
    let Some(citations) = &draft.citations else {
        return Vec::new();
    };
    let mut candidates: Vec<_> = live
        .iter()
        .filter_map(|point| {
            let vector = draft
                .embedding
                .as_ref()
                .zip(point.embedding.as_ref())
                .and_then(|(a, b)| a.similarity(b))
                .is_some_and(|score| score >= POINT_SHORTLIST_SIMILARITY);
            let shared_memory = point.citations.iter().any(|prior| {
                citations
                    .iter()
                    .any(|citation| same_memory(prior, citation))
            });
            (vector || shared_memory).then_some(Candidate { point, vector })
        })
        .collect();
    candidates.sort_unstable_by_key(|candidate| (!candidate.vector, candidate.point.id));
    candidates
}

fn validate_replacements(
    lists: &[Vec<Candidate<'_>>],
    checks: Option<Vec<ReplacementCheck>>,
) -> Result<Vec<Vec<Relationship>>, Error> {
    let count = lists.iter().map(Vec::len).sum::<usize>();
    let checks = match (count, checks) {
        (0, None) => Vec::new(),
        (n, Some(checks)) if n > 0 && checks.len() == n => checks,
        _ => return Err(Error::InvalidVerification),
    };
    let mut relationships = BTreeMap::new();
    for check in checks {
        let point = lists
            .get(check.point_id)
            .and_then(|candidates| candidates.get(check.candidate_id))
            .ok_or(Error::InvalidVerification)?
            .point;
        if point.id != check.predecessor_id
            || point.revision != check.predecessor_revision
            || relationships
                .insert((check.point_id, check.candidate_id), check.relationship)
                .is_some()
        {
            return Err(Error::InvalidVerification);
        }
    }
    Ok(lists
        .iter()
        .enumerate()
        .map(|(point_id, list)| {
            (0..list.len())
                .map(|id| relationships[&(point_id, id)])
                .collect()
        })
        .collect())
}

/// A held comparison has no outcome. An independent point has no target.
fn choose_replacement(
    candidates: &[Candidate<'_>],
    relationships: &[Relationship],
) -> Option<Option<PointTarget>> {
    for vector in [true, false] {
        let mut target = None;
        for (candidate, relationship) in candidates.iter().zip(relationships) {
            if candidate.vector != vector {
                continue;
            }
            match relationship {
                Relationship::Independent => {}
                Relationship::Uncertain => return None,
                Relationship::Replaces
                    if target.is_some() || !candidate.point.has_complete_support() =>
                {
                    return None;
                }
                Relationship::Replaces => {
                    target = Some(PointTarget {
                        id: candidate.point.id,
                        revision: candidate.point.revision,
                    });
                }
            }
        }
        if target.is_some() {
            return Some(target);
        }
    }
    Some(None)
}

fn suppresses(draft: &DraftPoint, retained: &ExistingPoint) -> bool {
    if draft.point.status != retained.status || draft.point.time_scope != retained.time_scope {
        return false;
    }
    if draft.point.statement == retained.statement {
        return true;
    }
    let similar = draft
        .embedding
        .as_ref()
        .zip(retained.embedding.as_ref())
        .and_then(|(a, b)| a.similarity(b))
        .is_some_and(|score| score >= POINT_SUPPRESSION_SIMILARITY);
    similar && suppression_terms(&draft.point.statement) == suppression_terms(&retained.statement)
}

fn suppression_terms(statement: &str) -> Vec<&str> {
    // Preserve order, negation, values and qualifiers. Vector proximity alone
    // cannot establish matching applicability.
    statement
        .split_whitespace()
        .filter_map(|word| match word.trim_end_matches(['.', ',', ';', ':']) {
            "" | "the" | "The" => None,
            "needs" => Some("requires"),
            "mandatory" => Some("required"),
            term => Some(term),
        })
        .collect()
}

#[cfg(test)]
#[path = "rel_summary_tests.rs"]
mod tests;
