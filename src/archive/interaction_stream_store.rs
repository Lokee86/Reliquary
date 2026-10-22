use crate::interaction_stream_codec::{CompactRecord, decode_compact, encode_compact};
use crate::{Container, CvaError, InteractionStreamRecord, InteractionStreamStatus};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub(crate) struct InteractionStreamStore {
    records: HashMap<String, InteractionStreamRecord>,
}

impl InteractionStreamStore {
    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), CvaError> {
        if let Some(record) = decode_compact(payload)? {
            self.apply_compact(record)?;
        }
        Ok(())
    }

    pub(crate) fn put(
        &mut self,
        container: &mut Container,
        record: InteractionStreamRecord,
    ) -> Result<(), CvaError> {
        self.validate_transition(&record)?;
        u32::try_from(record.content.len()).map_err(|_| {
            CvaError::InteractionStream("interaction stream content exceeds u32 length".into())
        })?;
        let mut frames = Vec::new();
        let previous = self.records.get(&record.message_id);
        let base_len = previous.map_or(0, |previous| previous.content.len());
        if previous.is_none() {
            let mut begin = record.clone();
            begin.content.clear();
            begin.status = InteractionStreamStatus::Streaming;
            frames.push(encode_compact(&CompactRecord::Begin(begin))?);
        }
        if record.content.len() > base_len {
            frames.push(encode_compact(&CompactRecord::Append {
                message_id: record.message_id.clone(),
                base_len: base_len as u32,
                suffix: record.content[base_len..].to_owned(),
            })?);
        }
        if record.status == InteractionStreamStatus::Interrupted
            && previous
                .is_none_or(|previous| previous.status != InteractionStreamStatus::Interrupted)
        {
            frames.push(encode_compact(&CompactRecord::Interrupted {
                message_id: record.message_id.clone(),
                content_len: record.content.len() as u32,
            })?);
        }
        for frame in frames {
            container.append(&frame)?;
            // Admit every durable prefix, including a checkpoint interrupted by I/O failure.
            self.ingest(&frame)?;
        }
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

    fn apply_compact(&mut self, record: CompactRecord) -> Result<(), CvaError> {
        match record {
            CompactRecord::Begin(begin) => {
                if let Some(current) = self.records.get(&begin.message_id) {
                    if !same_metadata(current, &begin) {
                        return Err(CvaError::InteractionStream(
                            "interaction stream Begin metadata conflict".into(),
                        ));
                    }
                } else {
                    self.validate_transition(&begin)?;
                    self.records.insert(begin.message_id.clone(), begin);
                }
            }
            CompactRecord::Append {
                message_id,
                base_len,
                suffix,
            } => {
                let current = self.records.get_mut(&message_id).ok_or_else(|| {
                    CvaError::InteractionStream("interaction stream Append before Begin".into())
                })?;
                let base = base_len as usize;
                if current.content.len() == base {
                    current.content.try_reserve(suffix.len()).map_err(|_| {
                        CvaError::InteractionStream("interaction stream allocation failed".into())
                    })?;
                    current.content.push_str(&suffix);
                } else if base.checked_add(suffix.len()) != Some(current.content.len())
                    || current.content.get(base..) != Some(suffix.as_str())
                {
                    return Err(CvaError::InteractionStream(
                        "interaction stream Append position conflict".into(),
                    ));
                }
            }
            CompactRecord::Interrupted {
                message_id,
                content_len,
            } => {
                let current = self.records.get_mut(&message_id).ok_or_else(|| {
                    CvaError::InteractionStream(
                        "interaction stream Interrupted before Begin".into(),
                    )
                })?;
                if current.content.len() != content_len as usize {
                    return Err(CvaError::InteractionStream(
                        "interaction stream Interrupted position conflict".into(),
                    ));
                }
                current.status = InteractionStreamStatus::Interrupted;
            }
        }
        Ok(())
    }

    fn validate_transition(&self, next: &InteractionStreamRecord) -> Result<(), CvaError> {
        if next.message_id.is_empty() || next.session_id.is_empty() {
            return Err(CvaError::InteractionStream(
                "interaction stream IDs must not be empty".into(),
            ));
        }
        if let Some(current) = self.records.get(&next.message_id) {
            if !same_metadata(current, next) {
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

fn same_metadata(left: &InteractionStreamRecord, right: &InteractionStreamRecord) -> bool {
    left.session_id == right.session_id
        && left.parent_message_id == right.parent_message_id
        && left.role == right.role
        && left.timestamp_ns == right.timestamp_ns
}
