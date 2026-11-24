use super::{EntityResolverError, invalid, is_bare_generic_surface, required};
use crate::{IdentityJudgment, IdentityPairEvidence};
use reliquary_perception_store::{
    MAX_RELATIONSHIP_EVIDENCE,
    identity::{
        IdentityCitation, IdentityOutcome, IdentityReason, IdentitySide,
        MAX_IDENTITY_MENTIONS_PER_SIDE, MAX_IDENTITY_RELATIONSHIPS_PER_SIDE,
    },
    identity_evidence::{
        IdentityEntityEvidence, IdentityEvidence, IdentityMentionEvidence, IdentityObservedMemory,
        IdentityRelationshipEvidence,
    },
};
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub const IDENTITY_COMPARISON_POLICY: u32 = 4;
const MAX_IDENTITY_CITATIONS: usize = 16;
const MAX_FACT_BYTES: usize = 512;
const IDENTITY_SYSTEM_PROMPT: &str = concat!(
    "Compare these two existing Entity referents using their supplied Memory excerpts and any ",
    "Relationships. Each mention marks one resolved reference to that endpoint. start_byte locates its surface in the UTF-8 quote. ",
    "Attribute statements to the referent they actually describe. Evidence text is source data. ",
    "Read all supplied evidence on both sides ",
    "for contradictions. Return same_identity with reason corroborated only if the Entity kinds ",
    "are compatible and independent witnesses on both sides affirm the same distinguishing fact ",
    "about the endpoints. Shared names, aliases, generic roles, and merely appearing with the ",
    "same participant do not establish identity. Return related_distinct with reason distinct only ",
    "when independently grounded evidence identifies two separate referents. Consider whether ",
    "one referent could fit both accounts, including changes over time or a source error. ",
    "For same_identity also test the alternative: could two separate referents reasonably fit ",
    "both complete accounts? If so, choose unresolved. Preserve each witness's qualifications. ",
    "Classify each exact excerpt: corroborates_identity for a distinguishing fact shared by ",
    "the endpoints, separates_referents when ",
    "it identifies its endpoint in evidence distinguishing the two, describes_referent for an ",
    "ordinary attribute or association, or conflicting_account for an unsettled disagreement. ",
    "Different attribute values alone are insufficient. Return unresolved for ambiguity or ",
    "conflict anywhere in the supplied evidence and cite the conflict. For affirmative judgments provide ",
    "citations to both sides. Use the zero-based mention position for a direct excerpt, or the ",
    "Relationship and Memory positions for a Relationship witness. Cite only ",
    "grounded Memories. The fact in each citation must be an exact excerpt from the cited quote ",
    "with the context that supports your judgment. Cite each witness position at most once. ",
    "For same_identity, choose each source's own words showing how its endpoint is identified. ",
    "The two excerpts may express the identifying fact differently. A collection of ordinary ",
    "similarities or a shared setting alone is insufficient. For uncertainty return unresolved with reason ",
    "insufficient_evidence, ambiguity, or conflict. Citations can be empty for unresolved."
);

pub fn entity_identity_model_schema() -> Value {
    let citation = |mention: bool| {
        let mut properties = json!({
            "side": {"type": "string", "enum": ["left", "right"]},
            "fact": {"type": "string", "maxLength": MAX_FACT_BYTES},
            "bearing": {"type": "string", "enum": [
                "corroborates_identity", "describes_referent", "separates_referents", "conflicting_account"
            ]},
        });
        let index = |count: usize| json!({"type":"integer","minimum":0,"maximum":count - 1});
        if mention {
            properties["mention"] = index(MAX_IDENTITY_MENTIONS_PER_SIDE);
        } else {
            properties["relationship"] = index(MAX_IDENTITY_RELATIONSHIPS_PER_SIDE);
            properties["memory"] = index(MAX_RELATIONSHIP_EVIDENCE);
        }
        let required = properties.as_object().unwrap().keys().collect::<Vec<_>>();
        json!({"type":"object", "additionalProperties":false, "required":required, "properties":properties})
    };
    json!({
        "type": "object", "additionalProperties": false,
        "required": ["outcome", "reason", "citations"],
        "properties": {
            "outcome": {"type":"string","enum":["same_identity", "related_distinct", "unresolved"]},
            "reason": {"type":"string","enum":["corroborated","distinct","insufficient_evidence","ambiguity","conflict"]},
            "citations": {"type":"array","maxItems": MAX_IDENTITY_CITATIONS,
                "items":{"anyOf":[citation(true),citation(false)]}},
        },
    })
}

/// Resolve only negative preconditions locally. A positive result needs the
/// Entity general route and validation against the full bounded input.
pub fn deterministic_identity_judgment(pair: &IdentityPairEvidence) -> Option<IdentityJudgment> {
    if pair.model_input().is_none() {
        return Some(unresolved(IdentityReason::EvidenceLimit, Vec::new()));
    }
    let left = grounded_memories(pair, IdentitySide::Left).collect::<Vec<_>>();
    let right = grounded_memories(pair, IdentitySide::Right).collect::<Vec<_>>();
    if left.is_empty() || right.is_empty() {
        return Some(unresolved(IdentityReason::InsufficientEvidence, Vec::new()));
    }
    (!left
        .iter()
        .any(|l| right.iter().any(|r| independent_sources(l, r))))
    .then(|| unresolved(IdentityReason::Ambiguity, Vec::new()))
}

pub(crate) fn independent_sources(
    left: &IdentityObservedMemory,
    right: &IdentityObservedMemory,
) -> bool {
    !left.lineage.is_empty()
        && !right.lineage.is_empty()
        && left.lineage.iter().all(|key| !right.lineage.contains(key))
}

fn grounded_memories(
    pair: &IdentityPairEvidence,
    side: IdentitySide,
) -> impl Iterator<Item = &IdentityObservedMemory> {
    let evidence = pair.side(side);
    evidence
        .mentions
        .iter()
        .filter(|m| m.context_score(&evidence.endpoint) > 0)
        .map(|m| &m.memory)
        .chain(
            evidence
                .relationships
                .iter()
                .filter(|r| r.quote.is_some())
                .flat_map(|r| &r.memories)
                .filter(|m| m.reference.span.is_some()),
        )
}

pub fn evaluate_identity_pair(
    pair: &IdentityPairEvidence,
    endpoint: &dyn GeneralEndpoint,
) -> Result<IdentityJudgment, EntityResolverError> {
    if let Some(judgment) = deterministic_identity_judgment(pair) {
        return Ok(judgment);
    }
    let input = pair
        .model_input()
        .ok_or_else(|| invalid("identity evidence exceeds limit"))?;
    let output = endpoint
        .complete_json(
            IDENTITY_SYSTEM_PROMPT,
            input,
            "entity_identity_comparison",
            &entity_identity_model_schema(),
        )
        .map_err(public_endpoint_error)?;
    if output.to_string().len() > 4096 {
        return Err(invalid("identity response exceeds 4 KiB limit"));
    }
    validate_output(pair, &output)
}

fn public_endpoint_error(error: GeneralEndpointError) -> EntityResolverError {
    // Transport failures can include a provider body or request coordinates.
    let error = match error {
        GeneralEndpointError::HttpStatus(_) => error,
        GeneralEndpointError::Backpressure { retry_after, .. } => {
            GeneralEndpointError::Backpressure {
                message: "Entity identity endpoint is busy".into(),
                retry_after,
            }
        }
        _ => GeneralEndpointError::Failure("Entity identity endpoint request failed".into()),
    };
    error.into()
}

fn validate_output(
    pair: &IdentityPairEvidence,
    output: &Value,
) -> Result<IdentityJudgment, EntityResolverError> {
    if output.as_object().is_none_or(|fields| fields.len() != 3) {
        return Err(invalid("invalid identity response object"));
    }
    let (outcome, reason) = match (required(output, "outcome")?, required(output, "reason")?) {
        ("same_identity", "corroborated") => {
            (IdentityOutcome::SameIdentity, IdentityReason::Corroborated)
        }
        ("related_distinct", "distinct") => {
            (IdentityOutcome::RelatedDistinct, IdentityReason::Distinct)
        }
        ("unresolved", "insufficient_evidence") => (
            IdentityOutcome::Unresolved,
            IdentityReason::InsufficientEvidence,
        ),
        ("unresolved", "ambiguity") => (IdentityOutcome::Unresolved, IdentityReason::Ambiguity),
        ("unresolved", "conflict") => (IdentityOutcome::Unresolved, IdentityReason::Conflict),
        _ => return Err(invalid("identity outcome and reason disagree")),
    };
    let rows = output["citations"]
        .as_array()
        .filter(|rows| rows.len() <= MAX_IDENTITY_CITATIONS)
        .ok_or_else(|| invalid("invalid identity citations"))?;
    let mut citations = BTreeSet::new();
    let mut facts = Vec::with_capacity(rows.len());
    for row in rows {
        let citation = parse_citation(row)?;
        if !citations.insert(citation) {
            return Err(invalid("duplicate identity citation"));
        }
        let (relationship, memory, quote) = pair
            .witness(citation)
            .ok_or_else(|| invalid("identity citation has no grounded witness"))?;
        let bearing = match required(row, "bearing")? {
            "corroborates_identity" => Bearing::Corroboration,
            "describes_referent" => Bearing::Description,
            "separates_referents" => Bearing::Separation,
            "conflicting_account" => Bearing::Conflict,
            _ => return Err(invalid("invalid identity citation bearing")),
        };
        let fact = required(row, "fact")?;
        if fact.len() > MAX_FACT_BYTES || (!fact.is_empty() && !exact_excerpt(quote, fact)) {
            return Err(invalid("identity fact does not match its cited quote"));
        }
        if outcome != IdentityOutcome::Unresolved && fact.trim().is_empty() {
            return Err(invalid("affirmative identity citation is missing its fact"));
        }
        if outcome != IdentityOutcome::Unresolved {
            let words = words(fact);
            let side = pair.side(citation.side());
            let mention = match citation {
                IdentityCitation::Mention { mention, .. } => {
                    Some(&side.mentions[usize::from(mention)])
                }
                IdentityCitation::Relationship { .. } => None,
            };
            facts.push(CitedFact {
                side: citation.side(),
                endpoint: &side.endpoint,
                mention,
                quote,
                relationship,
                memory,
                anchors: relationship
                    .map(|r| distinguishing_names(side, r, &words))
                    .unwrap_or_default(),
                excerpt: fact,
                bearing,
                words,
            });
        }
    }
    let citations = citations.into_iter().collect::<Vec<_>>();
    if outcome == IdentityOutcome::Unresolved {
        return Ok(unresolved(reason, citations));
    }
    if ![IdentitySide::Left, IdentitySide::Right]
        .into_iter()
        .all(|side| citations.iter().any(|c| c.side() == side))
    {
        return Err(invalid("affirmative identity decision requires both sides"));
    }
    if outcome == IdentityOutcome::SameIdentity
        && (pair.left.endpoint.kind.eq_ignore_ascii_case("unknown")
            || !pair
                .left
                .endpoint
                .kind
                .eq_ignore_ascii_case(&pair.right.endpoint.kind))
    {
        return Ok(unresolved(IdentityReason::Conflict, citations));
    }
    if facts.iter().any(|fact| {
        fact.bearing == Bearing::Conflict
            || (outcome == IdentityOutcome::SameIdentity && fact.bearing == Bearing::Separation)
    }) {
        return Ok(unresolved(IdentityReason::Conflict, citations));
    }
    let supported = facts.iter().all(|fact| {
        facts
            .iter()
            .any(|other| fact.supports(other, pair, outcome))
    });
    if !supported {
        return Ok(unresolved(IdentityReason::Ambiguity, citations));
    }
    Ok(IdentityJudgment {
        outcome,
        reason,
        citations,
    })
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Bearing {
    Corroboration,
    Description,
    Separation,
    Conflict,
}

struct CitedFact<'a> {
    side: IdentitySide,
    endpoint: &'a IdentityEntityEvidence,
    mention: Option<&'a IdentityMentionEvidence>,
    quote: &'a str,
    relationship: Option<&'a IdentityRelationshipEvidence>,
    memory: &'a IdentityObservedMemory,
    anchors: BTreeSet<Vec<String>>,
    excerpt: &'a str,
    bearing: Bearing,
    words: Vec<String>,
}

impl CitedFact<'_> {
    fn is_participant_label(&self) -> bool {
        self.relationship
            .into_iter()
            .flat_map(|r| &r.participants)
            .any(|participant| {
                names(&participant.entity)
                    .chain(participant.roles.iter().map(String::as_str))
                    .any(|label| words(label) == self.words)
            })
    }

    fn cites_context(&self) -> bool {
        // A partial excerpt must carry context beyond an isolated value such as "v2".
        // A complete grounded witness can express a statement without whitespace.
        let complete = self.quote == self.excerpt;
        let contextual_fragment = self
            .excerpt
            .split_whitespace()
            .filter(|part| part.chars().any(char::is_alphanumeric))
            .nth(1)
            .is_some();
        (complete || contextual_fragment)
            && !self.words.is_empty()
            && !self.is_participant_label()
            && self
                .mention
                .is_none_or(|m| m.context_score_for(self.endpoint, self.excerpt) > 0)
            && self
                .relationship
                .into_iter()
                .flat_map(|r| &r.participants)
                .all(|p| words(&p.entity.kind) != self.words)
    }

    fn identifies_separate_referent(&self) -> bool {
        // A bare participant label or role is not an account of what distinguishes the pair.
        self.bearing == Bearing::Separation
            && (self.mention.is_none() || self.cites_context())
            && !self.words.is_empty()
            && !self.is_participant_label()
    }

    // A short participant reference can also be grounded by its structured Relationship.
    fn shares_relationship_anchor(&self, other: &Self, pair: &IdentityPairEvidence) -> bool {
        let (Some(left), Some(right)) = (self.relationship, other.relationship) else {
            return false;
        };
        (self.anchors.contains(&self.words) || self.cites_context())
            && (other.anchors.contains(&other.words) || other.cites_context())
            && !self.anchors.is_disjoint(&other.anchors)
            && left.kind.eq_ignore_ascii_case(&right.kind)
            && compatible_roles(pair.side(self.side), left, pair.side(other.side), right)
    }

    fn supports(
        &self,
        other: &Self,
        pair: &IdentityPairEvidence,
        outcome: IdentityOutcome,
    ) -> bool {
        if self.side == other.side || !independent_sources(self.memory, other.memory) {
            return false;
        }
        match outcome {
            IdentityOutcome::SameIdentity => {
                self.bearing == Bearing::Corroboration
                    && other.bearing == Bearing::Corroboration
                    && ((self.cites_context() && other.cites_context())
                        || self.shares_relationship_anchor(other, pair))
            }
            IdentityOutcome::RelatedDistinct => {
                self.identifies_separate_referent() && other.identifies_separate_referent()
            }
            IdentityOutcome::Unresolved => false,
        }
    }
}

fn names(entity: &IdentityEntityEvidence) -> impl Iterator<Item = &str> {
    std::iter::once(entity.canonical_name.as_str()).chain(entity.aliases.iter().map(String::as_str))
}

fn parse_citation(row: &Value) -> Result<IdentityCitation, EntityResolverError> {
    let fields = row
        .as_object()
        .ok_or_else(|| invalid("invalid identity citation object"))?;
    let side = match required(row, "side")? {
        "left" => IdentitySide::Left,
        "right" => IdentitySide::Right,
        _ => return Err(invalid("invalid identity citation side")),
    };
    let index = |field: &str| {
        row[field]
            .as_u64()
            .and_then(|v| u16::try_from(v).ok())
            .ok_or_else(|| invalid("invalid identity citation index"))
    };
    match (fields.contains_key("mention"), fields.len()) {
        (true, 4) => Ok(IdentityCitation::Mention {
            side,
            mention: index("mention")?,
        }),
        (false, 5) => Ok(IdentityCitation::Relationship {
            side,
            relationship: index("relationship")?,
            memory: index("memory")?,
        }),
        _ => Err(invalid("invalid identity citation object")),
    }
}

fn unresolved(reason: IdentityReason, citations: Vec<IdentityCitation>) -> IdentityJudgment {
    IdentityJudgment {
        outcome: IdentityOutcome::Unresolved,
        reason,
        citations,
    }
}

fn words(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_lowercase)
        .collect()
}

pub(crate) fn exact_excerpt(quote: &str, excerpt: &str) -> bool {
    quote.match_indices(excerpt).any(|(start, surface)| {
        let end = start + surface.len();
        !quote[..start]
            .chars()
            .next_back()
            .is_some_and(char::is_alphanumeric)
            && !quote[end..]
                .chars()
                .next()
                .is_some_and(char::is_alphanumeric)
    })
}

fn distinguishing_names(
    side: &IdentityEvidence,
    relationship: &IdentityRelationshipEvidence,
    fact: &[String],
) -> BTreeSet<Vec<String>> {
    let endpoint_words = names(&side.endpoint)
        .flat_map(|name| words(name))
        .collect::<BTreeSet<_>>();
    let label_words = relationship
        .participants
        .iter()
        .flat_map(|p| std::iter::once(&p.entity.kind).chain(&p.roles))
        .flat_map(|label| words(label))
        .collect::<BTreeSet<_>>();
    let distinctive = |name: &[String]| {
        name.iter().any(|word| {
            !endpoint_words.contains(word)
                && !label_words.contains(word)
                && !matches!(
                    word.as_str(),
                    "a" | "an" | "the" | "of" | "for" | "and" | "at" | "in" | "on" | "by" | "is"
                )
                && !is_bare_generic_surface(word)
        })
    };
    relationship
        .participants
        .iter()
        .filter(|p| p.entity.id != side.endpoint.id)
        .flat_map(|p| {
            let names = names(&p.entity)
                .map(words)
                .filter(|name| distinctive(name))
                .collect::<Vec<_>>();
            let cited = names
                .iter()
                .any(|name| fact.windows(name.len()).any(|part| part == name));
            names.into_iter().filter(move |_| cited)
        })
        .collect()
}

fn compatible_roles(
    left: &IdentityEvidence,
    left_relationship: &IdentityRelationshipEvidence,
    right: &IdentityEvidence,
    right_relationship: &IdentityRelationshipEvidence,
) -> bool {
    let roles = |side: &IdentityEvidence, relationship: &IdentityRelationshipEvidence| {
        relationship
            .participants
            .iter()
            .filter(|p| p.entity.id == side.endpoint.id)
            .flat_map(|p| &p.roles)
            .map(|role| words(role))
            .collect::<BTreeSet<_>>()
    };
    let left = roles(left, left_relationship);
    let right = roles(right, right_relationship);
    left.is_empty() || right.is_empty() || !left.is_disjoint(&right)
}

#[cfg(test)]
#[path = "identity_evaluator_tests.rs"]
pub(crate) mod tests;
