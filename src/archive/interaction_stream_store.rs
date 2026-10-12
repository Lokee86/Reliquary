use crate::interaction_stream_codec::{decode, encode};
use crate::{Container, CvaError, InteractionStreamRecord, InteractionStreamStatus};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub(crate) struct InteractionStreamStore {
    records: HashMap<String, InteractionStreamRecord>,
}

impl InteractionStreamStore {
    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), CvaError> {
        if let Some(record) = decode(payload)? {
            self.apply(record)?;
        }
        Ok(())
    }

    pub(crate) fn put(
        &mut self,
        container: &mut Container,
        record: InteractionStreamRecord,
    ) -> Result<(), CvaError> {
        self.validate_transition(&record)?;
        container.append(&encode(&record)?)?;
        self.records.insert(record.message_id.clone(), record);
        Ok(())
    }

    pub(crate) fn record(&self, message_id: &str) -> Option<&InteractionStreamRecord> {
        self.records.get(message_id)
    }

    pub(crate) fn records_for_session(&self, session_id: &str) -> Vec<InteractionStreamRecord> {
        self.records
            .values()
            .filter(|record| record.session_id == session_id)
            .cloned()
            .collect()
    }

    pub(crate) fn borrowed_records_for_session(
        &self,
        session_id: &str,
    ) -> impl Iterator<Item = &InteractionStreamRecord> {
        self.records
            .values()
            .filter(move |record| record.session_id == session_id)
    }

    pub(crate) fn all_records(&self) -> Vec<InteractionStreamRecord> {
        self.records.values().cloned().collect()
    }

    fn apply(&mut self, record: InteractionStreamRecord) -> Result<(), CvaError> {
        self.validate_transition(&record)?;
        self.records.insert(record.message_id.clone(), record);
        Ok(())
    }

    fn validate_transition(&self, next: &InteractionStreamRecord) -> Result<(), CvaError> {
        if next.message_id.is_empty() || next.session_id.is_empty() {
            return Err(CvaError::InteractionStream(
                "interaction stream IDs must not be empty".into(),
            ));
        }
        if let Some(current) = self.records.get(&next.message_id) {
            if current.session_id != next.session_id
                || current.parent_message_id != next.parent_message_id
                || current.role != next.role
                || current.timestamp_ns != next.timestamp_ns
            {
                return Err(CvaError::InteractionStream(
                    "interaction stream metadata changed across checkpoints".into(),
                ));
            }
            if !next.content.starts_with(&current.content) {
                return Err(CvaError::InteractionStream(
                    "interaction stream content is not append-only".into(),
                ));
            }
            if current.status == InteractionStreamStatus::Interrupted
                && next.status == InteractionStreamStatus::Streaming
            {
                return Err(CvaError::InteractionStream(
                    "interrupted interaction stream cannot resume".into(),
                ));
            }
        }
        Ok(())
    }
}
