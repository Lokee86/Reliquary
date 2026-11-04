use super::*;

pub(super) fn clear_lease(w: &mut InsomniaWork) {
    w.lease_owner = None;
    w.lease_token = None;
    w.lease_expires_ns = None
}
pub(super) fn validate_order(o: &AcceptedEpisodeOrder) -> Result<(), InsomniaError> {
    if o.conversation_id.trim().is_empty() || o.start_node_id.trim().is_empty() {
        Err(InsomniaError::InvalidField("accepted Episode order"))
    } else {
        Ok(())
    }
}
pub(super) fn validate_work(w: &InsomniaWork) -> Result<(), InsomniaError> {
    let lease_fields = [
        w.lease_owner.is_some(),
        w.lease_token.is_some(),
        w.lease_expires_ns.is_some(),
    ];
    let leased = lease_fields.iter().all(|present| *present);
    let partial_lease = lease_fields.iter().any(|present| *present) && !leased;
    if partial_lease
        || (w.state == InsomniaWorkState::Processing) != leased
        || w.revision == 0
        || w.held_payload
            .as_ref()
            .is_some_and(|payload| payload.len() > MAX_HELD_PAYLOAD_BYTES)
        || (w.held_payload.is_some()
            && w.state != InsomniaWorkState::Held
            && w.state != InsomniaWorkState::Pending
            && w.state != InsomniaWorkState::Processing)
    {
        return Err(InsomniaError::CorruptRecord("work lease invariants"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending_work() -> InsomniaWork {
        InsomniaWork {
            episode_id: EpisodeId([1; 32]),
            priority: InsomniaPriority::Live,
            state: InsomniaWorkState::Pending,
            attempt_count: 0,
            lease_owner: None,
            lease_token: None,
            lease_expires_ns: None,
            retry_after_ns: None,
            last_error: None,
            held_payload: None,
            updated_at_ns: 0,
            revision: 1,
        }
    }

    #[test]
    fn rejects_partial_lease_fields_during_replay_validation() {
        let mut work = pending_work();
        work.lease_owner = Some("worker".into());
        assert!(matches!(
            validate_work(&work),
            Err(InsomniaError::CorruptRecord("work lease invariants"))
        ));

        work.lease_token = Some(InsomniaLeaseToken([2; 32]));
        assert!(matches!(
            validate_work(&work),
            Err(InsomniaError::CorruptRecord("work lease invariants"))
        ));
    }
}

pub(super) fn validate_receipt(r: &AcceptedCompletionState) -> Result<(), InsomniaError> {
    if r.attempt == 0
        || r.completed_at_ns < r.started_at_ns
        || r.extractor_model.trim().is_empty()
        || r.extractor_version.trim().is_empty()
        || r.memory_ids.len() > MAX_RECEIPT_IDS
        || r.external_memory_refs.len() > MAX_RECEIPT_IDS
        || r.extractor_model.len() > MAX_RECEIPT_TEXT_BYTES
        || r.extractor_version.len() > MAX_RECEIPT_TEXT_BYTES
    {
        return Err(InsomniaError::InvalidField("completion receipt"));
    }
    for r in &r.external_memory_refs {
        if r.owner_id.trim().is_empty() || r.owner_id.len() > MAX_RECEIPT_TEXT_BYTES {
            return Err(InsomniaError::InvalidField("external memory reference"));
        }
    }
    Ok(())
}
pub(super) fn token(id: EpisodeId, worker: &str, attempt: u32, now: i64) -> InsomniaLeaseToken {
    let mut h = Sha256::new();
    h.update(b"continuity-insomnia-lease\0");
    h.update(id.0);
    h.update((worker.len() as u64).to_le_bytes());
    h.update(worker.as_bytes());
    h.update(attempt.to_le_bytes());
    h.update(now.to_le_bytes());
    InsomniaLeaseToken(h.finalize().into())
}
