use crate::ArchiveVectorError;
use crate::{ArchiveFragmentId, ArchiveVectorId, PackedVectorId};

const FORMAT_MAGIC: &[u8; 8] = b"RELAVFM1";
const FORMAT_SCHEMA: u32 = 1;
const OBJECT_MAGIC: &[u8; 8] = b"RELAVEC1";
const HEADER: usize = 88;

#[derive(Clone, Debug)]
pub struct DecodedArchiveVectors {
    pub id: ArchiveVectorId,
    pub packed_vector_id: PackedVectorId,
    pub fragment_ids: Vec<ArchiveFragmentId>,
    pub max_fragment_archive_version: u64,
}

pub fn encode_archive_format() -> Vec<u8> {
    let mut b = Vec::with_capacity(12);
    b.extend_from_slice(FORMAT_MAGIC);
    b.extend_from_slice(&FORMAT_SCHEMA.to_le_bytes());
    b
}
pub fn decode_archive_format(b: &[u8]) -> Result<bool, ArchiveVectorError> {
    if !b.starts_with(FORMAT_MAGIC) {
        return Ok(false);
    }
    if b.len() != 12 || read_u32(b, 8)? != FORMAT_SCHEMA {
        return Err(ArchiveVectorError::CorruptRecord(
            "Archive-Vector format marker",
        ));
    }
    Ok(true)
}
pub fn encode_archive_object(
    id: ArchiveVectorId,
    packed: PackedVectorId,
    fragments: &[ArchiveFragmentId],
    max_fragment_archive_version: u64,
) -> Result<Vec<u8>, ArchiveVectorError> {
    let rows = u64::try_from(fragments.len()).map_err(|_| ArchiveVectorError::SizeOverflow)?;
    let n = fragments
        .len()
        .checked_mul(32)
        .ok_or(ArchiveVectorError::SizeOverflow)?;
    let mut b = Vec::with_capacity(
        HEADER
            .checked_add(n)
            .ok_or(ArchiveVectorError::SizeOverflow)?,
    );
    b.extend_from_slice(OBJECT_MAGIC);
    b.extend_from_slice(&id.0);
    b.extend_from_slice(&packed.0);
    b.extend_from_slice(&rows.to_le_bytes());
    b.extend_from_slice(&max_fragment_archive_version.to_le_bytes());
    for x in fragments {
        b.extend_from_slice(&x.0);
    }
    Ok(b)
}
pub fn decode_archive_object(
    b: &[u8],
) -> Result<Option<DecodedArchiveVectors>, ArchiveVectorError> {
    if !b.starts_with(OBJECT_MAGIC) {
        return Ok(None);
    }
    if b.len() < HEADER {
        return Err(ArchiveVectorError::CorruptRecord(
            "Archive-Vector object header",
        ));
    }
    let id = ArchiveVectorId(b[8..40].try_into().expect("fixed Archive-Vector id"));
    let packed_vector_id = PackedVectorId(b[40..72].try_into().expect("fixed packed-vector id"));
    let rows = usize::try_from(read_u64(b, 72)?).map_err(|_| ArchiveVectorError::SizeOverflow)?;
    let max_fragment_archive_version = read_u64(b, 80)?;
    let expected = HEADER
        .checked_add(
            rows.checked_mul(32)
                .ok_or(ArchiveVectorError::SizeOverflow)?,
        )
        .ok_or(ArchiveVectorError::SizeOverflow)?;
    if b.len() != expected {
        return Err(ArchiveVectorError::CorruptRecord(
            "Archive-Vector row mapping length",
        ));
    }
    let fragment_ids = b[HEADER..]
        .chunks_exact(32)
        .map(|x| ArchiveFragmentId(x.try_into().expect("fixed fragment id")))
        .collect();
    Ok(Some(DecodedArchiveVectors {
        id,
        packed_vector_id,
        fragment_ids,
        max_fragment_archive_version,
    }))
}
fn read_u32(b: &[u8], o: usize) -> Result<u32, ArchiveVectorError> {
    let x = b
        .get(o..o + 4)
        .ok_or(ArchiveVectorError::CorruptRecord("Archive-Vector integer"))?;
    Ok(u32::from_le_bytes(x.try_into().expect("u32 width")))
}
fn read_u64(b: &[u8], o: usize) -> Result<u64, ArchiveVectorError> {
    let x = b
        .get(o..o + 8)
        .ok_or(ArchiveVectorError::CorruptRecord("Archive-Vector integer"))?;
    Ok(u64::from_le_bytes(x.try_into().expect("u64 width")))
}
