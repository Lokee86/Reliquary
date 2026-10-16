use super::{ContextEvidence, EvidenceKind};
use crate::{EchoEvent, EchoEventKind};

pub fn render_assistant_content_from_echo(final_text: &str, events: &[EchoEvent]) -> String {
    let evidence = events
        .iter()
        .filter(|event| event.kind != EchoEventKind::ToolContextAccepted)
        .map(context_evidence_from_echo)
        .collect::<Vec<_>>();
    render_assistant_content(final_text, &evidence)
}

fn context_evidence_from_echo(event: &EchoEvent) -> ContextEvidence {
    ContextEvidence {
        kind: match event.kind {
            EchoEventKind::ReasoningSummary => EvidenceKind::ReasoningSummary,
            EchoEventKind::Commentary => EvidenceKind::Commentary,
            EchoEventKind::ReasoningTrace => EvidenceKind::ReasoningTrace,
            EchoEventKind::ToolCall => EvidenceKind::ToolCall,
            EchoEventKind::ToolResult => EvidenceKind::ToolResult,
            EchoEventKind::ToolContextAccepted
            | EchoEventKind::ActivityStarted
            | EchoEventKind::ActivityCompleted
            | EchoEventKind::ActivityFailed => EvidenceKind::Activity,
        },
        model_round: event.model_round,
        correlation_id: event.correlation_id.clone(),
        name: event.name.clone(),
        content: event.content.clone(),
    }
}

pub fn render_assistant_content(final_text: &str, evidence: &[ContextEvidence]) -> String {
    let evidence = evidence
        .iter()
        .filter_map(format_evidence)
        .collect::<Vec<_>>();
    if evidence.is_empty() {
        return final_text.to_owned();
    }
    format!(
        "[Warlock execution context for this prior assistant turn]\n{}\n[Final assistant response]\n{}",
        evidence.join("\n"),
        final_text
    )
}

fn format_evidence(event: &ContextEvidence) -> Option<String> {
    let label = match event.kind {
        EvidenceKind::ReasoningSummary => "reasoning_summary",
        EvidenceKind::Commentary => "commentary",
        EvidenceKind::ReasoningTrace => "reasoning_trace",
        EvidenceKind::ToolCall => "tool_call",
        EvidenceKind::ToolResult => "tool_result",
        EvidenceKind::Activity => return None,
    };
    let mut header = format!("[{label}");
    if let Some(round) = event.model_round {
        header.push_str(&format!(" round={round}"));
    }
    if let Some(name) = event.name.as_deref() {
        header.push_str(&format!(" name={name}"));
    }
    if let Some(id) = event.correlation_id.as_deref() {
        header.push_str(&format!(" id={id}"));
    }
    header.push(']');
    Some(format!("{header}\n{}", event.content))
}
