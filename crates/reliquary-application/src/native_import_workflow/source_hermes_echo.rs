//! Host-redacted Echo admission and typed projection for copied Hermes messages.
use super::*;
use reliquary_owner_session::model::{EchoEvent, EchoEventKind, HERMES_MODEL_ROUND_ECHO};
use std::collections::BTreeMap;

pub(super) const MAX_ECHO_EVENTS: usize = 1_024;
const MAX_ECHO_TEXT_BYTES: usize = 16_384;

/// One host-redacted, model-visible event tied to a message within the copy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HermesCopiedEcho {
    pub message_uid: String,
    pub sequence: u64,
    pub timestamp_ns: i64,
    pub model_round: u32,
    pub kind: HermesCopiedEchoKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HermesCopiedEchoKind {
    ModelRoundStarted,
    ModelRoundCompleted,
    ReasoningSummary {
        redacted_summary: String,
    },
    ToolCall {
        call_id: String,
        tool_name: String,
        redacted_arguments: String,
    },
    ToolResult {
        call_id: String,
        outcome: HermesCopiedToolOutcome,
        redacted_result: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HermesCopiedToolOutcome {
    Succeeded,
    Failed,
    Cancelled,
}

impl HermesCopiedToolOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

pub(super) fn normalize_echo(
    snapshot: &HermesCopiedSnapshot,
    conversation_id: &str,
    evidence: &[HermesCopiedEcho],
) -> Result<Vec<EchoEvent>, NativeImportSourceError> {
    let message_ids: BTreeSet<_> = snapshot
        .messages
        .iter()
        .map(|message| message.message_uid.as_str())
        .collect();
    let mut previous = None;
    let mut calls = BTreeMap::new();
    let mut text_bytes = 0;
    let mut events = Vec::with_capacity(evidence.len());
    for event in evidence {
        let anchor = event.message_uid.as_str();
        let key = (anchor, event.sequence);
        if !message_ids.contains(anchor) || event.timestamp_ns < 0 || previous == Some(key) {
            return Err(invalid("invalid Hermes Echo source or duplicate sequence"));
        }
        previous = Some(key);
        let link = |call_id| (anchor, event.model_round, call_id);
        let (kind, correlation_id, name, content) = match &event.kind {
            HermesCopiedEchoKind::ModelRoundStarted => (
                EchoEventKind::ActivityStarted,
                Some(HERMES_MODEL_ROUND_ECHO.into()),
                None,
                "",
            ),
            HermesCopiedEchoKind::ModelRoundCompleted => (
                EchoEventKind::ActivityCompleted,
                Some(HERMES_MODEL_ROUND_ECHO.into()),
                None,
                "",
            ),
            HermesCopiedEchoKind::ReasoningSummary { redacted_summary } => (
                EchoEventKind::ReasoningSummary,
                None,
                None,
                redacted_summary.as_str(),
            ),
            HermesCopiedEchoKind::ToolCall {
                call_id,
                tool_name,
                redacted_arguments,
            } => {
                if !valid_echo_identifier(call_id) || !valid_echo_identifier(tool_name) {
                    return Err(invalid("invalid Hermes Echo tool ID or name"));
                }
                if calls.insert(link(call_id.as_str()), false).is_some() {
                    return Err(invalid("duplicate Hermes Echo tool call"));
                }
                (
                    EchoEventKind::ToolCall,
                    Some(call_id.clone()),
                    Some(tool_name.clone()),
                    redacted_arguments.as_str(),
                )
            }
            HermesCopiedEchoKind::ToolResult {
                call_id,
                outcome,
                redacted_result,
            } => {
                let call = calls.get_mut(&link(call_id.as_str())).ok_or_else(|| {
                    invalid(
                        "Hermes Echo tool result lacks a preceding call for its source and round",
                    )
                })?;
                if std::mem::replace(call, true) {
                    return Err(invalid("duplicate Hermes Echo tool result"));
                }
                (
                    EchoEventKind::ToolResult,
                    Some(call_id.clone()),
                    Some(outcome.as_str().into()),
                    redacted_result.as_str(),
                )
            }
        };
        text_bytes += content.len();
        if content.len() > MAX_ECHO_TEXT_BYTES || text_bytes > MAX_COPY_BYTES {
            return Err(NativeImportSourceError::Limit(
                "Hermes Echo text exceeds its budget",
            ));
        }
        events.push(EchoEvent {
            conversation_id: conversation_id.to_owned(),
            message_id: event.message_uid.clone(),
            sequence: event.sequence,
            timestamp_ns: event.timestamp_ns,
            model_round: Some(event.model_round),
            kind,
            correlation_id,
            name,
            content: content.to_owned(),
        });
    }
    Ok(events)
}

fn valid_echo_identifier(value: &str) -> bool {
    (1..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.:-".contains(&byte))
}

pub(super) fn copied_echo(event: EchoEvent) -> Option<HermesCopiedEcho> {
    let kind = match event.kind {
        EchoEventKind::ActivityStarted
            if event.correlation_id.as_deref() == Some(HERMES_MODEL_ROUND_ECHO) =>
        {
            HermesCopiedEchoKind::ModelRoundStarted
        }
        EchoEventKind::ActivityCompleted
            if event.correlation_id.as_deref() == Some(HERMES_MODEL_ROUND_ECHO) =>
        {
            HermesCopiedEchoKind::ModelRoundCompleted
        }
        EchoEventKind::ReasoningSummary => HermesCopiedEchoKind::ReasoningSummary {
            redacted_summary: event.content,
        },
        EchoEventKind::ToolCall => HermesCopiedEchoKind::ToolCall {
            call_id: event.correlation_id?,
            tool_name: event.name?,
            redacted_arguments: event.content,
        },
        EchoEventKind::ToolResult => HermesCopiedEchoKind::ToolResult {
            call_id: event.correlation_id?,
            outcome: match event.name.as_deref()? {
                "succeeded" => HermesCopiedToolOutcome::Succeeded,
                "failed" => HermesCopiedToolOutcome::Failed,
                "cancelled" => HermesCopiedToolOutcome::Cancelled,
                _ => return None,
            },
            redacted_result: event.content,
        },
        _ => return None,
    };
    Some(HermesCopiedEcho {
        message_uid: event.message_id,
        sequence: event.sequence,
        timestamp_ns: event.timestamp_ns,
        model_round: event.model_round?,
        kind,
    })
}
