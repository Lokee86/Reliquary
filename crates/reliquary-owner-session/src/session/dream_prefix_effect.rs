use super::*;
use crate::records::{
    dream_empty::InitialDreamSettlement, dream_group::GroupedDreamRelation,
    dream_supersession::GroupedDreamSupersession,
};
use std::io::Read;
/// Read the bounded owner carriers relevant to compact Begin prefix checking.
pub(super) fn prefix_effect(
    reader: &mut impl std::io::Read,
    magic: [u8; 8],
) -> Result<(Option<u64>, bool), OwnerSessionError> {
    let mut payload = magic.to_vec();
    reader
        .take(128 * 1024 * 1024 + 1)
        .read_to_end(&mut payload)
        .map_err(|e| OwnerSessionError::Record(e.to_string()))?;
    if payload.len() > 128 * 1024 * 1024 {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    if let Some(record) = crate::records::dream_batch::DreamBatchRecord::decode(&payload)
        .map_err(OwnerSessionError::Record)?
    {
        return Ok((
            Some(record.graph_version + record.graph_records.len() as u64),
            !record.lifecycles.is_empty(),
        ));
    }
    if let Some(record) =
        InitialDreamSettlement::decode(&payload).map_err(OwnerSessionError::Record)?
    {
        return Ok((None, record.memory_delta.is_some()));
    }
    if let Some(record) =
        GroupedDreamRelation::decode(&payload).map_err(OwnerSessionError::Record)?
    {
        return Ok((Some(record.relation().graph_version), false));
    }
    if let Some(record) =
        GroupedDreamSupersession::decode(&payload).map_err(OwnerSessionError::Record)?
    {
        return Ok((Some(record.relation().graph_version), true));
    }
    Ok((None, false))
}
