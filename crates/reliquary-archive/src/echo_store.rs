use crate::echo_codec::{decode, encode};
use crate::{EchoError, EchoEvent};
use reliquary_container::{Container, ContainerIdentity, FileKind};
use std::collections::{BTreeMap, HashMap};

/// Separately owned source-keyed reasoning/tool evidence. It shares only the
/// physical REL Container with Archive and does not alter Archive activity.
#[derive(Debug, Default)]
pub struct EchoStore {
    owner_id: Option<[u8; 16]>,
    container_identity: Option<ContainerIdentity>,
    records: HashMap<(String, String), BTreeMap<u64, EchoEvent>>,
}

impl EchoStore {
    pub fn open(container: &Container) -> Result<Self, EchoError> {
        if container.identity().file_kind != FileKind::Reliquary {
            return Err(EchoError::WrongFileKind);
        }
        let mut store = Self {
            owner_id: Some(container.owner_id()),
            container_identity: Some(container.identity()),
            records: HashMap::new(),
        };
        for frame in container.read_frames()? {
            store.ingest(&frame.payload)?;
        }
        Ok(store)
    }

    /// Replay one raw frame. Frames owned by Archive or other domains pass through.
    pub fn ingest(&mut self, payload: &[u8]) -> Result<(), EchoError> {
        if let Some(event) = decode(payload)? {
            self.apply(event)?;
        }
        Ok(())
    }

    /// Persist evidence under the same Container owner, then accept it in memory.
    pub fn put(&mut self, container: &mut Container, event: EchoEvent) -> Result<bool, EchoError> {
        self.bind_container(container)?;
        if let Some(existing) = self.existing(&event) {
            return if existing == &event {
                Ok(false)
            } else {
                Err(EchoError::ConflictingSequence)
            };
        }
        let payload = encode(&event)?;
        container.append(&payload)?;
        container.sync()?;
        self.insert(event);
        Ok(true)
    }

    pub fn for_turn(&self, conversation_id: &str, message_id: &str) -> Vec<EchoEvent> {
        self.records
            .get(&(conversation_id.to_owned(), message_id.to_owned()))
            .map(|events| events.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn for_conversation(&self, conversation_id: &str) -> Vec<EchoEvent> {
        let mut events = self
            .records
            .iter()
            .filter(|((conversation, _), _)| conversation == conversation_id)
            .flat_map(|(_, events)| events.values().cloned())
            .collect::<Vec<_>>();
        events.sort_by(|left, right| {
            left.message_id
                .cmp(&right.message_id)
                .then(left.sequence.cmp(&right.sequence))
        });
        events
    }

    pub fn all_records(&self) -> Vec<EchoEvent> {
        let mut events = self
            .records
            .values()
            .flat_map(|events| events.values().cloned())
            .collect::<Vec<_>>();
        events.sort_by(|left, right| {
            left.conversation_id
                .cmp(&right.conversation_id)
                .then(left.message_id.cmp(&right.message_id))
                .then(left.sequence.cmp(&right.sequence))
        });
        events
    }

    pub fn owner_id(&self) -> Option<[u8; 16]> {
        self.owner_id
    }

    fn bind_container(&mut self, container: &Container) -> Result<(), EchoError> {
        if container.identity().file_kind != FileKind::Reliquary {
            return Err(EchoError::WrongFileKind);
        }
        match self.owner_id {
            Some(owner_id)
                if owner_id != container.owner_id()
                    || self.container_identity != Some(container.identity()) =>
            {
                Err(EchoError::WrongOwner)
            }
            Some(_) => Ok(()),
            None => {
                *self = Self::open(container)?;
                Ok(())
            }
        }
    }

    fn apply(&mut self, event: EchoEvent) -> Result<(), EchoError> {
        if let Some(existing) = self.existing(&event) {
            return if existing == &event {
                Ok(())
            } else {
                Err(EchoError::ConflictingSequence)
            };
        }
        self.records
            .try_reserve(1)
            .map_err(|_| EchoError::IndexCapacityExhausted)?;
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
