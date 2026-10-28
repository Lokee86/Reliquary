use crate::entity_resolver::{
    EntityResolver, invalid, mention_field, parse_reason, required_string,
};
use crate::entity_resolver_schema::{ENTITY_ADMISSION_SYSTEM_PROMPT, entity_admission_schema};
use crate::{
    EntityAdmissionDecision, EntityAdmissionOutput, EntityCandidateSet, EntityMaterialization,
    EntityResolutionReason, EntityResolverError, GeneralEndpoint, Memory,
};
use serde_json::{Value, json};

pub const MAX_ENTITY_ADMISSION_CONTEXT_MEMORIES: usize = 8;
const MAX_ENTITY_ADMISSION_CONTEXT_TEXT_BYTES: usize = 2048;

impl<E: GeneralEndpoint> EntityResolver<E> {
    pub(crate) fn admit(
        &self,
        memory: &Memory,
        set: &EntityCandidateSet,
        context: &[Memory],
    ) -> Result<EntityAdmissionOutput, EntityResolverError> {
        if !set.candidates.is_empty() {
            return Err(invalid("Entity admission requires an empty candidate set"));
        }
        if is_bare_generic_surface(&set.mention.text) {
            return Ok(EntityAdmissionOutput {
                decision: EntityAdmissionDecision::Reject,
                reason: EntityResolutionReason::GenericRole,
                materialization: None,
            });
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
            "context_evidence": context.iter()
                .take(MAX_ENTITY_ADMISSION_CONTEXT_MEMORIES)
                .map(|value| json!({
                    "memory_id": hex(&value.id.0),
                    "title": truncate_utf8(
                        &value.title,
                        MAX_ENTITY_ADMISSION_CONTEXT_TEXT_BYTES,
                    ),
                    "content": truncate_utf8(
                        &value.content,
                        MAX_ENTITY_ADMISSION_CONTEXT_TEXT_BYTES,
                    ),
                }))
                .collect::<Vec<_>>(),
        });
        let output = self.complete_json(
            ENTITY_ADMISSION_SYSTEM_PROMPT,
            &payload.to_string(),
            "entity_admission_v6",
            &entity_admission_schema(),
        )?;
        enforce_promotion_policy(parse_admission_output(&output)?, context, &set.mention.text)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EntityPromotionPolicy {
    Immediate,
    RequiresRecurrence,
    None,
}

struct ParsedEntityAdmission {
    output: EntityAdmissionOutput,
    promotion_policy: EntityPromotionPolicy,
}

fn parse_admission_output(output: &Value) -> Result<ParsedEntityAdmission, EntityResolverError> {
    let mut decision = match required_string(output, "decision")? {
        "create_new" => EntityAdmissionDecision::CreateNew,
        "unresolved" => EntityAdmissionDecision::Unresolved,
        "reject" => EntityAdmissionDecision::Reject,
        _ => return Err(invalid("unknown admission decision")),
    };
    let reason = parse_reason(required_string(output, "reason")?)?;
    if reason == EntityResolutionReason::RecurrenceRequired {
        decision = EntityAdmissionDecision::Unresolved;
    } else if reason.is_rejection_class() {
        decision = EntityAdmissionDecision::Reject;
    }
    let promotion_policy = match required_string(output, "promotion_policy")? {
        "immediate" => EntityPromotionPolicy::Immediate,
        "requires_recurrence" => EntityPromotionPolicy::RequiresRecurrence,
        "none" => EntityPromotionPolicy::None,
        _ => return Err(invalid("unknown Entity promotion policy")),
    };
    let kind = required_string(output, "entity_kind")?;
    let summary = required_string(output, "entity_summary")?;
    let materialization = if reason == EntityResolutionReason::RecurrenceRequired {
        None
    } else {
        match decision {
            EntityAdmissionDecision::CreateNew => {
                if kind == "unknown" || summary.trim().is_empty() {
                    return Err(invalid("create_new admission requires Entity metadata"));
                }
                if promotion_policy == EntityPromotionPolicy::None {
                    return Err(invalid("create_new admission requires promotion policy"));
                }
                Some(validate_materialization(kind, summary)?)
            }
            EntityAdmissionDecision::Unresolved | EntityAdmissionDecision::Reject => {
                if kind != "unknown" || !summary.is_empty() {
                    return Err(invalid(
                        "non-create admission must not materialize Entity metadata",
                    ));
                }
                None
            }
        }
    };
    Ok(ParsedEntityAdmission {
        output: EntityAdmissionOutput {
            decision,
            reason,
            materialization,
        },
        promotion_policy,
    })
}

fn enforce_promotion_policy(
    mut parsed: ParsedEntityAdmission,
    context: &[Memory],
    surface: &str,
) -> Result<EntityAdmissionOutput, EntityResolverError> {
    if parsed.output.decision != EntityAdmissionDecision::CreateNew {
        return Ok(parsed.output);
    }
    let materialization = parsed
        .output
        .materialization
        .as_ref()
        .ok_or_else(|| invalid("create_new admission missing Entity metadata"))?;
    if materialization_describes_transient(surface, &materialization.summary) {
        parsed.output.decision = EntityAdmissionDecision::Reject;
        parsed.output.reason = EntityResolutionReason::TransientValue;
        parsed.output.materialization = None;
        return Ok(parsed.output);
    }
    if materialization_describes_wrapper_category(&materialization.kind, &materialization.summary) {
        parsed.output.decision = EntityAdmissionDecision::Reject;
        parsed.output.reason = EntityResolutionReason::WrapperCategory;
        parsed.output.materialization = None;
        return Ok(parsed.output);
    }
    let hard_recurrence_kind = matches!(
        materialization.kind.as_str(),
        "code_symbol" | "enum_value" | "input_action"
    );
    if context.is_empty()
        && (parsed.promotion_policy == EntityPromotionPolicy::RequiresRecurrence
            || hard_recurrence_kind)
    {
        parsed.output.decision = EntityAdmissionDecision::Unresolved;
        parsed.output.reason = EntityResolutionReason::RecurrenceRequired;
        parsed.output.materialization = None;
    }
    Ok(parsed.output)
}

fn materialization_describes_transient(surface: &str, summary: &str) -> bool {
    let surface = surface.trim().to_ascii_lowercase();
    let summary = summary.to_ascii_lowercase();
    let transient_phrases = [
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
    ];
    transient_phrases
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

fn validate_materialization(
    kind: &str,
    summary: &str,
) -> Result<EntityMaterialization, EntityResolverError> {
    let summary = summary.trim();
    if kind.is_empty() || kind == "unknown" || kind.len() > crate::MAX_ENTITY_KIND_BYTES {
        return Err(invalid("invalid Entity kind"));
    }
    if summary.is_empty() || summary.len() > crate::MAX_ENTITY_SUMMARY_BYTES {
        return Err(invalid("invalid Entity summary"));
    }
    Ok(EntityMaterialization {
        kind: kind.to_owned(),
        summary: summary.to_owned(),
    })
}

pub(crate) fn is_obvious_transient_occurrence(memory: &Memory, set: &EntityCandidateSet) -> bool {
    let context = match set.mention.field {
        crate::MemoryTextField::Title => memory.title.as_str(),
        crate::MemoryTextField::Content => memory.content.as_str(),
    };
    is_obvious_transient_surface(&set.mention.text, context)
}

fn is_obvious_transient_surface(surface: &str, context: &str) -> bool {
    let surface = surface.trim();
    let lower = surface.to_ascii_lowercase();
    let context = context.to_ascii_lowercase();
    let context_plain = context.replace(['`', '\'', '"'], "");

    if explicitly_branch_or_worktree(&lower, &context_plain) {
        return true;
    }

    if let Some(value) = lower.strip_prefix("commit ") {
        if looks_like_git_object_id(value.trim_matches(|c: char| c == '`' || c == '\'' || c == '"'))
        {
            return true;
        }
    }
    if looks_like_git_object_id(surface)
        && (context.contains("commit")
            || context.contains("rollback")
            || context.contains("revision"))
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

fn explicitly_branch_or_worktree(surface: &str, context: &str) -> bool {
    ["branch", "worktree"].iter().any(|kind| {
        context.contains(&format!("{surface} {kind}"))
            || context.contains(&format!("{kind} {surface}"))
    })
}

fn looks_like_git_object_id(value: &str) -> bool {
    let value = value.trim();
    (6..=40).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn explicit_numbered_occurrence(value: &str, prefix: &str) -> bool {
    let Some(rest) = value.strip_prefix(prefix) else {
        return false;
    };
    let rest =
        rest.trim_start_matches(|c: char| c.is_whitespace() || c == '#' || c == ':' || c == '-');
    !rest.is_empty()
        && rest.bytes().all(|byte| {
            byte.is_ascii_digit() || byte.is_ascii_hexdigit() || matches!(byte, b'-' | b'_' | b'.')
        })
}

fn explicit_id_occurrence(value: &str, prefix: &str) -> bool {
    let Some(rest) = value.strip_prefix(prefix) else {
        return false;
    };
    let rest =
        rest.trim_start_matches(|c: char| c.is_whitespace() || c == ':' || c == '#' || c == '=');
    rest.len() >= 4
        && rest
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/'))
}

pub(crate) fn is_bare_generic_surface(value: &str) -> bool {
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
    let bare = normalized.strip_prefix("the ").unwrap_or(&normalized);
    GENERIC.contains(&bare)
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

#[cfg(test)]
mod transient_tests {
    use super::is_obvious_transient_surface;

    #[test]
    fn rejects_obvious_historical_occurrence_identifiers() {
        for (surface, context) in [
            ("commit 6ed8e7a", "Rollback to commit 6ed8e7a is final."),
            ("6ed8e7a", "The repository rollback targets commit 6ed8e7a."),
            (
                "clientv2",
                "Repository work should target the `clientv2` branch.",
            ),
            ("entity-fix", "Use worktree entity-fix for this task."),
            ("build #1842", "build #1842 failed during packaging."),
            ("benchmark run 17", "benchmark run 17 recorded old results."),
            ("test run 42", "test run 42 completed yesterday."),
            ("deployment #42", "deployment #42 was performed Friday."),
            ("request ID req-1234", "request ID req-1234 timed out."),
            ("response ID resp-88", "response ID resp-88 was cached."),
            ("session ID sess-42", "session ID sess-42 has ended."),
        ] {
            assert!(
                is_obvious_transient_surface(surface, context),
                "{surface} should be transient"
            );
        }
    }

    #[test]
    fn does_not_reject_continuing_identities_by_shape_alone() {
        for (surface, context) in [
            ("DefaultRoomID", "DefaultRoomID is a stable code symbol."),
            ("NetworkClient", "NetworkClient owns client transport."),
            (
                "main.go",
                "services/game-server/cmd/game-server/main.go is the entry file.",
            ),
            ("game.randomRange", "game.randomRange is a stable helper."),
            (
                "Space Rocks repository",
                "The Space Rocks repository continues across releases.",
            ),
        ] {
            assert!(
                !is_obvious_transient_surface(surface, context),
                "{surface} should remain eligible"
            );
        }
    }
}
