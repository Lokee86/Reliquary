//! Owner-lifecycle application of persisted Insomnia replay records.

use crate::{Container, ContainerError, InsomniaError};

#[derive(Debug)]
pub(crate) enum ReplayError {
    Decode(InsomniaError),
    Append(ContainerError),
}

pub(crate) fn decode_completion(
    payload: &[u8],
) -> Result<Option<crate::insomnia::completion::InsomniaCompletion>, &'static str> {
    crate::insomnia::completion::decode_completion(payload)
}

/// Replay one persisted journal record without separating its physical
/// publication from the owner projection update. Preserve the historical
/// append-before-apply order used by Import reconciliation.
pub(crate) fn replay_record(
    container: &mut Container,
    insomnia: &mut crate::insomnia::store::InsomniaStore,
    payload: &[u8],
) -> Result<bool, ReplayError> {
    let Some(record) =
        crate::insomnia::codec::decode_record(payload).map_err(ReplayError::Decode)?
    else {
        return Ok(false);
    };
    crate::owner_lifecycle::import_epoch_transport::append_replay_payload(container, payload)
        .map_err(ReplayError::Append)?;
    match record {
        crate::insomnia::codec::InsomniaRecord::Work(work) => insomnia.apply_work(work),
        crate::insomnia::codec::InsomniaRecord::Attempt(attempt) => insomnia.apply_attempt(attempt),
    }
    Ok(true)
}
