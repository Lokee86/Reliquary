use super::*;

pub(super) fn encode_work(
    owner: [u8; 16],
    o: &AcceptedEpisodeOrder,
    w: &InsomniaWork,
) -> Result<Vec<u8>, InsomniaError> {
    validate_work(w)?;
    let mut b = WORK_MAGIC_V2.to_vec();
    b.extend_from_slice(&owner);
    b.extend_from_slice(&w.revision.to_le_bytes());
    b.extend_from_slice(&w.episode_id.0);
    b.extend_from_slice(&o.source_through_ns.to_le_bytes());
    put_str(&mut b, &o.conversation_id)?;
    put_str(&mut b, &o.start_node_id)?;
    b.push(w.priority as u8 + 1);
    b.push(w.state as u8 + 1);
    b.extend_from_slice(&w.attempt_count.to_le_bytes());
    b.extend_from_slice(&w.updated_at_ns.to_le_bytes());
    put_opt_str(&mut b, w.lease_owner.as_deref())?;
    put_opt_bytes(&mut b, w.lease_token.map(|t| t.0).as_ref());
    put_opt_i64(&mut b, w.lease_expires_ns);
    put_opt_i64(&mut b, w.retry_after_ns);
    put_opt_str(&mut b, w.last_error.as_deref())?;
    put_opt_payload(&mut b, w.held_payload.as_deref())?;
    Ok(b)
}
pub(super) fn decode_work(
    b: &[u8],
) -> Result<([u8; 16], AcceptedEpisodeOrder, InsomniaWork), InsomniaError> {
    let v2 = b.starts_with(WORK_MAGIC_V2);
    if !v2 && !b.starts_with(WORK_MAGIC) {
        return Err(InsomniaError::CorruptRecord("work magic"));
    }
    let mut p = 8;
    let owner = take::<16>(b, &mut p)?;
    let revision = u64::from_le_bytes(take::<8>(b, &mut p)?);
    let episode_id = EpisodeId(take::<32>(b, &mut p)?);
    let source_through_ns = i64::from_le_bytes(take::<8>(b, &mut p)?);
    let conversation_id = get_str(b, &mut p)?;
    let start_node_id = get_str(b, &mut p)?;
    let priority = match byte(b, &mut p)? {
        1 => InsomniaPriority::ImmediateLive,
        2 => InsomniaPriority::Live,
        3 => InsomniaPriority::Import,
        _ => return Err(InsomniaError::CorruptRecord("priority")),
    };
    let state = match byte(b, &mut p)? {
        1 => InsomniaWorkState::Pending,
        2 => InsomniaWorkState::Processing,
        3 => InsomniaWorkState::Complete,
        4 => InsomniaWorkState::Failed,
        5 => InsomniaWorkState::Terminal,
        6 => InsomniaWorkState::Held,
        _ => return Err(InsomniaError::CorruptRecord("state")),
    };
    let attempt_count = u32::from_le_bytes(take::<4>(b, &mut p)?);
    let updated_at_ns = i64::from_le_bytes(take::<8>(b, &mut p)?);
    let lease_owner = get_opt_str(b, &mut p)?;
    let lease_token = get_opt_array32(b, &mut p)?.map(InsomniaLeaseToken);
    let lease_expires_ns = get_opt_i64(b, &mut p)?;
    let retry_after_ns = get_opt_i64(b, &mut p)?;
    let last_error = get_opt_str(b, &mut p)?;
    let held_payload = if v2 {
        get_opt_payload(b, &mut p)?
    } else {
        None
    };
    end(b, p)?;
    let order = AcceptedEpisodeOrder {
        episode_id,
        source_through_ns,
        conversation_id,
        start_node_id,
    };
    let work = InsomniaWork {
        episode_id,
        priority,
        state,
        attempt_count,
        lease_owner,
        lease_token,
        lease_expires_ns,
        retry_after_ns,
        last_error,
        held_payload,
        updated_at_ns,
        revision,
    };
    Ok((owner, order, work))
}
pub(super) fn encode_receipt(
    owner: [u8; 16],
    r: &AcceptedCompletionState,
) -> Result<Vec<u8>, InsomniaError> {
    validate_receipt(r)?;
    let mut b = RECEIPT_MAGIC.to_vec();
    b.extend_from_slice(&owner);
    b.extend_from_slice(&r.episode_id.0);
    b.extend_from_slice(&r.attempt.to_le_bytes());
    b.extend_from_slice(&r.started_at_ns.to_le_bytes());
    b.extend_from_slice(&r.completed_at_ns.to_le_bytes());
    put_str(&mut b, &r.extractor_model)?;
    put_str(&mut b, &r.extractor_version)?;
    b.extend_from_slice(&r.rejected_count.to_le_bytes());
    put_ids(
        &mut b,
        &r.memory_ids.iter().map(|id| id.0).collect::<Vec<_>>(),
    )?;
    b.extend_from_slice(
        &u32::try_from(r.external_memory_refs.len())
            .map_err(|_| InsomniaError::InvalidField("external refs"))?
            .to_le_bytes(),
    );
    for x in &r.external_memory_refs {
        put_str(&mut b, &x.owner_id)?;
        b.extend_from_slice(&x.memory_id.0)
    }
    Ok(b)
}
pub(super) fn decode_receipt(
    b: &[u8],
) -> Result<([u8; 16], AcceptedCompletionState), InsomniaError> {
    let mut p = 8;
    let owner = take::<16>(b, &mut p)?;
    let episode_id = EpisodeId(take::<32>(b, &mut p)?);
    let attempt = u32::from_le_bytes(take::<4>(b, &mut p)?);
    let started_at_ns = i64::from_le_bytes(take::<8>(b, &mut p)?);
    let completed_at_ns = i64::from_le_bytes(take::<8>(b, &mut p)?);
    let extractor_model = get_str(b, &mut p)?;
    let extractor_version = get_str(b, &mut p)?;
    let rejected_count = u32::from_le_bytes(take::<4>(b, &mut p)?);
    let memory_ids = get_ids(b, &mut p)?.into_iter().map(MemoryId).collect();
    let n = u32::from_le_bytes(take::<4>(b, &mut p)?) as usize;
    if n > MAX_RECEIPT_IDS {
        return Err(InsomniaError::CorruptRecord("too many external refs"));
    }
    let mut external_memory_refs = Vec::with_capacity(n);
    for _ in 0..n {
        external_memory_refs.push(ExternalMemoryRef {
            owner_id: get_str(b, &mut p)?,
            memory_id: MemoryId(take::<32>(b, &mut p)?),
        })
    }
    end(b, p)?;
    let r = AcceptedCompletionState {
        episode_id,
        attempt,
        started_at_ns,
        completed_at_ns,
        extractor_model,
        extractor_version,
        memory_ids,
        external_memory_refs,
        rejected_count,
    };
    validate_receipt(&r)?;
    Ok((owner, r))
}
