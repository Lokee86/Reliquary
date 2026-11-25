use crate::echo_codec::{decode, encode};
use crate::{EchoError, EchoEvent};
use reliquary_container::{Container, ContainerIdentity, FileKind, LeasedFrameReader, ObjectRef};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
enum EchoPayload {
    Resident(Box<EchoEvent>),
    Leased { frame: ObjectRef, digest: [u8; 32] },
}

type EchoSequences = BTreeMap<u64, EchoPayload>;

/// Separately owned source-keyed reasoning/tool evidence. It shares only the
/// physical REL Container with Archive and does not alter Archive activity.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct EchoStore {
    owner_id: Option<[u8; 16]>,
    container_identity: Option<ContainerIdentity>,
    records: BTreeMap<String, BTreeMap<String, EchoSequences>>,
    #[serde(skip)]
    reader: Option<LeasedFrameReader>,
}

impl EchoStore {
    pub fn open(container: &Container) -> Result<Self, EchoError> {
        Self::replay(container, false)
    }

    /// Validate Echo records and retain frame positions for on-demand reads.
    pub fn open_leased(container: &Container) -> Result<Self, EchoError> {
        Self::replay(container, true)
    }

    fn replay(container: &Container, leased: bool) -> Result<Self, EchoError> {
        if container.identity().file_kind != FileKind::Reliquary {
            return Err(EchoError::WrongFileKind);
        }
        let mut store = Self {
            owner_id: Some(container.owner_id()),
            container_identity: Some(container.identity()),
            ..Self::default()
        };
        if leased {
            store.bind_read_lease(container)?;
        }
        let mut frames =
            container.scan_matching_frames(|prefix| prefix.starts_with(b"RELECHO1"))?;
        while let Some(frame) = frames.next_frame()? {
            if let Some(event) = decode(frame.payload)?
                && !store.is_repeat(&event)?
            {
                let backing = leased.then(|| (frame.object, Sha256::digest(frame.payload).into()));
                store.insert(event, backing);
            }
        }
        Ok(store)
    }

    pub fn bind_read_lease(&mut self, container: &Container) -> Result<(), EchoError> {
        self.bind_container(container)?;
        self.reader = Some(container.leased_frame_reader()?);
        Ok(())
    }

    /// Replay one raw frame. Frames owned by Archive or other domains pass through.
    pub fn ingest(&mut self, payload: &[u8]) -> Result<(), EchoError> {
        if let Some(event) = decode(payload)?
            && !self.is_repeat(&event)?
        {
            self.insert(event, None);
        }
        Ok(())
    }

    /// Persist evidence under the same Container owner, then accept it in memory.
    pub fn put(&mut self, container: &mut Container, event: EchoEvent) -> Result<bool, EchoError> {
        self.bind_container(container)?;
        if self.is_repeat(&event)? {
            return Ok(false);
        }
        let payload = encode(&event)?;
        let frame = container.append(&payload)?;
        container.sync()?;
        let backing = self
            .reader
            .as_ref()
            .map(|_| (frame, Sha256::digest(&payload).into()));
        self.insert(event, backing);
        Ok(true)
    }

    pub fn for_turn(&self, conversation: &str, message: &str) -> Result<Vec<EchoEvent>, EchoError> {
        self.records
            .get(conversation)
            .and_then(|messages| messages.get(message))
            .into_iter()
            .flatten()
            .map(|(&sequence, payload)| self.read(conversation, message, sequence, payload))
            .collect()
    }

    pub fn for_conversation(&self, conversation: &str) -> Result<Vec<EchoEvent>, EchoError> {
        let mut events = Vec::new();
        if let Some(messages) = self.records.get(conversation) {
            for (message, sequences) in messages {
                for (&sequence, payload) in sequences {
                    events.push(self.read(conversation, message, sequence, payload)?);
                }
            }
        }
        Ok(events)
    }

    pub fn all_records(&self) -> Result<Vec<EchoEvent>, EchoError> {
        let mut events = Vec::new();
        for conversation in self.records.keys() {
            events.extend(self.for_conversation(conversation)?);
        }
        Ok(events)
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

    fn read(
        &self,
        conversation: &str,
        message: &str,
        sequence: u64,
        payload: &EchoPayload,
    ) -> Result<EchoEvent, EchoError> {
        match payload {
            EchoPayload::Resident(event) => Ok(event.as_ref().clone()),
            EchoPayload::Leased { frame, digest } => {
                let reader = self
                    .reader
                    .as_ref()
                    .ok_or(EchoError::InvalidRecord("missing Echo read lease"))?;
                let bytes = reader.read_frame(*frame, *digest)?;
                let event = decode(&bytes).and_then(|event| {
                    event
                        .filter(|event| {
                            event.conversation_id == conversation
                                && event.message_id == message
                                && event.sequence == sequence
                        })
                        .ok_or(EchoError::InvalidRecord(
                            "Echo frame does not match its source",
                        ))
                });
                if event.is_err() {
                    reader.fence_writes();
                }
                event
            }
        }
    }

    fn is_repeat(&self, event: &EchoEvent) -> Result<bool, EchoError> {
        let Some(payload) = self
            .records
            .get(&event.conversation_id)
            .and_then(|messages| messages.get(&event.message_id))
            .and_then(|sequences| sequences.get(&event.sequence))
        else {
            return Ok(false);
        };
        if self.read(
            &event.conversation_id,
            &event.message_id,
            event.sequence,
            payload,
        )? != *event
        {
            return Err(EchoError::ConflictingSequence);
        }
        Ok(true)
    }

    fn insert(&mut self, event: EchoEvent, backing: Option<(ObjectRef, [u8; 32])>) {
        let sequences = self
            .records
            .entry(event.conversation_id.clone())
            .or_default()
            .entry(event.message_id.clone())
            .or_default();
        let sequence = event.sequence;
        let payload = match backing {
            Some((frame, digest)) => EchoPayload::Leased { frame, digest },
            None => EchoPayload::Resident(Box::new(event)),
        };
        sequences.insert(sequence, payload);
    }
}
