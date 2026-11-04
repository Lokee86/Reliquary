use crate::{ContextEvidence, EvidenceKind};
use reliquary_archive::{EchoEvent, EchoEventKind};
pub fn render_assistant_content_from_echo(final_text: &str, events: &[EchoEvent]) -> String {
    let evidence = events
        .iter()
        .filter(|e| e.kind != EchoEventKind::ToolContextAccepted)
        .map(|e| ContextEvidence {
            kind: match e.kind {
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
            model_round: e.model_round,
            correlation_id: e.correlation_id.clone(),
            name: e.name.clone(),
            content: e.content.clone(),
        })
        .collect::<Vec<_>>();
    render_assistant_content(final_text, &evidence)
}
pub fn render_assistant_content(final_text: &str, evidence: &[ContextEvidence]) -> String {
    let items = evidence
        .iter()
        .filter_map(format_evidence)
        .collect::<Vec<_>>();
    if items.is_empty() {
        final_text.to_owned()
    } else {
        format!(
            "[Execution context for this prior assistant turn]\n{}\n[Final assistant response]\n{}",
            items.join("\n"),
            final_text
        )
    }
}
fn format_evidence(e: &ContextEvidence) -> Option<String> {
    let label = match e.kind {
        EvidenceKind::ReasoningSummary => "reasoning_summary",
        EvidenceKind::Commentary => "commentary",
        EvidenceKind::ReasoningTrace => "reasoning_trace",
        EvidenceKind::ToolCall => "tool_call",
        EvidenceKind::ToolResult => "tool_result",
        EvidenceKind::Activity => return None,
    };
    let mut header = format!("[{label}");
    if let Some(v) = e.model_round {
        header.push_str(&format!(" round={v}"))
    }
    if let Some(v) = e.name.as_deref() {
        header.push_str(&format!(" name={v}"))
    }
    if let Some(v) = e.correlation_id.as_deref() {
        header.push_str(&format!(" id={v}"))
    }
    header.push(']');
    Some(format!("{header}\n{}", e.content))
}
