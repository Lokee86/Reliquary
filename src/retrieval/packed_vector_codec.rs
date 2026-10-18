use crate::{PackedVectorError, PackedVectorId};
use lodestone_packed::{PackedVectors, ScalarType, VectorSchema};
use sha2::{Digest, Sha256};

const FORMAT_MAGIC: &[u8; 8] = b"CVAPVFM1";
const FORMAT_SCHEMA: u32 = 1;
const OBJECT_MAGIC: &[u8; 8] = b"CVAPVEC1";
const OBJECT_HEADER_LEN: usize = 64;
const PRUNED_MAGIC: &[u8; 8] = b"CVAPVRN1";
const PRUNED_LEN: usize = 128;

#[derive(Clone, Copy)]
pub(crate) struct DecodedPrunedPackedVector {
    pub info: crate::PackedVectorInfo,
    pub manifest_digest: [u8; 32],
}

pub(crate) struct DecodedPackedVector<'a> {
    pub id: PackedVectorId,
    pub schema: VectorSchema,
    pub count: u64,
    pub bytes: &'a [u8],
}

pub(crate) fn encode_format() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(12);
    bytes.extend_from_slice(FORMAT_MAGIC);
    bytes.extend_from_slice(&FORMAT_SCHEMA.to_le_bytes());
    bytes
}

pub(crate) fn decode_format(bytes: &[u8]) -> Result<bool, PackedVectorError> {
    if !bytes.starts_with(FORMAT_MAGIC) {
        return Ok(false);
    }
    if bytes.len() != 12 || read_u32(bytes, 8)? != FORMAT_SCHEMA {
        return Err(PackedVectorError::CorruptRecord(
            "packed-vector format marker",
        ));
    }
    Ok(true)
}

pub(crate) fn encode_pruned_marker(
    info: crate::PackedVectorInfo,
    manifest_digest: [u8; 32],
) -> Result<Vec<u8>, PackedVectorError> {
    let expected = u64::try_from(
        info.schema
            .row_bytes()
            .map_err(|_| PackedVectorError::SizeOverflow)?,
    )
    .map_err(|_| PackedVectorError::SizeOverflow)?
    .checked_mul(info.count)
    .ok_or(PackedVectorError::SizeOverflow)?;
    if expected != info.byte_len || manifest_digest == [0; 32] {
        return Err(PackedVectorError::CorruptRecord("pruned payload marker"));
    }
    let mut bytes = Vec::with_capacity(PRUNED_LEN);
    bytes.extend_from_slice(PRUNED_MAGIC);
    bytes.extend_from_slice(&info.id.0);
    bytes.extend_from_slice(&info.schema.dimensions.to_le_bytes());
    bytes.push(info.schema.scalar.tag());
    bytes.extend_from_slice(&[0; 3]);
    bytes.extend_from_slice(&info.count.to_le_bytes());
    bytes.extend_from_slice(&info.byte_len.to_le_bytes());
    bytes.extend_from_slice(&manifest_digest);
    bytes.extend_from_slice(&Sha256::digest(&bytes));
    Ok(bytes)
}

pub(crate) fn decode_pruned_marker(
    bytes: &[u8],
) -> Result<Option<DecodedPrunedPackedVector>, PackedVectorError> {
    if !bytes.starts_with(PRUNED_MAGIC) {
        return Ok(None);
    }
    if bytes.len() != PRUNED_LEN || bytes[45..48] != [0; 3] {
        return Err(PackedVectorError::CorruptRecord("pruned payload marker"));
    }
    let checksum_at = bytes.len() - 32;
    if Sha256::digest(&bytes[..checksum_at]).as_slice() != &bytes[checksum_at..] {
        return Err(PackedVectorError::CorruptRecord("pruned payload checksum"));
    }
    let id = PackedVectorId(bytes[8..40].try_into().expect("fixed packed-vector id"));
    let dimensions = read_u32(bytes, 40)?;
    let scalar = ScalarType::from_tag(bytes[44])
        .map_err(|_| PackedVectorError::CorruptRecord("pruned packed-vector scalar"))?;
    let schema = VectorSchema::new(dimensions, scalar)
        .map_err(|_| PackedVectorError::CorruptRecord("pruned packed-vector schema"))?;
    let count = read_u64(bytes, 48)?;
    let byte_len = read_u64(bytes, 56)?;
    let expected = u64::try_from(
        schema
            .row_bytes()
            .map_err(|_| PackedVectorError::SizeOverflow)?,
    )
    .map_err(|_| PackedVectorError::SizeOverflow)?
    .checked_mul(count)
    .ok_or(PackedVectorError::SizeOverflow)?;
    let manifest_digest: [u8; 32] = bytes[64..96].try_into().expect("fixed manifest digest");
    if byte_len != expected || manifest_digest == [0; 32] {
        return Err(PackedVectorError::CorruptRecord("pruned payload shape"));
    }
    Ok(Some(DecodedPrunedPackedVector {
        info: crate::PackedVectorInfo {
            id,
            schema,
            count,
            byte_len,
        },
        manifest_digest,
    }))
}

pub(crate) fn encode_object(
    id: PackedVectorId,
    packed: &PackedVectors,
) -> Result<Vec<u8>, PackedVectorError> {
    let count = u64::try_from(packed.count()).map_err(|_| PackedVectorError::SizeOverflow)?;
    let byte_len =
        u64::try_from(packed.bytes().len()).map_err(|_| PackedVectorError::SizeOverflow)?;
    let mut bytes = Vec::with_capacity(
        OBJECT_HEADER_LEN
            .checked_add(packed.bytes().len())
            .ok_or(PackedVectorError::SizeOverflow)?,
    );
    bytes.extend_from_slice(OBJECT_MAGIC);
    bytes.extend_from_slice(&id.0);
    bytes.extend_from_slice(&packed.schema().dimensions.to_le_bytes());
    bytes.push(packed.schema().scalar.tag());
    bytes.extend_from_slice(&[0; 3]);
    bytes.extend_from_slice(&count.to_le_bytes());
    bytes.extend_from_slice(&byte_len.to_le_bytes());
    bytes.extend_from_slice(packed.bytes());
    Ok(bytes)
}

pub(crate) fn decode_object(
    bytes: &[u8],
) -> Result<Option<DecodedPackedVector<'_>>, PackedVectorError> {
    if !bytes.starts_with(OBJECT_MAGIC) {
        return Ok(None);
    }
    if bytes.len() < OBJECT_HEADER_LEN || bytes[45..48] != [0, 0, 0] {
        return Err(PackedVectorError::CorruptRecord(
            "packed-vector object header",
        ));
    }
    let id = PackedVectorId(bytes[8..40].try_into().expect("fixed packed-vector id"));
    let dimensions = read_u32(bytes, 40)?;
    let scalar = ScalarType::from_tag(bytes[44])
        .map_err(|_| PackedVectorError::CorruptRecord("packed-vector scalar type"))?;
    let schema = VectorSchema::new(dimensions, scalar)
        .map_err(|_| PackedVectorError::CorruptRecord("packed-vector schema"))?;
    let count = read_u64(bytes, 48)?;
    let byte_len = read_u64(bytes, 56)?;
    let matrix = &bytes[OBJECT_HEADER_LEN..];
    if u64::try_from(matrix.len()).ok() != Some(byte_len) {
        return Err(PackedVectorError::CorruptRecord(
            "packed-vector byte length",
        ));
    }
    let expected = u64::try_from(
        schema
            .row_bytes()
            .map_err(|_| PackedVectorError::SizeOverflow)?,
    )
    .map_err(|_| PackedVectorError::SizeOverflow)?
    .checked_mul(count)
    .ok_or(PackedVectorError::SizeOverflow)?;
    if expected != byte_len {
        return Err(PackedVectorError::CorruptRecord(
            "packed-vector matrix shape",
        ));
    }
    Ok(Some(DecodedPackedVector {
        id,
        schema,
        count,
        bytes: matrix,
    }))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, PackedVectorError> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or(PackedVectorError::CorruptRecord("packed-vector integer"))?;
    Ok(u32::from_le_bytes(raw.try_into().expect("u32 width")))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, PackedVectorError> {
    let raw = bytes
        .get(offset..offset + 8)
        .ok_or(PackedVectorError::CorruptRecord("packed-vector integer"))?;
    Ok(u64::from_le_bytes(raw.try_into().expect("u64 width")))
}
