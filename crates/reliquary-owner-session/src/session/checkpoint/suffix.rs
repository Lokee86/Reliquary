use super::super::{
    GraphAdmission, MemoryBodyId, MemoryId, OwnerSessionError, RelSession,
    identity_separation::{PREFIX as SEPARATION_PREFIX, SeparationProjection},
    observations,
    rel_dependencies::REL_DEPENDENCY_MAGIC,
    replay, replay_prefix,
};
use crate::records::user_memory::UserMemoryRecord;
use reliquary_container::{Container, PhysicalCheckpoint};
use reliquary_memory_core::{
    MemoryReplayStart, body_id_in_frame, decode_grouped_creation, history_change_in_frame,
    memory_id,
};
use std::collections::BTreeSet;

/// A seeded replay only accepts carriers whose complete cross-store effects
/// are handled here. Other carriers go through full admission.
pub(super) fn replay(
    session: &mut RelSession,
    container: &Container,
    checkpoint: &PhysicalCheckpoint,
    extent: u64,
) -> Result<bool, OwnerSessionError> {
    if checkpoint.covered_end == extent {
        return Ok(true);
    }
    if !session.common.maintenance.is_empty() {
        return Ok(false);
    }
    if let Some(epoch) = session.imports.open_epoch() {
        if epoch.format_version != 2
            || session.imports.settlement(&epoch.import_id).is_some()
            || session.has_native_import_historical_freshness(&epoch.import_id)
        {
            return Ok(false);
        }
        let import_id = epoch.import_id.clone();
        let pre_import_memory = epoch.pre.memory_version;
        return replay_import_suffix(
            session,
            container,
            checkpoint.covered_end,
            &import_id,
            pre_import_memory,
        );
    }
    let mut frames = container.scan_frames_from(checkpoint.covered_end)?;
    let mut memory_changed = false;
    let mut suffix_bodies = BTreeSet::new();
    while let Some(frame) = frames.next_frame()? {
        let payload = frame.payload;
        if payload.starts_with(b"RELVERS1") || payload.starts_with(b"RELVERS2") {
            continue; // Already accepted by the physical clock.
        }
        if session.common.ego.replay_frame(payload)? {
            continue;
        }
        if payload.starts_with(REL_DEPENDENCY_MAGIC) {
            session.rel_dependencies.replay(session.owner_id, payload)?;
            continue;
        }
        if reliquary_archive::is_turn_or_branch_publication(payload)?
            || reliquary_archive::is_source_removal_publication(payload)?
        {
            // These publications carry their own coordinates. The separate
            // project-binding carriers that need a frontier use full replay.
            continue;
        }
        let envelope = reliquary_container::decode_publication(payload)?;
        let body = envelope.as_ref().map_or(payload, |value| value.body);
        if body.starts_with(super::super::identity_receipt::PREFIX) {
            return Ok(false);
        }
        if body.starts_with(SEPARATION_PREFIX) || body.starts_with(observations::PREFIX) {
            continue;
        }
        if let Some(id) = body_id_in_frame(body)?
            && !session.common.memories.contains_body_payload(id)
        {
            if session.common.memories.references_body(id) {
                // Newly backed revisions can change cross-store effects.
                return Ok(false);
            }
            suffix_bodies.insert(id);
        }
        if native_memory_frame(body) || (envelope.is_none() && management_frame(payload)) {
            memory_changed = true;
        } else {
            return Ok(false);
        }
    }

    replay_typed_suffix(
        session,
        container,
        checkpoint.covered_end,
        memory_changed,
        suffix_bodies,
    )
}

fn replay_import_suffix(
    session: &mut RelSession,
    container: &Container,
    covered_end: u64,
    import_id: &str,
    pre_import_memory: u64,
) -> Result<bool, OwnerSessionError> {
    let mut frames = container.scan_frames_from(covered_end)?;
    let mut source_tail = false;
    let mut dream_tail = false;
    let mut native_sources = super::dream_suffix::NativeDreamSources::default();
    while let Some(frame) = frames.next_frame()? {
        let payload = frame.payload;
        if payload.starts_with(b"RELVERS1") || payload.starts_with(b"RELVERS2") {
            continue;
        }
        if payload.starts_with(b"RELTURN2") || payload.starts_with(b"RELBRCH1") {
            if dream_tail {
                return Ok(false);
            }
            source_tail = true;
            session
                .archive
                .replay_frame(container, frame.object, payload, 0)?;
        } else {
            // Current Dream only changes projections it can validate against a
            // fixed Archive and extraction cohort. Other histories go through
            // canonical replay, which discards this provisional session.
            if source_tail
                || super::dream_suffix::replay(
                    session,
                    payload,
                    import_id,
                    pre_import_memory,
                    &mut native_sources,
                )
                .is_err()
            {
                return Ok(false);
            }
            dream_tail = true;
        }
    }
    Ok(true)
}

fn native_memory_frame(payload: &[u8]) -> bool {
    [b"RELMBOD1", b"RELMCRT1", b"RELMDLT1", b"RELMTMB1"]
        .iter()
        .any(|marker| payload.starts_with(*marker))
}

fn management_frame(payload: &[u8]) -> bool {
    payload.starts_with(b"RELMCOR") || payload.starts_with(b"RELMRET")
}

fn replay_typed_suffix(
    session: &mut RelSession,
    container: &Container,
    covered_end: u64,
    memory_changed: bool,
    mut suffix_bodies: BTreeSet<MemoryBodyId>,
) -> Result<bool, OwnerSessionError> {
    let common = &mut session.common;
    let mut memory_head = common.memories.memory_version();
    let mut touched = BTreeSet::new();
    if memory_changed {
        common.memories = replay::replay_memories(
            container,
            MemoryReplayStart::Checkpoint {
                store: std::mem::take(&mut common.memories),
                covered_end,
            },
            true,
            &mut BTreeSet::new(),
            &mut common.user_memory_requests,
        )?;
    }

    let mut frames = container.scan_frames_from(covered_end)?;
    while let Some(frame) = frames.next_frame()? {
        if reliquary_archive::is_turn_or_branch_publication(frame.payload)?
            || reliquary_archive::is_source_removal_publication(frame.payload)?
        {
            if let Some(change) = reliquary_archive::copied_source_change_in_frame(frame.payload)? {
                let request = &change.request;
                if session
                    .copied_source_head(
                        &request.namespace,
                        &request.source_id,
                        &request.conversation_id,
                        &request.source_sha256,
                    )?
                    .is_none()
                {
                    return Err(OwnerSessionError::Record(
                        "checkpoint copied source head is unavailable".into(),
                    ));
                }
            }
            session
                .archive
                .replay_frame(container, frame.object, frame.payload, 0)?;
            continue;
        }
        if frame.payload.starts_with(REL_DEPENDENCY_MAGIC) {
            continue; // Already applied in the eligibility pass.
        }
        let envelope = reliquary_container::decode_publication(frame.payload)?;
        let body = envelope.as_ref().map_or(frame.payload, |value| value.body);
        if body.starts_with(observations::PREFIX) {
            observations::admit_suffix(session, frame.payload, memory_head, &suffix_bodies)?;
            continue;
        }
        if let Some(id) = body_id_in_frame(body)? {
            suffix_bodies.remove(&id);
        }
        let common = &mut session.common;
        if let Some(record) =
            UserMemoryRecord::decode(frame.payload).map_err(OwnerSessionError::Record)?
        {
            replay_prefix::validate_user_memory_prefix(
                container,
                &common.memories,
                memory_head,
                common.graph.graph_version(),
                &record,
            )?;
            if let Some(edge) = record.relation() {
                let admission = GraphAdmission::for_changes(
                    session.owner_id,
                    [memory_id(&record.key).0, record.previous.id],
                    [],
                );
                common
                    .graph
                    .apply_management_supersession(&admission, edge)?;
            }
            for child in record.history_frames() {
                track_memory_change(child, &mut memory_head, &mut touched)?;
            }
        } else {
            if body.starts_with(SEPARATION_PREFIX) {
                if !session.identity_separation_history.replay(
                    frame.payload,
                    SeparationProjection {
                        owner: session.owner_id,
                        history_generation: session.history_generation,
                        receipts: &session.identity_history,
                        perception: &common.perception,
                        memory_cut: memory_head,
                        perception_cut: common.perception.version(),
                    },
                )? {
                    return Ok(false);
                }
            } else if memory_changed {
                track_memory_change(body, &mut memory_head, &mut touched)?;
            }
        }
    }
    let common = &mut session.common;
    if memory_head != common.memories.memory_version() {
        return Err(OwnerSessionError::Record(
            "checkpoint Memory frontier differs".into(),
        ));
    }
    // A replacement with missing backing could be part of a retained-prune
    // proof, which is admitted by the full owner replay.
    if touched.iter().any(|id| {
        common
            .memories
            .revision_records_at_cut(*id, memory_head)
            .is_some_and(|revisions| {
                revisions
                    .iter()
                    .any(|record| !common.memories.contains_body_payload(record.body_id))
            })
    }) {
        return Ok(false);
    }
    if memory_changed {
        common.refresh_dream_index(&touched.into_iter().collect::<Vec<_>>())?;
    }
    Ok(true)
}

fn track_memory_change(
    payload: &[u8],
    head: &mut u64,
    touched: &mut BTreeSet<MemoryId>,
) -> Result<(), OwnerSessionError> {
    let change = if payload.starts_with(b"RELMCRT1") {
        let record = decode_grouped_creation(payload)?;
        Some((record.id, record.memory_version))
    } else {
        history_change_in_frame(payload)?
    };
    if let Some((id, version)) = change {
        *head = version;
        touched.insert(id);
    }
    Ok(())
}
