use crate::echo_codec::{decode, encode};
use crate::{Container, EchoError, EchoEvent};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Default)]
pub(crate) struct EchoStore {
    records: HashMap<(String, String), BTreeMap<u64, EchoEvent>>,
}

impl EchoStore {
    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), EchoError> {
        if let Some(event) = decode(payload)? {
            self.apply(event)?;
        }
        Ok(())
    }

    pub(crate) fn put(
        &mut self,
        container: &mut Container,
        event: EchoEvent,
    ) -> Result<bool, EchoError> {
        if let Some(existing) = self.existing(&event) {
            return if existing == &event {
                Ok(false)
            } else {
                Err(EchoError::ConflictingSequence)
            };
        }
        let payload = encode(&event)?;
        container.append(&payload)?;
        self.insert(event);
        Ok(true)
    }

    pub(crate) fn for_turn(&self, conversation_id: &str, message_id: &str) -> Vec<EchoEvent> {
        self.records
            .get(&(conversation_id.to_owned(), message_id.to_owned()))
            .map(|events| events.values().cloned().collect())
            .unwrap_or_default()
    }

    pub(crate) fn for_conversation(&self, conversation_id: &str) -> Vec<EchoEvent> {
        let mut records = self
            .records
            .iter()
            .filter(|((conversation, _), _)| conversation == conversation_id)
            .flat_map(|(_, events)| events.values().cloned())
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            left.message_id
                .cmp(&right.message_id)
                .then(left.sequence.cmp(&right.sequence))
        });
        records
    }

    pub(crate) fn all_records(&self) -> Vec<EchoEvent> {
        let mut records = self
            .records
            .values()
            .flat_map(|events| events.values().cloned())
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            left.conversation_id
                .cmp(&right.conversation_id)
                .then(left.message_id.cmp(&right.message_id))
                .then(left.sequence.cmp(&right.sequence))
        });
        records
    }

    fn apply(&mut self, event: EchoEvent) -> Result<(), EchoError> {
        if let Some(existing) = self.existing(&event) {
            return if existing == &event {
                Ok(())
            } else {
                Err(EchoError::ConflictingSequence)
            };
        }
        self.insert(event);
        Ok(())
    }

    fn existing(&self, event: &EchoEvent) -> Option<&EchoEvent> {
        self.records
            .get(&(event.conversation_id.clone(), event.message_id.clone()))
            .and_then(|events| events.get(&event.sequence))
    }

    fn insert(&mut self, event: EchoEvent) {
        let key = (event.conversation_id.clone(), event.message_id.clone());
        self.records
            .entry(key)
            .or_default()
            .insert(event.sequence, event);
    }
}
