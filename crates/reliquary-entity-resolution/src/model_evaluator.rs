use crate::{
    EntityCandidate, EntityResolutionReason, IdentityRelation, MentionSpan, ResolutionChoice,
    ResolutionDecision, ResolutionJudgment, ResolutionOutcome, TextField, evaluate_judgment,
};
use reliquary_memory_core::MemoryBody;
use reliquary_switchboard::{DecisionEndpoint, GeneralEndpoint, GeneralEndpointError};
use serde_json::{Value, json};
use std::sync::Arc;

pub const MAX_ENTITY_MODEL_INPUT_BYTES: usize = 16 * 1024;
pub const ENTITY_RESOLUTION_SYSTEM_PROMPT: &str = "Resolve the exact supplied Memory mention against only the supplied candidates. Candidate similarity is not proof of identity. Return resolve_existing only for the same referent, create_new only for a distinct durable referent, unresolved when uncertain, and reject transient or non-entity mentions. Keep identity_relation and mention_kind explicit.";
pub const ENTITY_ADMISSION_SYSTEM_PROMPT: &str = "Decide whether this exact Memory mention is a continuing durable Entity. Reject generic, transient, historical, sentence-local, and wrapper-category mentions. Fine-grained implementation artifacts require recurrence evidence from another Memory. For create_new return a narrow kind and source-grounded stable identity summary; otherwise use kind unknown and an empty summary.";

pub fn entity_resolution_model_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["decision","reason","target_candidate_index","identity_relation","mention_kind"],"properties":{
        "decision":{"type":"string","enum":["resolve_existing","create_new","unresolved","reject"]},
        "reason":{"type":"string","enum":["context_match","context_conflict_new_identity","named_referent","persistent_artifact","descriptive_identity","first_seen_identity","insufficient_evidence","generic_role","abstract_process","transient_value","sentence_local","wrapper_category","ambiguous","recurrence_required"]},
        "target_candidate_index":{"type":"integer"},
        "identity_relation":{"type":"string","enum":["same_identity","related_distinct","uncertain"]},
        "mention_kind":{"type":"string","minLength":1,"maxLength":128}
    }})
}

pub fn entity_admission_model_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["decision","reason","promotion_policy","entity_kind","entity_summary"],"properties":{
        "decision":{"type":"string","enum":["create_new","unresolved","reject"]},
        "reason":{"type":"string","enum":["context_match","context_conflict_new_identity","named_referent","persistent_artifact","descriptive_identity","first_seen_identity","insufficient_evidence","generic_role","abstract_process","transient_value","sentence_local","wrapper_category","ambiguous","recurrence_required"]},
        "promotion_policy":{"type":"string","enum":["immediate","requires_recurrence","none"]},
        "entity_kind":{"type":"string","maxLength":128},"entity_summary":{"type":"string","maxLength":1024}
    }})
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityAdmissionMaterialization {
    pub kind: String,
    pub summary: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityAdmissionOutcome {
    pub decision: AdmissionDecision,
    pub reason: EntityResolutionReason,
    pub materialization: Option<EntityAdmissionMaterialization>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionDecision {
    CreateNew,
    Unresolved,
    Reject,
}

#[derive(Debug)]
pub enum EntityResolverError {
    Endpoint(GeneralEndpointError),
    InvalidOutput(String),
}
impl From<GeneralEndpointError> for EntityResolverError {
    fn from(value: GeneralEndpointError) -> Self {
        Self::Endpoint(value)
    }
}
impl std::fmt::Display for EntityResolverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Endpoint(e) => write!(f, "{e}"),
            Self::InvalidOutput(m) => write!(f, "invalid Entity model output: {m}"),
        }
    }
}
impl std::error::Error for EntityResolverError {}

pub struct EntityModelEvaluator<E> {
    endpoint: E,
    decision: Option<Arc<dyn DecisionEndpoint>>,
}
impl<E: GeneralEndpoint> EntityModelEvaluator<E> {
    pub fn new(endpoint: E) -> Self {
        Self {
            endpoint,
            decision: None,
        }
    }
    pub fn with_decision_endpoint(mut self, decision: Option<Arc<dyn DecisionEndpoint>>) -> Self {
        self.decision = decision;
        self
    }
    pub fn model(&self) -> &str {
        self.endpoint.model()
    }

    pub fn resolve(
        &self,
        body: &MemoryBody,
        mention: &MentionSpan,
        candidates: &[EntityCandidate],
    ) -> Result<ResolutionOutcome, EntityResolverError> {
        let source = match mention.field {
            TextField::Title => &body.title,
            TextField::Content => &body.content,
        };
        let surface = source
            .get(mention.start_byte..mention.end_byte)
            .ok_or_else(|| invalid("mention span is out of bounds or splits UTF-8"))?;
        if surface != mention.text || surface.trim().is_empty() {
            return Err(invalid("mention surface does not match exact Memory span"));
        }
        if is_bare_generic_surface(surface) {
            return Ok(ResolutionOutcome {
                decision: ResolutionDecision::Reject,
                reason: EntityResolutionReason::GenericRole,
            });
        }
        if is_obvious_transient_occurrence(surface, source) {
            return Ok(ResolutionOutcome {
                decision: ResolutionDecision::Reject,
                reason: EntityResolutionReason::TransientValue,
            });
        }
        if candidates.len() > 8 {
            return Err(invalid("candidate count exceeds 8"));
        }
        let payload = json!({"title":body.title,"content":body.content,"mention":{"field":match mention.field { TextField::Title=>"title",TextField::Content=>"content"},"start_byte":mention.start_byte,"end_byte":mention.end_byte,"text":surface},"candidates":candidates.iter().enumerate().map(|(i,c)|json!({"candidate_index":i,"entity_id":hex(&c.entity.id.0),"canonical_name":c.entity.canonical_name,"aliases":c.entity.aliases,"kind":c.entity.kind,"summary":c.entity.summary,"evidence":{"exact_surface":c.exact_surface,"normalized_surface":c.normalized_surface,"alias_surface":c.alias_surface,"source_association":c.source_association,"lexical_memory_hits":c.lexical_memory_hits,"graph_neighbor_hits":c.graph_neighbor_hits}})).collect::<Vec<_>>()});
        if let Some(decision) = &self.decision {
            let encoded =
                serde_json::to_vec(&payload).map_err(|error| invalid(&error.to_string()))?;
            if encoded.len() > MAX_ENTITY_MODEL_INPUT_BYTES {
                return Err(invalid("Entity model input exceeds byte limit"));
            }
            if let Some(outcome) =
                crate::decision_fast_path::try_fast_path(decision.as_ref(), &payload, candidates)
            {
                return Ok(outcome);
            }
        }
        let output = self.complete(
            ENTITY_RESOLUTION_SYSTEM_PROMPT,
            &payload,
            "entity_resolution",
            entity_resolution_model_schema(),
        )?;
        let choice = match required(&output, "decision")? {
            "resolve_existing" => {
                let i = index(&output)?;
                ResolutionChoice::Candidate(i)
            }
            "create_new" => ResolutionChoice::CreateNew,
            "unresolved" => ResolutionChoice::Unresolved,
            "reject" => ResolutionChoice::Reject,
            _ => return Err(invalid("unknown resolution decision")),
        };
        if matches!(choice, ResolutionChoice::Candidate(index) if index >= candidates.len()) {
            return Err(invalid("target candidate index out of range"));
        }
        let relation = match required(&output, "identity_relation")? {
            "same_identity" => IdentityRelation::SameIdentity,
            "related_distinct" => IdentityRelation::RelatedDistinct,
            "uncertain" => IdentityRelation::Uncertain,
            _ => return Err(invalid("unknown identity relation")),
        };
        let reason = parse_reason(required(&output, "reason")?)?;
        let kind = parse_kind(required(&output, "mention_kind")?)?;
        Ok(evaluate_judgment(
            candidates,
            ResolutionJudgment {
                choice,
                relation,
                mention_kind: kind,
                reason,
            },
        ))
    }

    pub fn admit(
        &self,
        body: &MemoryBody,
        mention: &MentionSpan,
        recurrence_evidence: &[String],
    ) -> Result<EntityAdmissionOutcome, EntityResolverError> {
        let source = match mention.field {
            TextField::Title => &body.title,
            TextField::Content => &body.content,
        };
        let surface = source
            .get(mention.start_byte..mention.end_byte)
            .ok_or_else(|| invalid("mention span is out of bounds or splits UTF-8"))?;
        if surface != mention.text || surface.trim().is_empty() {
            return Err(invalid("mention surface does not match exact Memory span"));
        }
        if is_bare_generic_surface(surface) {
            return Ok(EntityAdmissionOutcome {
                decision: AdmissionDecision::Reject,
                reason: EntityResolutionReason::GenericRole,
                materialization: None,
            });
        }
        if is_obvious_transient_occurrence(surface, source) {
            return Ok(EntityAdmissionOutcome {
                decision: AdmissionDecision::Reject,
                reason: EntityResolutionReason::TransientValue,
                materialization: None,
            });
        }
        if recurrence_evidence.len() > 3 {
            return Err(invalid("recurrence evidence count exceeds 3"));
        }
        let payload = json!({"title":body.title,"content":body.content,"mention":{"field":match mention.field{TextField::Title=>"title",TextField::Content=>"content"},"start_byte":mention.start_byte,"end_byte":mention.end_byte,"text":surface},"recurrence_evidence":recurrence_evidence.iter().map(|s|truncate(s,4096)).collect::<Vec<_>>()});
        let output = self.complete(
            ENTITY_ADMISSION_SYSTEM_PROMPT,
            &payload,
            "entity_admission",
            entity_admission_model_schema(),
        )?;
        let mut decision = match required(&output, "decision")? {
            "create_new" => AdmissionDecision::CreateNew,
            "unresolved" => AdmissionDecision::Unresolved,
            "reject" => AdmissionDecision::Reject,
            _ => return Err(invalid("unknown admission decision")),
        };
        let mut reason = parse_reason(required(&output, "reason")?)?;
        let policy = required(&output, "promotion_policy")?;
        if !matches!(policy, "immediate" | "requires_recurrence" | "none") {
            return Err(invalid("unknown Entity promotion policy"));
        }
        if reason == EntityResolutionReason::RecurrenceRequired {
            decision = AdmissionDecision::Unresolved;
        } else if is_rejection_reason(reason) {
            decision = AdmissionDecision::Reject;
        }
        let kind = required(&output, "entity_kind")?;
        let summary = required(&output, "entity_summary")?.trim();
        if reason == EntityResolutionReason::RecurrenceRequired {
            return Ok(EntityAdmissionOutcome {
                decision: AdmissionDecision::Unresolved,
                reason,
                materialization: None,
            });
        }
        let recurrence_required = policy == "requires_recurrence"
            || matches!(kind, "code_symbol" | "enum_value" | "input_action");
        let materialization = if decision == AdmissionDecision::CreateNew {
            if kind.is_empty()
                || kind == "unknown"
                || kind.len() > 128
                || summary.is_empty()
                || summary.len() > 1024
            {
                return Err(invalid(
                    "create_new requires bounded Entity kind and summary",
                ));
            }
            // Context text is not identity proof. Keep recurrence-gated
            // artifacts pending until callers can provide a typed,
            // independently verified same-identity proof.
            if recurrence_required {
                decision = AdmissionDecision::Unresolved;
                reason = EntityResolutionReason::RecurrenceRequired;
                None
            } else if policy == "none" {
                return Err(invalid("create_new admission requires promotion policy"));
            } else if materialization_describes_transient(surface, summary) {
                decision = AdmissionDecision::Reject;
                reason = EntityResolutionReason::TransientValue;
                None
            } else if materialization_describes_wrapper_category(kind, summary) {
                decision = AdmissionDecision::Reject;
                reason = EntityResolutionReason::WrapperCategory;
                None
            } else {
                Some(EntityAdmissionMaterialization {
                    kind: kind.to_owned(),
                    summary: summary.to_owned(),
                })
            }
        } else {
            if kind != "unknown" || !summary.is_empty() {
                return Err(invalid(
                    "non-create admission must not return Entity metadata",
                ));
            }
            None
        };
        if reason == EntityResolutionReason::RecurrenceRequired {
            decision = AdmissionDecision::Unresolved;
        }
        if is_rejection_reason(reason) {
            decision = AdmissionDecision::Reject;
        }
        Ok(EntityAdmissionOutcome {
            decision,
            reason,
            materialization: if decision == AdmissionDecision::CreateNew {
                materialization
            } else {
                None
            },
        })
    }

    fn complete(
        &self,
        prompt: &str,
        payload: &Value,
        name: &str,
        schema: Value,
    ) -> Result<Value, EntityResolverError> {
        let serialized = payload.to_string();
        if serialized.len() > MAX_ENTITY_MODEL_INPUT_BYTES {
            return Err(invalid("model input exceeds 16 KiB limit"));
        }
        let out = self
            .endpoint
            .complete_json(prompt, &serialized, name, &schema)?;
        if serde_json::to_vec(&out)
            .map_err(|e| invalid(&e.to_string()))?
            .len()
            > 4096
        {
            return Err(invalid("model response exceeds 4 KiB limit"));
        }
        Ok(out)
    }
}

fn materialization_describes_transient(surface: &str, summary: &str) -> bool {
    let surface = surface.trim().to_ascii_lowercase();
    let summary = summary.to_ascii_lowercase();
    [
        "source-control branch",
        "source control branch",
        "git branch",
        "temporary branch",
        "task branch",
        "temporary worktree",
        "task worktree",
        "benchmark run",
        "test run",
        "build instance",
        "deployment instance",
        "session instance",
        "dated snapshot",
    ]
    .iter()
    .any(|phrase| summary.contains(phrase))
        || summary.contains(&format!("branch named {surface}"))
        || summary.contains(&format!("worktree named {surface}"))
}

fn materialization_describes_wrapper_category(kind: &str, summary: &str) -> bool {
    if kind != "domain_entity" && kind != "other" {
        return false;
    }
    let summary = summary.to_ascii_lowercase();
    [
        "category of",
        "general category",
        "class of systems",
        "class of tools",
        "class of applications",
        "broad concept",
        "general concept",
    ]
    .iter()
    .any(|phrase| summary.contains(phrase))
}

fn is_bare_generic_surface(value: &str) -> bool {
    const GENERIC: &[&str] = &[
        "agent",
        "application",
        "assistant",
        "client",
        "code",
        "data",
        "deployment",
        "devtool",
        "documentation",
        "entry",
        "file",
        "flow",
        "implementation",
        "lane",
        "pipeline",
        "project",
        "repository",
        "request",
        "response",
        "script",
        "scripts",
        "server",
        "session",
        "shader",
        "state",
        "status",
        "system",
        "tool",
        "tooling",
        "user",
    ];
    let normalized = value.trim().to_ascii_lowercase();
    GENERIC.contains(&normalized.strip_prefix("the ").unwrap_or(&normalized))
}

fn is_obvious_transient_occurrence(surface: &str, context: &str) -> bool {
    let surface = surface.trim();
    let lower = surface.to_ascii_lowercase();
    let context = context.to_ascii_lowercase();
    let context_plain = context.replace(['`', '\'', '"'], "");
    if ["branch", "worktree"].iter().any(|kind| {
        context_plain.contains(&format!("{lower} {kind}"))
            || context_plain.contains(&format!("{kind} {lower}"))
    }) {
        return true;
    }
    if let Some(value) = lower.strip_prefix("commit ") {
        if looks_like_git_object_id(value.trim_matches(|c: char| matches!(c, '`' | '\'' | '"'))) {
            return true;
        }
    }
    if looks_like_git_object_id(surface)
        && ["commit", "rollback", "revision"]
            .iter()
            .any(|word| context.contains(word))
    {
        return true;
    }
    explicit_numbered_occurrence(&lower, "build")
        || explicit_numbered_occurrence(&lower, "benchmark run")
        || explicit_numbered_occurrence(&lower, "test run")
        || explicit_numbered_occurrence(&lower, "deployment")
        || explicit_id_occurrence(&lower, "request id")
        || explicit_id_occurrence(&lower, "response id")
        || explicit_id_occurrence(&lower, "run id")
        || explicit_id_occurrence(&lower, "build id")
        || explicit_id_occurrence(&lower, "deployment id")
        || explicit_id_occurrence(&lower, "session id")
}

fn looks_like_git_object_id(value: &str) -> bool {
    (6..=40).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn explicit_numbered_occurrence(value: &str, prefix: &str) -> bool {
    let Some(rest) = value.strip_prefix(prefix) else {
        return false;
    };
    let rest = rest.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '#' | ':' | '-'));
    !rest.is_empty()
        && rest.bytes().all(|byte| {
            byte.is_ascii_digit() || byte.is_ascii_hexdigit() || matches!(byte, b'-' | b'_' | b'.')
        })
}
fn explicit_id_occurrence(value: &str, prefix: &str) -> bool {
    let Some(rest) = value.strip_prefix(prefix) else {
        return false;
    };
    let rest = rest.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, ':' | '#' | '='));
    rest.len() >= 4
        && rest
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/'))
}

fn parse_reason(v: &str) -> Result<EntityResolutionReason, EntityResolverError> {
    Ok(match v {
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
        _ => return Err(invalid("unknown resolution reason")),
    })
}
fn parse_kind(v: &str) -> Result<&'static str, EntityResolverError> {
    Ok(match v {
        "application" => "application",
        "code_component" => "code_component",
        "code_symbol" => "code_symbol",
        "code_type" => "code_type",
        "data_format" => "data_format",
        "directory" => "directory",
        "domain_entity" => "domain_entity",
        "enum_value" => "enum_value",
        "file" => "file",
        "input_action" => "input_action",
        "programming_language" => "programming_language",
        "repository" => "repository",
        "runtime_entity" => "runtime_entity",
        "service" => "service",
        "subsystem" => "subsystem",
        "tool" => "tool",
        "tool_service" => "tool_service",
        "ui_component" => "ui_component",
        "other" => "other",
        "unknown" => "unknown",
        _ => return Err(invalid("unknown Entity kind")),
    })
}
fn required<'a>(v: &'a Value, k: &str) -> Result<&'a str, EntityResolverError> {
    v.get(k)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(&format!("missing string field {k}")))
}
fn index(v: &Value) -> Result<usize, EntityResolverError> {
    v.get("target_candidate_index")
        .and_then(Value::as_i64)
        .and_then(|i| usize::try_from(i).ok())
        .ok_or_else(|| invalid("target candidate index must be nonnegative integer"))
}
fn invalid(m: &str) -> EntityResolverError {
    EntityResolverError::InvalidOutput(m.to_owned())
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
fn hex(b: &[u8; 32]) -> String {
    b.iter().map(|v| format!("{v:02x}")).collect()
}
fn truncate(v: &str, n: usize) -> &str {
    let mut e = v.len().min(n);
    while !v.is_char_boundary(e) {
        e -= 1;
    }
    &v[..e]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResolutionDecision;
    use reliquary_perception_store::{Entity, EntityId};
    use std::sync::Mutex;
    struct Stub(Mutex<Vec<Value>>);
    struct DecisionStub(Option<Value>);
    impl DecisionEndpoint for DecisionStub {
        fn model(&self) -> &str {
            "decision-test"
        }
        fn evaluate(
            &self,
            _: &Value,
            _: &Value,
        ) -> Result<Value, reliquary_switchboard::DecisionEndpointError> {
            self.0
                .clone()
                .ok_or(reliquary_switchboard::DecisionEndpointError::InvalidResponse("unavailable"))
        }
    }
    impl GeneralEndpoint for Stub {
        fn model(&self) -> &str {
            "entity-test"
        }
        fn complete_json(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: &Value,
        ) -> Result<Value, GeneralEndpointError> {
            self.0
                .lock()
                .unwrap()
                .pop()
                .ok_or(GeneralEndpointError::InvalidResponse("no queued response"))
        }
    }
    fn body() -> MemoryBody {
        MemoryBody {
            title: "Using Atlas service".into(),
            content: "Atlas service stores project state.".into(),
        }
    }
    fn candidate() -> EntityCandidate {
        EntityCandidate {
            entity: Entity {
                id: EntityId([7; 32]),
                revision: 1,
                canonical_name: "Atlas service".into(),
                aliases: vec![],
                kind: "service".into(),
                summary: "Persistent service".into(),
                mutation_id: String::new(),
                created_at_ns: 0,
                updated_at_ns: 0,
            },
            exact_surface: true,
            normalized_surface: false,
            alias_surface: false,
            source_association: false,
            lexical_memory_hits: 0,
            graph_neighbor_hits: 0,
            best_lexical_score: 0.0,
            supporting_memory_ids: vec![],
        }
    }
    fn mention() -> MentionSpan {
        MentionSpan {
            field: TextField::Title,
            start_byte: 6,
            end_byte: 19,
            text: "Atlas service".into(),
        }
    }

    #[test]
    fn decision_fast_path_requires_confidence_margin_and_falls_back() {
        let response = |selected: f64, other: f64| json!({"answers":{"resolution":{"type":"choice","choice":"candidate_0","probabilities":{"candidate_0":selected,"unresolved":other}}}});
        let evaluator = EntityModelEvaluator::new(Stub(Mutex::new(vec![])))
            .with_decision_endpoint(Some(Arc::new(DecisionStub(Some(response(0.97, 0.01))))));
        assert_eq!(
            evaluator
                .resolve(&body(), &mention(), &[candidate()])
                .unwrap()
                .decision,
            ResolutionDecision::ResolveExisting(EntityId([7; 32]))
        );
        let fallback = json!({"decision":"unresolved","reason":"ambiguous","target_candidate_index":0,"identity_relation":"uncertain","mention_kind":"unknown"});
        for decision in [
            Some(response(0.94, 0.01)),
            Some(response(0.96, 0.83)),
            Some(json!({"invalid":true})),
            None,
        ] {
            let evaluator = EntityModelEvaluator::new(Stub(Mutex::new(vec![fallback.clone()])))
                .with_decision_endpoint(Some(Arc::new(DecisionStub(decision))));
            assert_eq!(
                evaluator
                    .resolve(&body(), &mention(), &[candidate()])
                    .unwrap()
                    .decision,
                ResolutionDecision::Unresolved
            );
        }
        let mut unrelated = candidate();
        unrelated.exact_surface = false;
        let evaluator = EntityModelEvaluator::new(Stub(Mutex::new(vec![fallback])))
            .with_decision_endpoint(Some(Arc::new(DecisionStub(Some(response(0.99, 0.0))))));
        assert_eq!(
            evaluator
                .resolve(&body(), &mention(), &[unrelated])
                .unwrap()
                .decision,
            ResolutionDecision::Unresolved
        );
    }

    #[test]
    fn resolver_response_is_guarded_by_exact_span_kind_and_candidate_identity() {
        let response = json!({"decision":"resolve_existing","reason":"context_match","target_candidate_index":0,"identity_relation":"same_identity","mention_kind":"service"});
        let evaluator =
            EntityModelEvaluator::new(Stub(Mutex::new(vec![response.clone(), response])));
        let out = evaluator
            .resolve(&body(), &mention(), &[candidate()])
            .unwrap();
        assert_eq!(
            out.decision,
            ResolutionDecision::ResolveExisting(EntityId([7; 32]))
        );
        let mut wrong = mention();
        wrong.text = "Atlas services".into();
        assert!(evaluator.resolve(&body(), &wrong, &[candidate()]).is_err());
    }

    #[test]
    fn resolver_rejects_out_of_range_index_and_keeps_kind_conflicts_unresolved() {
        let out_of_range = json!({"decision":"resolve_existing","reason":"context_match","target_candidate_index":1,"identity_relation":"same_identity","mention_kind":"service"});
        let evaluator = EntityModelEvaluator::new(Stub(Mutex::new(vec![out_of_range])));
        assert!(
            evaluator
                .resolve(&body(), &mention(), &[candidate()])
                .is_err()
        );

        let wrong_kind = json!({"decision":"resolve_existing","reason":"context_match","target_candidate_index":0,"identity_relation":"same_identity","mention_kind":"tool"});
        let evaluator = EntityModelEvaluator::new(Stub(Mutex::new(vec![wrong_kind])));
        let out = evaluator
            .resolve(&body(), &mention(), &[candidate()])
            .unwrap();
        assert_eq!(out.decision, crate::ResolutionDecision::Unresolved);
    }

    #[test]
    fn admission_requires_recurrence_for_fine_grained_entities() {
        let response = json!({"decision":"create_new","reason":"first_seen_identity","promotion_policy":"immediate","entity_kind":"code_symbol","entity_summary":"A stable symbol."});
        let evaluator = EntityModelEvaluator::new(Stub(Mutex::new(vec![response])));
        let unverified_context = vec!["Another Memory mentions Atlas service.".to_owned()];
        let out = evaluator
            .admit(&body(), &mention(), &unverified_context)
            .unwrap();
        assert_eq!(out.decision, AdmissionDecision::Unresolved);
        assert_eq!(out.reason, EntityResolutionReason::RecurrenceRequired);
        assert!(out.materialization.is_none());
    }

    #[test]
    fn generic_and_transient_surfaces_reject_before_model_call() {
        let empty = EntityModelEvaluator::new(Stub(Mutex::new(vec![])));
        let generic_body = MemoryBody {
            title: "agent".into(),
            content: String::new(),
        };
        let generic = MentionSpan {
            field: TextField::Title,
            start_byte: 0,
            end_byte: 5,
            text: "agent".into(),
        };
        let out = empty.resolve(&generic_body, &generic, &[]).unwrap();
        assert_eq!(out.decision, crate::ResolutionDecision::Reject);
        assert_eq!(out.reason, EntityResolutionReason::GenericRole);

        let transient_body = MemoryBody {
            title: "Build 42 failed".into(),
            content: String::new(),
        };
        let transient = MentionSpan {
            field: TextField::Title,
            start_byte: 0,
            end_byte: 8,
            text: "Build 42".into(),
        };
        let out = empty
            .resolve(&transient_body, &transient, &[candidate()])
            .unwrap();
        assert_eq!(out.decision, crate::ResolutionDecision::Reject);
        assert_eq!(out.reason, EntityResolutionReason::TransientValue);
        let out = empty.admit(&transient_body, &transient, &[]).unwrap();
        assert_eq!(out.decision, AdmissionDecision::Reject);
        assert_eq!(out.reason, EntityResolutionReason::TransientValue);
    }

    #[test]
    fn admission_validates_policy_and_rejects_transient_or_wrapper_materialization() {
        let body = MemoryBody {
            title: "clientv2".into(),
            content: "Use clientv2 for this release.".into(),
        };
        let mention = MentionSpan {
            field: TextField::Title,
            start_byte: 0,
            end_byte: 8,
            text: "clientv2".into(),
        };
        let responses = vec![
            json!({"decision":"create_new","reason":"first_seen_identity","promotion_policy":"unknown","entity_kind":"tool","entity_summary":"A release management tool."}),
            json!({"decision":"create_new","reason":"first_seen_identity","promotion_policy":"immediate","entity_kind":"domain_entity","entity_summary":"A general category of tools."}),
            json!({"decision":"create_new","reason":"first_seen_identity","promotion_policy":"immediate","entity_kind":"other","entity_summary":"The source-control branch named clientv2."}),
        ];
        let evaluator = EntityModelEvaluator::new(Stub(Mutex::new(responses)));
        let transient = evaluator.admit(&body, &mention, &[]).unwrap();
        assert_eq!(transient.decision, AdmissionDecision::Reject);
        assert_eq!(transient.reason, EntityResolutionReason::TransientValue);
        assert!(transient.materialization.is_none());
        let wrapper = evaluator.admit(&body, &mention, &[]).unwrap();
        assert_eq!(wrapper.decision, AdmissionDecision::Reject);
        assert_eq!(wrapper.reason, EntityResolutionReason::WrapperCategory);
        assert!(wrapper.materialization.is_none());
        assert!(evaluator.admit(&body, &mention, &[]).is_err());
    }
}
