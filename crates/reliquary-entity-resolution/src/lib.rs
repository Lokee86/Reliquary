//! Pure Entity-resolution policy over caller-supplied Memory and Entity facts.
//!
//! This crate validates exact body spans, ranks evidence-backed candidates and
//! emits guarded proposals. It does not open stores, resolve authorization or
//! mutate Entity/Memory/Graph state.

use reliquary_memory_core::{MemoryBody, MemoryId};
use reliquary_perception_store::{Entity, EntityId, EntityResolutionReason};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

mod decision_fast_path;
mod model_evaluator;
pub use model_evaluator::{
    AdmissionDecision, ENTITY_ADMISSION_SYSTEM_PROMPT, ENTITY_RESOLUTION_SYSTEM_PROMPT,
    EntityAdmissionMaterialization, EntityAdmissionOutcome, EntityModelEvaluator,
    EntityResolverError, MAX_ENTITY_MODEL_INPUT_BYTES, entity_admission_model_schema,
    entity_resolution_model_schema,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TextField {
    Title,
    Content,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MentionSpan {
    pub field: TextField,
    pub start_byte: usize,
    pub end_byte: usize,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MentionBindingError {
    EmptySurface,
    InvalidBounds,
    InvalidUtf8Boundary,
    SurfaceMismatch,
}

/// Returns the exact source slice only when the submitted body and byte span agree.
pub fn bind_mention<'a>(
    body: &'a MemoryBody,
    mention: &MentionSpan,
) -> Result<&'a str, MentionBindingError> {
    if mention.text.trim().is_empty() {
        return Err(MentionBindingError::EmptySurface);
    }
    let source = match mention.field {
        TextField::Title => &body.title,
        TextField::Content => &body.content,
    };
    if mention.start_byte >= mention.end_byte || mention.end_byte > source.len() {
        return Err(MentionBindingError::InvalidBounds);
    }
    let slice = source
        .get(mention.start_byte..mention.end_byte)
        .ok_or(MentionBindingError::InvalidUtf8Boundary)?;
    if slice != mention.text {
        return Err(MentionBindingError::SurfaceMismatch);
    }
    Ok(slice)
}

#[derive(Clone, Debug, PartialEq)]
pub struct EntityCandidate {
    pub entity: Entity,
    pub exact_surface: bool,
    pub normalized_surface: bool,
    pub alias_surface: bool,
    pub source_association: bool,
    pub lexical_memory_hits: usize,
    pub graph_neighbor_hits: usize,
    pub best_lexical_score: f64,
    pub supporting_memory_ids: Vec<MemoryId>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CandidateEvidence {
    pub exact_surface: bool,
    pub normalized_surface: bool,
    pub alias_surface: bool,
    pub source_association: bool,
}

/// Stable evidence ordering pinned by Entity Resolution: textual evidence,
/// independent evidence lanes, lexical strength/count, graph support, then ID.
pub fn candidate_order(left: &EntityCandidate, right: &EntityCandidate) -> Ordering {
    let left_lanes = usize::from(left.source_association)
        + usize::from(left.lexical_memory_hits > 0)
        + usize::from(left.graph_neighbor_hits > 0);
    let right_lanes = usize::from(right.source_association)
        + usize::from(right.lexical_memory_hits > 0)
        + usize::from(right.graph_neighbor_hits > 0);
    right
        .exact_surface
        .cmp(&left.exact_surface)
        .then_with(|| right.normalized_surface.cmp(&left.normalized_surface))
        .then_with(|| right.alias_surface.cmp(&left.alias_surface))
        .then_with(|| right_lanes.cmp(&left_lanes))
        .then_with(|| right.best_lexical_score.total_cmp(&left.best_lexical_score))
        .then_with(|| right.lexical_memory_hits.cmp(&left.lexical_memory_hits))
        .then_with(|| right.graph_neighbor_hits.cmp(&left.graph_neighbor_hits))
        .then_with(|| left.entity.id.cmp(&right.entity.id))
}

pub fn rank_candidates(candidates: &mut [EntityCandidate]) {
    candidates.sort_by(candidate_order);
}

/// Finds same-kind name conflicts before creating a new Entity. Language
/// qualifier removal applies only to the leading qualifier, and path matching
/// also compares the final path component as in the pinned guard.
pub fn creation_conflicts(
    current: &[Entity],
    surface: &str,
    kind: &str,
    limit: usize,
) -> Vec<Entity> {
    if kind == "principal" {
        return Vec::new();
    }
    let mut qualifiers = BTreeSet::new();
    for entity in current
        .iter()
        .filter(|entity| entity.kind == "programming_language")
    {
        for value in std::iter::once(entity.canonical_name.as_str())
            .chain(entity.aliases.iter().map(String::as_str))
        {
            if let Some(token) = single_token(value) {
                qualifiers.insert(token);
            }
        }
    }
    let wanted = identity_guard_keys(surface, &qualifiers);
    if wanted.is_empty() {
        return Vec::new();
    }
    let mut matches = current
        .iter()
        .filter(|entity| {
            entity.kind == kind
                && std::iter::once(entity.canonical_name.as_str())
                    .chain(entity.aliases.iter().map(String::as_str))
                    .any(|candidate| {
                        identity_guard_keys(candidate, &qualifiers)
                            .iter()
                            .any(|key| wanted.contains(key))
                    })
        })
        .cloned()
        .collect::<Vec<_>>();
    matches.sort_by_key(|entity| entity.id);
    matches.truncate(limit);
    matches
}

const GENERIC_SUFFIXES: &[&str] = &["package", "module", "component"];
fn identity_guard_keys(surface: &str, qualifiers: &BTreeSet<String>) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    append_surface_keys(surface, qualifiers, &mut keys);
    if let Some(last) = surface
        .split(['/', '\\'])
        .filter(|part| !part.trim().is_empty())
        .next_back()
    {
        append_surface_keys(last, qualifiers, &mut keys);
    }
    keys
}
fn append_surface_keys(surface: &str, qualifiers: &BTreeSet<String>, keys: &mut BTreeSet<String>) {
    let parts = identity_tokens_ordered(surface);
    if parts.is_empty() {
        return;
    }
    keys.insert(parts.concat());
    if parts.len() > 1
        && parts
            .last()
            .is_some_and(|value| GENERIC_SUFFIXES.contains(&value.as_str()))
    {
        keys.insert(parts[..parts.len() - 1].concat());
    }
    if parts.len() > 1 && qualifiers.contains(&parts[0]) {
        keys.insert(parts[1..].concat());
    }
}
fn single_token(value: &str) -> Option<String> {
    let values = identity_tokens_ordered(value);
    (values.len() == 1).then(|| values[0].clone())
}
fn identity_tokens_ordered(value: &str) -> Vec<String> {
    let chars: Vec<_> = value.chars().collect();
    let mut output = Vec::new();
    let mut current = String::new();
    for (index, ch) in chars.iter().copied().enumerate() {
        if !ch.is_alphanumeric() {
            if !current.is_empty() {
                output.push(std::mem::take(&mut current));
            }
            continue;
        }
        let split_camel = ch.is_uppercase()
            && !current.is_empty()
            && chars
                .get(index.wrapping_sub(1))
                .is_some_and(|prior| prior.is_lowercase())
            && chars.get(index + 1).is_some_and(|next| next.is_lowercase());
        if split_camel {
            output.push(std::mem::take(&mut current));
        }
        current.extend(ch.to_lowercase());
    }
    if !current.is_empty() {
        output.push(current);
    }
    output
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityRelation {
    SameIdentity,
    RelatedDistinct,
    Uncertain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionChoice {
    Candidate(usize),
    CreateNew,
    Unresolved,
    Reject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionDecision {
    ResolveExisting(EntityId),
    CreateNew,
    Unresolved,
    Reject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionJudgment {
    pub choice: ResolutionChoice,
    pub relation: IdentityRelation,
    pub mention_kind: &'static str,
    pub reason: EntityResolutionReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionOutcome {
    pub decision: ResolutionDecision,
    pub reason: EntityResolutionReason,
}

/// Applies a prepared judgment only when candidate index, surface evidence,
/// identity relation, and kind rules make that judgment safe.
pub fn evaluate_judgment(
    candidates: &[EntityCandidate],
    judgment: ResolutionJudgment,
) -> ResolutionOutcome {
    let mut decision = match judgment.choice {
        ResolutionChoice::Candidate(index) => {
            let Some(candidate) = candidates.get(index) else {
                return ResolutionOutcome {
                    decision: ResolutionDecision::Unresolved,
                    reason: EntityResolutionReason::Ambiguous,
                };
            };
            let compatible = judgment.mention_kind == candidate.entity.kind
                || (judgment.mention_kind == "unknown"
                    && (candidate.exact_surface || candidate.normalized_surface));
            match judgment.relation {
                IdentityRelation::SameIdentity
                    if compatible && (candidate.exact_surface || candidate.normalized_surface) =>
                {
                    ResolutionDecision::ResolveExisting(candidate.entity.id)
                }
                IdentityRelation::SameIdentity => ResolutionDecision::Unresolved,
                IdentityRelation::RelatedDistinct if candidate.exact_surface && compatible => {
                    ResolutionDecision::Unresolved
                }
                IdentityRelation::RelatedDistinct => ResolutionDecision::CreateNew,
                IdentityRelation::Uncertain => ResolutionDecision::Unresolved,
            }
        }
        ResolutionChoice::CreateNew => {
            let exact_same_kind = candidates.iter().any(|candidate| {
                candidate.exact_surface && candidate.entity.kind == judgment.mention_kind
            });
            match judgment.relation {
                IdentityRelation::SameIdentity => ResolutionDecision::Unresolved,
                IdentityRelation::RelatedDistinct if exact_same_kind => {
                    ResolutionDecision::Unresolved
                }
                IdentityRelation::RelatedDistinct => ResolutionDecision::CreateNew,
                IdentityRelation::Uncertain => ResolutionDecision::Unresolved,
            }
        }
        ResolutionChoice::Unresolved => ResolutionDecision::Unresolved,
        ResolutionChoice::Reject => ResolutionDecision::Reject,
    };
    let mut reason = judgment.reason;
    if reason == EntityResolutionReason::RecurrenceRequired {
        decision = ResolutionDecision::Unresolved;
    } else if is_rejection_reason(reason) {
        decision = ResolutionDecision::Reject;
    }
    if decision == ResolutionDecision::Unresolved
        && judgment.relation == IdentityRelation::RelatedDistinct
        && reason == EntityResolutionReason::ContextMatch
    {
        reason = EntityResolutionReason::Ambiguous;
    }
    ResolutionOutcome { decision, reason }
}

fn is_rejection_reason(reason: EntityResolutionReason) -> bool {
    matches!(
        reason,
        EntityResolutionReason::GenericRole
            | EntityResolutionReason::AbstractProcess
            | EntityResolutionReason::TransientValue
            | EntityResolutionReason::SentenceLocal
            | EntityResolutionReason::WrapperCategory
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityReconciliationCandidate {
    pub left: EntityId,
    pub right: EntityId,
    pub deterministic: bool,
    pub score: u16,
}

/// Deterministic reconciliation candidate generation; principal Entities are
/// excluded, exact same-kind claims are surfaced first, then lexical pairs.
pub fn reconciliation_candidates(
    entities: &[Entity],
    pair_limit: usize,
) -> Vec<EntityReconciliationCandidate> {
    let mut by_pair = BTreeMap::<(EntityId, EntityId), EntityReconciliationCandidate>::new();
    let mut claims = BTreeMap::<(String, String), BTreeSet<EntityId>>::new();
    for entity in entities.iter().filter(|entity| entity.kind != "principal") {
        for surface in std::iter::once(entity.canonical_name.as_str())
            .chain(entity.aliases.iter().map(String::as_str))
        {
            let key = surface.trim().to_lowercase();
            if !key.is_empty() {
                claims
                    .entry((key, entity.kind.clone()))
                    .or_default()
                    .insert(entity.id);
            }
        }
    }
    for ids in claims.into_values() {
        let ids: Vec<_> = ids.into_iter().collect();
        for i in 0..ids.len() {
            for j in i + 1..ids.len() {
                add_pair(&mut by_pair, ids[i], ids[j], true, u16::MAX);
            }
        }
    }
    let mut by_token = BTreeMap::<String, Vec<EntityId>>::new();
    let mut tokens_by_id = HashMap::<EntityId, BTreeSet<String>>::new();
    let kind_by_id = entities
        .iter()
        .map(|e| (e.id, e.kind.as_str()))
        .collect::<HashMap<_, _>>();
    for entity in entities.iter().filter(|entity| entity.kind != "principal") {
        let tokens = identity_tokens(&entity.canonical_name);
        for token in &tokens {
            by_token.entry(token.clone()).or_default().push(entity.id);
        }
        tokens_by_id.insert(entity.id, tokens);
    }
    let mut seen = HashSet::new();
    for ids in by_token.into_values() {
        if ids.len() > 32 {
            continue;
        }
        for i in 0..ids.len() {
            for j in i + 1..ids.len() {
                let (left, right) = ordered(ids[i], ids[j]);
                if !seen.insert((left, right)) || kind_by_id.get(&left) != kind_by_id.get(&right) {
                    continue;
                }
                let (Some(a), Some(b)) = (tokens_by_id.get(&left), tokens_by_id.get(&right)) else {
                    continue;
                };
                let shared = a.intersection(b).count();
                let union = a.union(b).count().max(1);
                let score = ((shared * 1000) / union) as u16;
                if a.is_subset(b) || b.is_subset(a) || score >= 330 {
                    add_pair(&mut by_pair, left, right, false, score);
                }
            }
        }
    }
    let mut values: Vec<_> = by_pair.into_values().collect();
    let by_id = entities
        .iter()
        .map(|e| (e.id, e))
        .collect::<HashMap<_, _>>();
    values.sort_by(|left, right| {
        right
            .deterministic
            .cmp(&left.deterministic)
            .then_with(|| right.score.cmp(&left.score))
            .then_with(|| {
                by_id
                    .get(&left.left)
                    .map(|e| e.canonical_name.as_str())
                    .unwrap_or("")
                    .cmp(
                        by_id
                            .get(&right.left)
                            .map(|e| e.canonical_name.as_str())
                            .unwrap_or(""),
                    )
            })
            .then_with(|| left.left.cmp(&right.left))
            .then_with(|| left.right.cmp(&right.right))
    });
    values.truncate(pair_limit);
    values
}

fn add_pair(
    map: &mut BTreeMap<(EntityId, EntityId), EntityReconciliationCandidate>,
    a: EntityId,
    b: EntityId,
    deterministic: bool,
    score: u16,
) {
    let (left, right) = ordered(a, b);
    map.entry((left, right))
        .and_modify(|value| {
            value.deterministic |= deterministic;
            value.score = value.score.max(score);
        })
        .or_insert(EntityReconciliationCandidate {
            left,
            right,
            deterministic,
            score,
        });
}
fn ordered(a: EntityId, b: EntityId) -> (EntityId, EntityId) {
    if a <= b { (a, b) } else { (b, a) }
}
fn identity_tokens(value: &str) -> BTreeSet<String> {
    const GENERIC: &[&str] = &[
        "application",
        "component",
        "directory",
        "file",
        "module",
        "project",
        "repository",
        "repo",
        "server",
        "service",
        "subsystem",
        "system",
        "tool",
    ];
    value
        .split(|ch: char| !ch.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|token| token.len() >= 4 && !GENERIC.contains(&token.as_str()))
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReconciliationRelation {
    SameIdentity,
    RelatedDistinct,
    Uncertain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergeGuardError {
    SameEntity,
    MissingEntity,
    StaleRevision,
    PrincipalEntity,
    KindMismatch,
    NotSameIdentity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MergeProposal {
    pub left: EntityId,
    pub left_revision: u64,
    pub right: EntityId,
    pub right_revision: u64,
}

/// Produces only a revision-bound proposal. It never selects a survivor or writes a merge.
pub fn guarded_merge_proposal(
    entities: &[Entity],
    left: EntityId,
    expected_left_revision: u64,
    right: EntityId,
    expected_right_revision: u64,
    relation: ReconciliationRelation,
) -> Result<MergeProposal, MergeGuardError> {
    if left == right {
        return Err(MergeGuardError::SameEntity);
    }
    if relation != ReconciliationRelation::SameIdentity {
        return Err(MergeGuardError::NotSameIdentity);
    }
    let left_entity = entities
        .iter()
        .find(|e| e.id == left)
        .ok_or(MergeGuardError::MissingEntity)?;
    let right_entity = entities
        .iter()
        .find(|e| e.id == right)
        .ok_or(MergeGuardError::MissingEntity)?;
    if left_entity.revision != expected_left_revision
        || right_entity.revision != expected_right_revision
    {
        return Err(MergeGuardError::StaleRevision);
    }
    if left_entity.kind == "principal" || right_entity.kind == "principal" {
        return Err(MergeGuardError::PrincipalEntity);
    }
    if left_entity.kind != right_entity.kind {
        return Err(MergeGuardError::KindMismatch);
    }
    Ok(MergeProposal {
        left,
        left_revision: left_entity.revision,
        right,
        right_revision: right_entity.revision,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_perception_store::Entity;

    fn entity(id: u8, kind: &str, name: &str) -> Entity {
        Entity {
            id: EntityId([id; 32]),
            revision: 1,
            canonical_name: name.into(),
            aliases: vec![],
            kind: kind.into(),
            summary: "summary".into(),
            mutation_id: String::new(),
            created_at_ns: 0,
            updated_at_ns: 0,
        }
    }

    #[test]
    fn mention_binding_requires_exact_utf8_body_span() {
        let body = MemoryBody {
            title: "Café server".into(),
            content: "x".into(),
        };
        let good = MentionSpan {
            field: TextField::Title,
            start_byte: 0,
            end_byte: 5,
            text: "Café".into(),
        };
        assert_eq!(bind_mention(&body, &good), Ok("Café"));
        let split = MentionSpan {
            start_byte: 4,
            end_byte: 6,
            text: "é ".into(),
            ..good.clone()
        };
        assert_eq!(
            bind_mention(&body, &split),
            Err(MentionBindingError::InvalidUtf8Boundary)
        );
        let mismatch = MentionSpan {
            text: "Cafe".into(),
            ..good
        };
        assert_eq!(
            bind_mention(&body, &mismatch),
            Err(MentionBindingError::SurfaceMismatch)
        );
    }

    #[test]
    fn reconciliation_candidates_are_deterministic_and_exclude_principals() {
        let values = [
            entity(2, "tool", "Northstar Engine"),
            entity(1, "tool", "Northstar Engine"),
            entity(3, "principal", "Northstar Engine"),
        ];
        let pairs = reconciliation_candidates(&values, 10);
        assert_eq!(pairs.len(), 1);
        assert!(pairs[0].deterministic);
        assert_eq!(
            (pairs[0].left, pairs[0].right),
            (EntityId([1; 32]), EntityId([2; 32]))
        );
    }

    #[test]
    fn ambiguous_or_incompatible_identity_judgments_do_not_resolve() {
        let candidate = EntityCandidate {
            entity: entity(1, "file", "main.go"),
            exact_surface: true,
            normalized_surface: false,
            alias_surface: false,
            source_association: false,
            lexical_memory_hits: 0,
            graph_neighbor_hits: 0,
            best_lexical_score: 0.0,
            supporting_memory_ids: vec![],
        };
        let out = evaluate_judgment(
            &[candidate.clone()],
            ResolutionJudgment {
                choice: ResolutionChoice::Candidate(0),
                relation: IdentityRelation::RelatedDistinct,
                mention_kind: "file",
                reason: EntityResolutionReason::ContextMatch,
            },
        );
        assert_eq!(
            out,
            ResolutionOutcome {
                decision: ResolutionDecision::Unresolved,
                reason: EntityResolutionReason::Ambiguous
            }
        );
        let wrong_kind = evaluate_judgment(
            &[candidate],
            ResolutionJudgment {
                choice: ResolutionChoice::Candidate(0),
                relation: IdentityRelation::SameIdentity,
                mention_kind: "project",
                reason: EntityResolutionReason::ContextMatch,
            },
        );
        assert_eq!(wrong_kind.decision, ResolutionDecision::Unresolved);
    }

    #[test]
    fn merge_guard_binds_same_identity_kind_and_revisions_without_mutating() {
        let values = [entity(1, "tool", "Engine"), entity(2, "tool", "Engine")];
        assert!(
            guarded_merge_proposal(
                &values,
                EntityId([1; 32]),
                1,
                EntityId([2; 32]),
                1,
                ReconciliationRelation::SameIdentity
            )
            .is_ok()
        );
        assert_eq!(
            guarded_merge_proposal(
                &values,
                EntityId([1; 32]),
                2,
                EntityId([2; 32]),
                1,
                ReconciliationRelation::SameIdentity
            ),
            Err(MergeGuardError::StaleRevision)
        );
        assert_eq!(
            guarded_merge_proposal(
                &values,
                EntityId([1; 32]),
                1,
                EntityId([2; 32]),
                1,
                ReconciliationRelation::Uncertain
            ),
            Err(MergeGuardError::NotSameIdentity)
        );
        let principal = [
            entity(1, "principal", "Alice"),
            entity(2, "principal", "Alice"),
        ];
        assert_eq!(
            guarded_merge_proposal(
                &principal,
                EntityId([1; 32]),
                1,
                EntityId([2; 32]),
                1,
                ReconciliationRelation::SameIdentity
            ),
            Err(MergeGuardError::PrincipalEntity)
        );
    }

    #[test]
    fn creation_guard_matches_path_basename_and_qualified_names() {
        let current = [
            entity(1, "tool", "GameServer"),
            entity(2, "programming_language", "Go"),
        ];
        assert_eq!(
            creation_conflicts(&current, "services/game-server", "tool", 8).len(),
            1
        );
        assert_eq!(
            creation_conflicts(&current, "Go Game Server", "tool", 8).len(),
            1
        );
        assert!(creation_conflicts(&current, "Game Server devtools", "tool", 8).is_empty());
        assert!(creation_conflicts(&current, "Alice", "principal", 8).is_empty());
    }
}
