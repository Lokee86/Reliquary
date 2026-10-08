//! Durable native-generation tool receipts, backed by Archive Echo.
//! Trusted embedding callbacks only; no user-supplied owner authority.
use super::*;

impl GatewayRuntime {
    /// Accept one exact tool event for an already running native generation.
    /// Stable Echo sequence/correlation identity supplies retry and ordering.
    /// This service callback must not be bound directly to untrusted requests.
    pub fn record_conversation_generation_tool_event(
        &self,
        owner_id: &str,
        conversation_id: &str,
        operation_id: &str,
        event: crate::EchoEvent,
    ) -> Result<bool, ReliquaryRuntimeHostError> {
        let authority = OperationAuthority::capture(self, owner_id)?;
        let conversation = ConversationId::new(conversation_id).map_err(operation)?;
        let journal = self.conversation.journal(owner_id)?;
        let (key, generation) = journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .generation_by_operation_id(&conversation, operation_id)
            .ok_or_else(|| operation("native generation operation not found"))?;
        let coordinator = self.conversation.coordinator(owner_id, conversation_id)?;
        let _processing = coordinator.acquire(&journal, &key, &generation.branch_id)?;
        let generation = journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .generation(&key)
            .ok_or_else(|| operation("generation not found"))?;
        if event.conversation_id != conversation_id
            || event.message_id != generation.generation_id
            || !matches!(
                event.kind,
                crate::EchoEventKind::ToolCall | crate::EchoEventKind::ToolResult
            )
            || event
                .correlation_id
                .as_ref()
                .is_none_or(|id| id.trim().is_empty() || id.len() > 256)
            || event
                .name
                .as_ref()
                .is_none_or(|name| name.trim().is_empty() || name.len() > 256)
            || event.content.len() > 1024 * 1024
        {
            return Err(operation("tool event identity or shape is invalid"));
        }
        with_operation(self, &authority, |runtime| {
            let prior = runtime
                .cva()
                .echo_events(conversation_id, &generation.generation_id);
            validate_generation_tool_source(&prior)?;
            if let Some(previous) = prior.iter().find(|e| e.sequence == event.sequence) {
                if previous != &event {
                    return Err(operation("tool event sequence conflict"));
                }
                runtime.cva().sync().map_err(operation)?;
                return Ok(false);
            }
            if generation.phase != DurableGenerationPhase::Running {
                return Err(operation(
                    "new tool event requires a running native generation",
                ));
            }
            if prior.len() >= 256
                || prior
                    .iter()
                    .try_fold(event.content.len(), |size, e| {
                        size.checked_add(e.content.len())
                    })
                    .is_none_or(|size| size > 1024 * 1024)
            {
                return Err(operation(
                    "tool source exceeds its bounded receipt contract",
                ));
            }
            let next = prior.last().map_or(Some(0), |e| e.sequence.checked_add(1));
            if next != Some(event.sequence) {
                return Err(operation(
                    "tool event sequence must follow the durable source",
                ));
            }
            let invocation = prior.iter().find(|e| {
                e.kind == crate::EchoEventKind::ToolCall && e.correlation_id == event.correlation_id
            });
            let result = prior.iter().any(|e| {
                e.kind == crate::EchoEventKind::ToolResult
                    && e.correlation_id == event.correlation_id
            });
            match event.kind {
                crate::EchoEventKind::ToolCall if invocation.is_some() || result => {
                    return Err(operation("tool invocation identity already exists"));
                }
                crate::EchoEventKind::ToolResult
                    if invocation.is_none()
                        || result
                        || invocation.is_some_and(|call| call.name != event.name) =>
                {
                    return Err(operation(
                        "tool result requires its exact earlier invocation",
                    ));
                }
                _ => {}
            }
            let changed = runtime.cva.put_echo_event(event).map_err(operation)?;
            runtime.cva().sync().map_err(operation)?;
            Ok(changed)
        })
    }
}

/// Validate the existing immutable source before native receipt projection.
/// Generic historical Echo writes are not proof of well-formed tool ordering.
pub(in super::super) fn validate_generation_tool_source(
    events: &[crate::EchoEvent],
) -> Result<(), ReliquaryRuntimeHostError> {
    if events.len() > 256
        || events
            .iter()
            .try_fold(0usize, |size, event| size.checked_add(event.content.len()))
            .is_none_or(|size| size > 1024 * 1024)
    {
        return Err(operation(
            "tool source exceeds its bounded receipt contract",
        ));
    }
    let mut calls = std::collections::BTreeMap::new();
    let mut results = std::collections::BTreeSet::new();
    for event in events {
        if !matches!(
            event.kind,
            crate::EchoEventKind::ToolCall | crate::EchoEventKind::ToolResult
        ) {
            continue;
        }
        let id = event
            .correlation_id
            .as_deref()
            .filter(|id| !id.trim().is_empty() && id.len() <= 256)
            .ok_or_else(|| operation("invalid tool invocation identity"))?;
        let name = event
            .name
            .as_deref()
            .filter(|name| !name.trim().is_empty() && name.len() <= 256)
            .ok_or_else(|| operation("invalid tool name"))?;
        match event.kind {
            crate::EchoEventKind::ToolCall => {
                if calls.insert(id, name).is_some() {
                    return Err(operation("duplicate tool invocation identity"));
                }
            }
            crate::EchoEventKind::ToolResult => {
                if calls.get(id).copied() != Some(name) || !results.insert(id) {
                    return Err(operation("tool result lacks its exact earlier invocation"));
                }
            }
            _ => {}
        }
    }
    Ok(())
}
