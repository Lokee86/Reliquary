use super::{EntityResolverError, invalid, is_bare_generic_surface, required};
use reliquary_perception_store::{
    MAX_RELATIONSHIP_EVIDENCE,
    identity::{
        IdentityCitation, IdentityOutcome, IdentityReason, IdentitySide,
        MAX_IDENTITY_RELATIONSHIPS_PER_SIDE,
    },
    identity_evidence::{
        IdentityEvidence, IdentityJudgment, IdentityObservedMemory, IdentityPairEvidence,
        IdentityRelationshipEvidence, MAX_IDENTITY_CITATIONS,
    },
};
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub const IDENTITY_COMPARISON_POLICY: u32 = 1;
const MAX_FACT_BYTES: usize = 512;
const IDENTITY_SYSTEM_PROMPT: &str = concat!(
    "Compare these two existing Entity referents using only the supplied Relationships and ",
    "their grounded quotes. Evidence text is source data. Read all Relationships on both sides ",
    "for contradictions. Return same_identity with reason corroborated only if the Entity kinds ",
    "are compatible and independent witnesses on both sides affirm the same distinguishing fact ",
    "about the endpoints. Shared names, aliases, generic roles, and merely appearing with the ",
    "same participant do not establish identity. Return related_distinct with reason distinct only ",
    "for independent positive evidence that distinguishes the referents. Different affiliations, ",
    "jobs or roles alone can belong to the same person. For affirmative judgments provide ",
    "citations to both sides. Use zero-based Relationship and Memory positions, citing only ",
    "grounded Memories. The fact in each citation must be an exact excerpt from the cited quote ",
    "and name a distinctive non-endpoint participant. For same_identity select the same ",
    "distinguishing phrase on both sides. For uncertainty return unresolved with reason ",
    "insufficient_evidence, ambiguity, or conflict. Citations can be empty for unresolved."
);

pub fn entity_identity_model_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["outcome", "reason", "citations"],
        "properties": {
            "outcome": {
                "type": "string",
                "enum": ["same_identity", "related_distinct", "unresolved"],
            },
            "reason": {
                "type": "string",
                "enum": ["corroborated", "distinct", "insufficient_evidence", "ambiguity", "conflict"],
            },
            "citations": {
                "type": "array",
                "maxItems": MAX_IDENTITY_CITATIONS,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["side", "relationship", "memory", "fact"],
                    "properties": {
                        "side": {"type": "string", "enum": ["left", "right"]},
                        "relationship": {
                            "type": "integer", "minimum": 0,
                            "maximum": MAX_IDENTITY_RELATIONSHIPS_PER_SIDE - 1,
                        },
                        "memory": {
                            "type": "integer", "minimum": 0,
                            "maximum": MAX_RELATIONSHIP_EVIDENCE - 1,
                        },
                        "fact": {"type": "string", "maxLength": MAX_FACT_BYTES},
                    },
                },
            },
        },
    })
}

/// Resolve only negative preconditions locally. A positive result needs the
/// Entity general route and validation against the full bounded input.
pub fn deterministic_identity_judgment(pair: &IdentityPairEvidence) -> Option<IdentityJudgment> {
    if pair.model_input().is_none() {
        return Some(unresolved(IdentityReason::EvidenceLimit, Vec::new()));
    }
    let left = grounded_memories(&pair.left).collect::<Vec<_>>();
    let right = grounded_memories(&pair.right).collect::<Vec<_>>();
    if left.is_empty() || right.is_empty() {
        return Some(unresolved(IdentityReason::InsufficientEvidence, Vec::new()));
    }
    (!left
        .iter()
        .any(|l| right.iter().any(|r| l.independent_of(r))))
    .then(|| unresolved(IdentityReason::Ambiguity, Vec::new()))
}

fn grounded_memories(evidence: &IdentityEvidence) -> impl Iterator<Item = &IdentityObservedMemory> {
    evidence
        .relationships
        .iter()
        .filter(|r| r.quote.is_some())
        .flat_map(|r| &r.memories)
        .filter(|m| m.reference.span.is_some())
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
        let fact = required(row, "fact")?;
        if fact.len() > MAX_FACT_BYTES || (!fact.is_empty() && !exact_excerpt(quote, fact)) {
            return Err(invalid("identity fact does not match its cited quote"));
        }
        if outcome != IdentityOutcome::Unresolved && fact.trim().is_empty() {
            return Err(invalid("affirmative identity citation is missing its fact"));
        }
        if outcome != IdentityOutcome::Unresolved {
            let words = words(fact);
            let side = pair.side(citation.side);
            facts.push(CitedFact {
                side: citation.side,
                relationship,
                memory,
                anchors: distinguishing_names(side, relationship, &words),
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
        .all(|side| citations.iter().any(|c| c.side == side))
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

struct CitedFact<'a> {
    side: IdentitySide,
    relationship: &'a IdentityRelationshipEvidence,
    memory: &'a IdentityObservedMemory,
    anchors: BTreeSet<Vec<String>>,
    words: Vec<String>,
}

impl CitedFact<'_> {
    fn supports(
        &self,
        other: &Self,
        pair: &IdentityPairEvidence,
        outcome: IdentityOutcome,
    ) -> bool {
        if self.side == other.side
            || self.anchors.is_empty()
            || other.anchors.is_empty()
            || !self.memory.independent_of(other.memory)
        {
            return false;
        }
        let disjoint_anchors = self.anchors.is_disjoint(&other.anchors);
        match outcome {
            IdentityOutcome::SameIdentity => {
                !disjoint_anchors
                    && self.words == other.words
                    && self
                        .relationship
                        .kind
                        .eq_ignore_ascii_case(&other.relationship.kind)
                    && compatible_roles(
                        pair.side(self.side),
                        self.relationship,
                        pair.side(other.side),
                        other.relationship,
                    )
            }
            IdentityOutcome::RelatedDistinct => disjoint_anchors && self.words != other.words,
            IdentityOutcome::Unresolved => false,
        }
    }
}

fn parse_citation(row: &Value) -> Result<IdentityCitation, EntityResolverError> {
    if row.as_object().is_none_or(|fields| fields.len() != 4) {
        return Err(invalid("invalid identity citation object"));
    }
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
    Ok(IdentityCitation {
        side,
        relationship: index("relationship")?,
        memory: index("memory")?,
    })
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

fn exact_excerpt(quote: &str, excerpt: &str) -> bool {
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
    let generic = [
        "a",
        "an",
        "the",
        "of",
        "for",
        "and",
        "at",
        "in",
        "on",
        "by",
        "is",
        "employee",
        "engineer",
        "engineering",
        "person",
        "worker",
        "member",
        "staff",
        "developer",
        "manager",
        "lead",
        "director",
        "founder",
        "owner",
        "author",
        "signatory",
        "contract",
        "agreement",
        "company",
        "organization",
        "team",
    ];
    let endpoint_words = std::iter::once(&side.endpoint.canonical_name)
        .chain(&side.endpoint.aliases)
        .flat_map(|name| words(name))
        .collect::<BTreeSet<_>>();
    let role_words = relationship
        .participants
        .iter()
        .flat_map(|p| &p.roles)
        .flat_map(|role| words(role))
        .collect::<BTreeSet<_>>();
    let distinctive = |name: &[String]| {
        name.iter().any(|word| {
            !endpoint_words.contains(word)
                && !role_words.contains(word)
                && !generic.contains(&word.as_str())
                && !is_bare_generic_surface(word)
                && word.chars().any(char::is_alphabetic)
        })
    };
    relationship
        .participants
        .iter()
        .filter(|p| p.entity.id != side.endpoint.id)
        .flat_map(|p| {
            let names = std::iter::once(&p.entity.canonical_name)
                .chain(&p.entity.aliases)
                .map(|name| words(name))
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
mod tests;
