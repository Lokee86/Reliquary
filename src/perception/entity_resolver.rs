use crate::entity_resolver_schema::{ENTITY_RESOLVER_SYSTEM_PROMPT, entity_resolver_schema};
use crate::{
    EntityCandidateSet, EntityResolutionDecision, EntityResolutionReason, EntityResolverError,
    EntityResolverOutput, GeneralEndpoint, Memory,
};
use serde_json::{Value, json};

pub const MAX_ENTITY_RESOLVER_EVIDENCE_MEMORIES: usize = 3;
pub const MAX_ENTITY_RESOLVER_EVIDENCE_TEXT_BYTES: usize = 4096;

pub(crate) struct EntityResolverCore<E> {
    endpoint: E,
    system_prompt: String,
}

impl<E: GeneralEndpoint> EntityResolverCore<E> {
    pub(crate) fn new(endpoint: E) -> Self {
        Self {
            endpoint,
            system_prompt: ENTITY_RESOLVER_SYSTEM_PROMPT.to_owned(),
        }
    }

    pub(crate) fn with_system_prompt(endpoint: E, system_prompt: impl Into<String>) -> Self {
        Self {
            endpoint,
            system_prompt: system_prompt.into(),
        }
    }

    pub(crate) fn model(&self) -> &str {
        self.endpoint.model()
    }

    pub(crate) fn complete_json(
        &self,
        system_prompt: &str,
        user_payload: &str,
        schema_name: &str,
        schema: &Value,
    ) -> Result<Value, EntityResolverError> {
        Ok(self
            .endpoint
            .complete_json(system_prompt, user_payload, schema_name, schema)?)
    }

    pub(crate) fn resolve(
        &self,
        memory: &Memory,
        set: &EntityCandidateSet,
        evidence: &[Vec<Memory>],
    ) -> Result<EntityResolverOutput, EntityResolverError> {
        if evidence.len() != set.candidates.len() {
            return Err(invalid("candidate evidence length mismatch"));
        }
        let payload = json!({
            "title": memory.title,
            "content": memory.content,
            "mention": {
                "field": mention_field(set.mention.field),
                "start_byte": set.mention.start_byte,
                "end_byte": set.mention.end_byte,
                "text": set.mention.text,
            },
            "candidates": set.candidates.iter().zip(evidence).enumerate().map(
                |(index, (candidate, memories))| json!({
                    "candidate_index": index,
                    "entity_id": hex(&candidate.entity.id.0),
                    "canonical_name": candidate.entity.canonical_name,
                    "aliases": candidate.entity.aliases,
                    "kind": candidate.entity.kind,
                    "summary": candidate.entity.summary,
                    "evidence": memories.iter().take(MAX_ENTITY_RESOLVER_EVIDENCE_MEMORIES).map(
                        |memory| json!({
                            "memory_id": hex(&memory.id.0),
                            "text": truncate_utf8(
                                &memory.content,
                                MAX_ENTITY_RESOLVER_EVIDENCE_TEXT_BYTES,
                            ),
                        })
                    ).collect::<Vec<_>>(),
                })
            ).collect::<Vec<_>>(),
        });
        let output = self.complete_json(
            &self.system_prompt,
            &payload.to_string(),
            "entity_store_resolution_v9",
            &entity_resolver_schema(),
        )?;
        parse_output(&output, set)
    }
}

fn parse_output(
    output: &Value,
    set: &EntityCandidateSet,
) -> Result<EntityResolverOutput, EntityResolverError> {
    let raw_decision = required_string(output, "decision")?;
    let mut reason = parse_reason(required_string(output, "reason")?)?;
    let has_v6_identity_fields =
        output.get("identity_relation").is_some() && output.get("mention_kind").is_some();
    let identity_relation = output
        .get("identity_relation")
        .and_then(Value::as_str)
        .unwrap_or("same_identity");
    let mention_kind = output
        .get("mention_kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let target = output
        .get("target_candidate_index")
        .and_then(Value::as_i64)
        .ok_or_else(|| invalid("target_candidate_index must be an integer"))?;

    let mut decision = match raw_decision {
        "resolve_existing" => {
            let index = usize::try_from(target).map_err(|_| invalid("target out of range"))?;
            let candidate = set
                .candidates
                .get(index)
                .ok_or_else(|| invalid("target out of range"))?;
            if !has_v6_identity_fields {
                EntityResolutionDecision::ResolveExisting(candidate.entity.id)
            } else {
                match identity_relation {
                    "same_identity"
                        if identity_kinds_compatible(
                            mention_kind,
                            candidate.entity.kind.as_str(),
                            candidate,
                        ) =>
                    {
                        EntityResolutionDecision::ResolveExisting(candidate.entity.id)
                    }
                    "same_identity" => {
                        reason = EntityResolutionReason::InsufficientEvidence;
                        EntityResolutionDecision::Unresolved
                    }
                    "related_distinct"
                        if candidate.exact_surface
                            && identity_kinds_compatible(
                                mention_kind,
                                candidate.entity.kind.as_str(),
                                candidate,
                            ) =>
                    {
                        reason = EntityResolutionReason::Ambiguous;
                        EntityResolutionDecision::Unresolved
                    }
                    "related_distinct" => {
                        reason = EntityResolutionReason::ContextConflictNewIdentity;
                        EntityResolutionDecision::CreateNew
                    }
                    "uncertain" => {
                        reason = EntityResolutionReason::InsufficientEvidence;
                        EntityResolutionDecision::Unresolved
                    }
                    _ => return Err(invalid("unknown identity_relation")),
                }
            }
        }
        "create_new" => {
            if has_v6_identity_fields && identity_relation == "same_identity" {
                reason = EntityResolutionReason::InsufficientEvidence;
                EntityResolutionDecision::Unresolved
            } else if has_v6_identity_fields
                && identity_relation == "related_distinct"
                && set.candidates.iter().any(|candidate| {
                    candidate.exact_surface && mention_kind == candidate.entity.kind.as_str()
                })
            {
                reason = EntityResolutionReason::Ambiguous;
                EntityResolutionDecision::Unresolved
            } else {
                EntityResolutionDecision::CreateNew
            }
        }
        "unresolved" => EntityResolutionDecision::Unresolved,
        "reject" => EntityResolutionDecision::Reject,
        _ => return Err(invalid("unknown decision")),
    };
    if reason == EntityResolutionReason::RecurrenceRequired {
        decision = EntityResolutionDecision::Unresolved;
    } else if reason.is_rejection_class() {
        decision = EntityResolutionDecision::Reject;
    }
    Ok(EntityResolverOutput { decision, reason })
}

fn identity_kinds_compatible(
    mention_kind: &str,
    candidate_kind: &str,
    candidate: &crate::EntityCandidate,
) -> bool {
    if mention_kind == candidate_kind {
        return true;
    }
    if mention_kind == "unknown" {
        return candidate.exact_surface || candidate.normalized_surface;
    }
    false
}

pub(crate) fn parse_reason(value: &str) -> Result<EntityResolutionReason, EntityResolverError> {
    Ok(match value {
        "context_match" => EntityResolutionReason::ContextMatch,
        "context_conflict_new_identity" => EntityResolutionReason::ContextConflictNewIdentity,
        "named_referent" => EntityResolutionReason::NamedReferent,
        "persistent_artifact" => EntityResolutionReason::PersistentArtifact,
        "descriptive_identity" => EntityResolutionReason::DescriptiveIdentity,
        "first_seen_identity" => EntityResolutionReason::FirstSeenIdentity,
        "insufficient_evidence" => EntityResolutionReason::InsufficientEvidence,
        "generic_role" => EntityResolutionReason::GenericRole,
        "abstract_process" => EntityResolutionReason::AbstractProcess,
        "transient_value" => EntityResolutionReason::TransientValue,
        "sentence_local" => EntityResolutionReason::SentenceLocal,
        "wrapper_category" => EntityResolutionReason::WrapperCategory,
        "ambiguous" => EntityResolutionReason::Ambiguous,
        "recurrence_required" => EntityResolutionReason::RecurrenceRequired,
        _ => return Err(invalid("unknown reason")),
    })
}

pub(crate) fn required_string<'a>(
    value: &'a Value,
    key: &str,
) -> Result<&'a str, EntityResolverError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(&format!("missing string field {key}")))
}

pub(crate) fn mention_field(value: crate::MemoryTextField) -> &'static str {
    match value {
        crate::MemoryTextField::Title => "title",
        crate::MemoryTextField::Content => "content",
    }
}

fn truncate_utf8(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

fn hex(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

pub(crate) fn invalid(message: &str) -> EntityResolverError {
    EntityResolverError::InvalidOutput(message.into())
}
