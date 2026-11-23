use super::*;
use sha2::{Digest, Sha256};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(super) enum StoredBody {
    Inline(MemoryBody),
    Indexed(BodyLocation),
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(super) enum BodyEncoding {
    Standard,
    Grouped,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct BodyLocation {
    object: ObjectRef,
    payload_offset: u64,
    length: u64,
    digest: [u8; 32],
    body_len: usize,
    encoding: BodyEncoding,
}

impl StoredBody {
    pub(super) fn new(
        body: MemoryBody,
        bytes: &[u8],
        location: Option<(ObjectRef, u64, BodyEncoding)>,
    ) -> Self {
        match location {
            Some((object, payload_offset, encoding)) => Self::Indexed(BodyLocation {
                object,
                payload_offset,
                length: bytes.len() as u64,
                digest: Sha256::digest(bytes).into(),
                body_len: body.title.len().saturating_add(body.content.len()),
                encoding,
            }),
            None => Self::Inline(body),
        }
    }

    pub(super) fn body_len(&self) -> usize {
        match self {
            Self::Inline(body) => body.title.len().saturating_add(body.content.len()),
            Self::Indexed(old) => old.body_len,
        }
    }
}

impl MemoryStore {
    pub(super) fn insert_body(
        &mut self,
        id: MemoryBodyId,
        body: MemoryBody,
        bytes: &[u8],
        location: Option<(ObjectRef, u64, BodyEncoding)>,
    ) -> Result<(), MemoryError> {
        if let Some(existing) = self.load_body(id)? {
            if existing.as_ref() != &body {
                return Err(MemoryError::HashCollision);
            }
        } else {
            self.state
                .bodies
                .insert(id, StoredBody::new(body, bytes, location));
        }
        Ok(())
    }

    pub(super) fn load_body(
        &self,
        id: MemoryBodyId,
    ) -> Result<Option<Cow<'_, MemoryBody>>, MemoryError> {
        let Some(body) = self.state.bodies.get(&id) else {
            return Ok(None);
        };
        match body {
            StoredBody::Inline(body) => Ok(Some(Cow::Borrowed(body))),
            StoredBody::Indexed(location) => {
                let reader = self.reader.as_ref().ok_or(MemoryError::Corrupt(
                    "indexed Memory body has no active owner lease",
                ))?;
                let bytes = reader.read_frame_range(
                    location.object,
                    location.payload_offset,
                    location.length,
                    location.digest,
                )?;
                let body = match location.encoding {
                    BodyEncoding::Standard if bytes.starts_with(BODY) => decode_body(&bytes)
                        .and_then(|(stored_id, body)| {
                            if stored_id == id && memory_body_id(&body.title, &body.content) == id {
                                Ok(body)
                            } else {
                                Err(MemoryError::Corrupt("indexed Memory body identity differs"))
                            }
                        }),
                    BodyEncoding::Standard => Err(MemoryError::Corrupt(
                        "indexed Memory body is outside its record family",
                    )),
                    BodyEncoding::Grouped => decode_grouped_body(id, &bytes),
                };
                let body = body.inspect_err(|_| reader.fence_writes())?;
                if location.body_len != body.title.len().saturating_add(body.content.len()) {
                    reader.fence_writes();
                    return Err(MemoryError::Corrupt("indexed Memory body extent differs"));
                }
                Ok(Some(Cow::Owned(body)))
            }
        }
    }

    pub(super) fn body_len(&self, id: MemoryBodyId) -> Option<usize> {
        self.state.bodies.get(&id).map(StoredBody::body_len)
    }
}
