//! Pointer-free, disposable lexical projections of accepted Archive generations.
use super::*;
use reliquary_archive::FragmentLexicalIndex;
use sha2::{Digest, Sha256};
use std::{io::Read, sync::Arc};

pub(crate) const ARCHIVE_LEXICAL_INDEX_MAGIC: &[u8; 8] = b"RELALX01";
const KEY_BYTES: usize = 8 + 16 + 32;
const HEADER_BYTES: usize = KEY_BYTES + 32 + 8 + 4;
const SEAL_BYTES: usize = 32;
const UNICODE: (u8, u8, u8) = std::char::UNICODE_VERSION;
const ANALYZER: [u8; 4] = [UNICODE.0, UNICODE.1, UNICODE.2, 0];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveLexicalIndexPublication {
    pub profile_id: CompatibilityProfileId,
    pub generation_id: [u8; 32],
    pub added_bytes: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct StoredArchiveLexicalIndex {
    pub(super) generation: VectorGenerationId,
    pub(super) source_digest: [u8; 32],
    pub(super) fragments_with_text: usize,
    pub(super) index: Arc<FragmentLexicalIndex>,
    pub(super) seal: [u8; SEAL_BYTES],
}

impl StoredArchiveLexicalIndex {
    pub(super) fn encode(
        owner: [u8; 16],
        generation: VectorGenerationId,
        source_digest: [u8; 32],
        fragments_with_text: usize,
        index: Arc<FragmentLexicalIndex>,
    ) -> (Self, Vec<u8>) {
        let mut payload = Vec::new();
        payload.extend_from_slice(ARCHIVE_LEXICAL_INDEX_MAGIC);
        payload.extend_from_slice(&owner);
        payload.extend_from_slice(&generation.0);
        payload.extend_from_slice(&source_digest);
        payload.extend_from_slice(&(fragments_with_text as u64).to_le_bytes());
        payload.extend_from_slice(&ANALYZER);
        index.encode_into(&mut payload);
        let seal: [u8; SEAL_BYTES] = Sha256::digest(&payload).into();
        payload.extend_from_slice(&seal);
        (
            Self {
                generation,
                source_digest,
                fragments_with_text,
                index,
                seal,
            },
            payload,
        )
    }

    fn decode(generation: VectorGenerationId, rows: usize, payload: &[u8]) -> Option<Self> {
        let seal = archive_lexical_index_seal(payload)?;
        let record = &payload[..payload.len() - SEAL_BYTES];
        let (header, body) = record.split_at_checked(HEADER_BYTES)?;
        if header[HEADER_BYTES - ANALYZER.len()..] != ANALYZER {
            return None;
        }
        let source_digest = header[KEY_BYTES..KEY_BYTES + 32].try_into().ok()?;
        let fragments_with_text = usize::try_from(u64::from_le_bytes(
            header[KEY_BYTES + 32..KEY_BYTES + 40].try_into().ok()?,
        ))
        .ok()?;
        if fragments_with_text > rows {
            return None;
        }
        let index = Arc::new(FragmentLexicalIndex::decode(body, rows).ok()?);
        Some(Self {
            generation,
            source_digest,
            fragments_with_text,
            index,
            seal,
        })
    }

    pub(super) fn matches(
        &self,
        generation: VectorGenerationId,
        source_digest: [u8; 32],
        fragments_with_text: usize,
    ) -> bool {
        self.generation == generation
            && self.source_digest == source_digest
            && self.fragments_with_text == fragments_with_text
    }
}

pub(super) fn open_archive_lexical_indexes(
    container: &Container,
    profiles: &CompatibilityProfileStore,
    generations: &VectorGenerationStore,
    mappings: &ArchiveVectorStore,
) -> Result<BTreeMap<CompatibilityProfileId, StoredArchiveLexicalIndex>, OwnerSessionError> {
    let mut active = BTreeMap::new();
    for generation in generations.current_generations() {
        let profile = generation.compatibility_profile_id;
        if profiles.get_active(profile).is_some() {
            let (_, fragments) = mappings.row_mapping(generation.archive_vector_id)?;
            active.insert(generation.id.0, (profile, fragments.len()));
        }
    }
    let mut indexes = BTreeMap::new();
    if active.is_empty() {
        return Ok(indexes);
    }
    let owner = container.owner_id();
    let mut cursor = container.physical_cursor()?;
    let mut payload = Vec::new();
    while let Some(mut frame) = cursor.next_frame()? {
        if frame.object().length < (HEADER_BYTES + SEAL_BYTES) as u64 {
            continue;
        }
        let mut marker = [0; 8];
        frame.read_exact(&mut marker).map_err(ContainerError::Io)?;
        if &marker != ARCHIVE_LEXICAL_INDEX_MAGIC {
            continue;
        }
        let mut key = [0; KEY_BYTES - 8];
        frame.read_exact(&mut key).map_err(ContainerError::Io)?;
        if key[..16] != owner {
            continue;
        }
        let generation = VectorGenerationId(key[16..].try_into().expect("generation ID width"));
        let Some(&(profile, rows)) = active.get(&generation.0) else {
            continue;
        };
        payload.clear();
        payload.extend_from_slice(&marker);
        payload.extend_from_slice(&key);
        frame
            .read_to_end(&mut payload)
            .map_err(ContainerError::Io)?;
        // An absent or damaged cache never shadows the accepted source.
        if let Some(index) = StoredArchiveLexicalIndex::decode(generation, rows, &payload) {
            indexes.insert(profile, index);
        }
    }
    Ok(indexes)
}

pub(crate) fn archive_lexical_index_seal(payload: &[u8]) -> Option<[u8; SEAL_BYTES]> {
    if !payload.starts_with(ARCHIVE_LEXICAL_INDEX_MAGIC)
        || payload.len() < HEADER_BYTES + SEAL_BYTES
    {
        return None;
    }
    let (record, footer) = payload.split_at(payload.len() - SEAL_BYTES);
    let seal: [u8; SEAL_BYTES] = footer.try_into().ok()?;
    (<[u8; SEAL_BYTES]>::from(Sha256::digest(record)) == seal).then_some(seal)
}
