//! Perception-owned retained resolution transport interpretation.
pub(crate) fn retained_resolution_carrier(
    payload: &[u8],
) -> Result<Option<Vec<u8>>, crate::MemoryError> {
    if crate::entity_resolution_codec::retained::is_source(payload) {
        return crate::entity_resolution_codec::retained::encode(payload).map(Some);
    }
    if crate::entity_resolution_codec::retained::decode(payload)?.is_some() {
        return Ok(Some(payload.to_vec()));
    }
    Ok(None)
}
pub(crate) fn is_resolution_delta(payload: &[u8]) -> bool {
    crate::entity_resolution_codec::delta::is_current(payload)
}
