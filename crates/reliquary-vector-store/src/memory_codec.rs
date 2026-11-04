use crate::{
    CompatibilityProfileId, MemoryBodyId, MemoryVectorId, PackedVectorError, PackedVectorId,
};
const FORMAT: &[u8; 8] = b"RELMVFM1";
const OBJECT: &[u8; 8] = b"RELMVEC1";
const HEADER: usize = 112;
pub(crate) struct Decoded {
    pub id: MemoryVectorId,
    pub profile: CompatibilityProfileId,
    pub packed: PackedVectorId,
    pub bodies: Vec<MemoryBodyId>,
}
pub fn encode_format() -> Vec<u8> {
    [FORMAT.as_slice(), &1u32.to_le_bytes()].concat()
}
pub(crate) fn decode_format(b: &[u8]) -> Result<bool, PackedVectorError> {
    if !b.starts_with(FORMAT) {
        return Ok(false);
    }
    if b.len() != 12 || u32::from_le_bytes(b[8..12].try_into().unwrap()) != 1 {
        return Err(PackedVectorError::CorruptRecord(
            "Memory-Vector format marker",
        ));
    }
    Ok(true)
}
pub(crate) fn encode_object(
    id: MemoryVectorId,
    profile: CompatibilityProfileId,
    packed: PackedVectorId,
    bodies: &[MemoryBodyId],
) -> Result<Vec<u8>, PackedVectorError> {
    let mut b = Vec::with_capacity(
        HEADER
            + bodies
                .len()
                .checked_mul(32)
                .ok_or(PackedVectorError::SizeOverflow)?,
    );
    b.extend_from_slice(OBJECT);
    b.extend_from_slice(&id.0);
    b.extend_from_slice(&profile.0);
    b.extend_from_slice(&packed.0);
    b.extend_from_slice(
        &u64::try_from(bodies.len())
            .map_err(|_| PackedVectorError::SizeOverflow)?
            .to_le_bytes(),
    );
    for x in bodies {
        b.extend_from_slice(&x.0)
    }
    Ok(b)
}
pub(crate) fn decode_object(b: &[u8]) -> Result<Option<Decoded>, PackedVectorError> {
    if !b.starts_with(OBJECT) {
        return Ok(None);
    }
    if b.len() < HEADER {
        return Err(PackedVectorError::CorruptRecord(
            "Memory-Vector object header",
        ));
    }
    let n = usize::try_from(u64::from_le_bytes(b[104..112].try_into().unwrap()))
        .map_err(|_| PackedVectorError::SizeOverflow)?;
    if HEADER
        .checked_add(n.checked_mul(32).ok_or(PackedVectorError::SizeOverflow)?)
        .ok_or(PackedVectorError::SizeOverflow)?
        != b.len()
    {
        return Err(PackedVectorError::CorruptRecord(
            "Memory-Vector row mapping length",
        ));
    }
    let mut bodies = Vec::with_capacity(n);
    for x in b[HEADER..].chunks_exact(32) {
        bodies.push(MemoryBodyId(x.try_into().unwrap()))
    }
    Ok(Some(Decoded {
        id: MemoryVectorId(b[8..40].try_into().unwrap()),
        profile: CompatibilityProfileId(b[40..72].try_into().unwrap()),
        packed: PackedVectorId(b[72..104].try_into().unwrap()),
        bodies,
    }))
}
