//! Self-contained, disposable current index snapshots. Never Memory authority.
use super::model::*;
use super::store::{IndexedChunk, IndexedMessage, SourceState};
use sha2::{Digest, Sha256};

pub const MAGIC: &[u8; 8] = b"RELXIDX2";
pub enum Record {
    Snapshot(SourceState),
    Discard,
}

pub fn encode(state: &SourceState) -> Result<Vec<u8>, ExternalSourceError> {
    state.validate()?;
    let mut body = Vec::new();
    string(&mut body, &state.namespace)?;
    body.extend_from_slice(&state.contract.to_le_bytes());
    body.extend_from_slice(&state.profile);
    body.extend_from_slice(&state.dimensions.to_le_bytes());
    for value in [
        state.generation,
        state.revision,
        state.cursor,
        state.last_base,
    ] {
        body.extend_from_slice(&value.to_le_bytes());
    }
    body.extend_from_slice(&state.last_digest);
    body.push(u8::from(state.rebuild));
    count(&mut body, state.messages.len())?;
    for message in &state.messages {
        let descriptor = &message.descriptor;
        string(&mut body, &descriptor.reference.conversation_id)?;
        string(&mut body, &descriptor.reference.message_id)?;
        body.extend_from_slice(&descriptor.reference.content_hash);
        body.push(match descriptor.role.as_str() {
            "user" => 0,
            "assistant" => 1,
            "system" => 2,
            "tool" => 3,
            _ => return Err(ExternalSourceError::InvalidContract),
        });
        match &descriptor.principal_id {
            None => body.push(0),
            Some(principal) => {
                body.push(1);
                string(&mut body, principal)?;
            }
        }
        body.extend_from_slice(&descriptor.timestamp_ns.to_le_bytes());
        body.push(u8::from(descriptor.active));
        count(&mut body, message.chunks.len())?;
        for chunk in &message.chunks {
            let end =
                u32::try_from(chunk.reference.end).map_err(|_| ExternalSourceError::Bounds)?;
            body.extend_from_slice(&end.to_le_bytes());
            body.extend_from_slice(&chunk.matrix);
            body.extend_from_slice(&chunk.row.to_le_bytes());
            count(&mut body, chunk.terms.len())?;
            for term in &chunk.terms {
                body.extend_from_slice(term);
            }
            if body.len() > MAX_EXTERNAL_INDEX_BYTES {
                return Err(ExternalSourceError::Bounds);
            }
        }
        if body.len() > MAX_EXTERNAL_INDEX_BYTES {
            return Err(ExternalSourceError::Bounds);
        }
    }
    envelope(0, &body)
}

pub fn discard() -> Vec<u8> {
    envelope(1, &[]).expect("empty derived discard is bounded")
}

fn envelope(kind: u8, body: &[u8]) -> Result<Vec<u8>, ExternalSourceError> {
    if body.len() > MAX_EXTERNAL_INDEX_BYTES {
        return Err(ExternalSourceError::Bounds);
    }
    let mut bytes = MAGIC.to_vec();
    bytes.push(kind);
    bytes.extend_from_slice(&(body.len() as u64).to_le_bytes());
    bytes.extend_from_slice(body);
    let digest = Sha256::digest(&bytes);
    bytes.extend_from_slice(&digest);
    Ok(bytes)
}

pub fn decode(bytes: &[u8]) -> Result<Record, ExternalSourceError> {
    if bytes.len() < 49 || bytes.len() > MAX_EXTERNAL_INDEX_BYTES + 49 || !bytes.starts_with(MAGIC)
    {
        return Err(ExternalSourceError::Bounds);
    }
    let length = u64::from_le_bytes(bytes[9..17].try_into().unwrap());
    if usize::try_from(length).ok().and_then(|n| n.checked_add(49)) != Some(bytes.len()) {
        return Err(ExternalSourceError::Bounds);
    }
    let expected: [u8; 32] = Sha256::digest(&bytes[..bytes.len() - 32]).into();
    if bytes[bytes.len() - 32..] != expected {
        return Err(ExternalSourceError::Storage);
    }
    let mut reader = Reader {
        bytes: &bytes[17..bytes.len() - 32],
        cursor: 0,
    };
    if bytes[8] == 1 {
        return if reader.bytes.is_empty() {
            Ok(Record::Discard)
        } else {
            Err(ExternalSourceError::InvalidContract)
        };
    }
    if bytes[8] != 0 {
        return Err(ExternalSourceError::InvalidContract);
    }
    let namespace = reader.string()?;
    let contract = reader.u32()?;
    let profile = reader.hash()?;
    let dimensions = reader.u32()?;
    let generation = reader.u64()?;
    let revision = reader.u64()?;
    let cursor = reader.u64()?;
    let last_base = reader.u64()?;
    let last_digest = reader.hash()?;
    let rebuild = reader.boolean()?;
    let message_count = reader.count(MAX_EXTERNAL_MANIFEST, 55)?;
    let mut messages = Vec::with_capacity(message_count);
    for _ in 0..message_count {
        let conversation_id = reader.string()?;
        let message_id = reader.string()?;
        let content_hash = reader.hash()?;
        let role = match reader.byte()? {
            0 => "user",
            1 => "assistant",
            2 => "system",
            3 => "tool",
            _ => return Err(ExternalSourceError::InvalidContract),
        }
        .to_owned();
        let principal_id = if reader.boolean()? {
            Some(reader.string()?)
        } else {
            None
        };
        let timestamp_ns = i64::from_le_bytes(reader.take(8)?.try_into().unwrap());
        let active = reader.boolean()?;
        let reference = ExternalMessageRef {
            source_namespace: namespace.clone(),
            conversation_id,
            message_id,
            content_hash,
        };
        let descriptor = ExternalMessageDescriptor {
            reference,
            role,
            principal_id,
            timestamp_ns,
            active,
        };
        let chunk_count = reader.count(MAX_EXTERNAL_MESSAGE_BYTES, 44)?;
        let mut chunks = Vec::with_capacity(chunk_count);
        let mut start = 0;
        for _ in 0..chunk_count {
            let end = u64::from(reader.u32()?);
            let matrix = reader.hash()?;
            let row = reader.u32()?;
            let term_count = reader.count(1024, 32)?;
            let mut terms = Vec::with_capacity(term_count);
            for _ in 0..term_count {
                terms.push(reader.hash()?);
            }
            chunks.push(IndexedChunk {
                reference: ExternalChunkRef {
                    message: descriptor.reference.clone(),
                    start,
                    end,
                },
                matrix,
                row,
                terms,
            });
            start = end;
        }
        messages.push(IndexedMessage { descriptor, chunks });
    }
    if reader.cursor != reader.bytes.len() {
        return Err(ExternalSourceError::InvalidContract);
    }
    let state = SourceState {
        namespace,
        contract,
        profile,
        dimensions,
        generation,
        revision,
        cursor,
        messages,
        last_base,
        last_digest,
        rebuild,
    };
    state.validate()?;
    Ok(Record::Snapshot(state))
}

fn count(out: &mut Vec<u8>, length: usize) -> Result<(), ExternalSourceError> {
    out.extend_from_slice(
        &u32::try_from(length)
            .map_err(|_| ExternalSourceError::Bounds)?
            .to_le_bytes(),
    );
    Ok(())
}
fn string(out: &mut Vec<u8>, value: &str) -> Result<(), ExternalSourceError> {
    count(out, value.len())?;
    out.extend_from_slice(value.as_bytes());
    Ok(())
}
struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], ExternalSourceError> {
        let end = self
            .cursor
            .checked_add(n)
            .ok_or(ExternalSourceError::Bounds)?;
        let bytes = self
            .bytes
            .get(self.cursor..end)
            .ok_or(ExternalSourceError::Bounds)?;
        self.cursor = end;
        Ok(bytes)
    }
    fn byte(&mut self) -> Result<u8, ExternalSourceError> {
        Ok(self.take(1)?[0])
    }
    fn boolean(&mut self) -> Result<bool, ExternalSourceError> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(ExternalSourceError::InvalidContract),
        }
    }
    fn u32(&mut self) -> Result<u32, ExternalSourceError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, ExternalSourceError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn hash(&mut self) -> Result<[u8; 32], ExternalSourceError> {
        Ok(self.take(32)?.try_into().unwrap())
    }
    fn string(&mut self) -> Result<String, ExternalSourceError> {
        let len = self.u32()? as usize;
        String::from_utf8(self.take(len)?.to_vec())
            .map_err(|_| ExternalSourceError::InvalidContract)
    }
    fn count(&mut self, max: usize, minimum_bytes: usize) -> Result<usize, ExternalSourceError> {
        let n = self.u32()? as usize;
        if n > max || n > (self.bytes.len() - self.cursor) / minimum_bytes {
            return Err(ExternalSourceError::Bounds);
        }
        Ok(n)
    }
}
