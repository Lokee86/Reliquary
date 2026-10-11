use super::{
    ReliquaryRuntimeHost, ReliquaryRuntimeHostError, semantic_access::RuntimeSemanticOwnerKind,
};
pub use crate::runtime_shared::{RuntimeEchoSource, RuntimeEchoTurn};

impl ReliquaryRuntimeHost {
    pub fn read_echo_source_for(
        &self,
        owner_id: &str,
        conversation_id: &str,
        start_message_id: &str,
        end_message_id: &str,
    ) -> Result<RuntimeEchoSource, ReliquaryRuntimeHostError> {
        let owner = self.semantic_owner(owner_id)?;
        if owner.kind != RuntimeSemanticOwnerKind::Reliquary {
            return Err(ReliquaryRuntimeHostError::Operation(
                "Echo source requires a Reliquary owner".into(),
            ));
        }
        let transcript =
            self.conversation_transcript_for(owner_id, conversation_id, end_message_id)?;
        let start = transcript
            .iter()
            .position(|turn| turn.message_id == start_message_id)
            .ok_or_else(|| {
                ReliquaryRuntimeHostError::Operation(
                    "Echo source start is not on the returned transcript path".into(),
                )
            })?;
        if transcript.last().map(|turn| turn.message_id.as_str()) != Some(end_message_id) {
            return Err(ReliquaryRuntimeHostError::Operation(
                "Echo source end is not the requested transcript leaf".into(),
            ));
        }
        let mut turns = Vec::new();
        for turn in &transcript[start..] {
            let events = self.echo_events_for(owner_id, conversation_id, &turn.message_id)?;
            if !events.is_empty() {
                turns.push(RuntimeEchoTurn {
                    message_id: turn.message_id.clone(),
                    events,
                });
            }
        }
        Ok(RuntimeEchoSource {
            owner_id: owner_id.to_owned(),
            conversation_id: conversation_id.to_owned(),
            start_message_id: start_message_id.to_owned(),
            end_message_id: end_message_id.to_owned(),
            turns,
        })
    }
}
