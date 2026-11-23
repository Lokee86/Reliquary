use super::super::{
    GraphAdmission, MemoryId, OwnerSessionError, RelSession,
    rel_dependencies::REL_DEPENDENCY_MAGIC, replay, replay_prefix,
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
    // A suspended import has its own publication fence and proof rules.
    if session.imports.open_epoch().is_some() || !session.common.maintenance.is_empty() {
        return Ok(false);
    }
    let mut frames = container.scan_frames_from(checkpoint.covered_end)?;
    let mut memory_changed = false;
    while let Some(frame) = frames.next_frame()? {
        let payload = frame.payload;
        if payload.starts_with(b"RELVERS1") || payload.starts_with(b"RELVERS2") {
            continue; // Already accepted by the physical clock.
        }
        if payload.starts_with(REL_DEPENDENCY_MAGIC) {
            session.rel_dependencies.replay(session.owner_id, payload)?;
            continue;
        }
        if reliquary_archive::is_turn_or_branch_publication(payload)? {
            // These publications carry their own coordinates. The separate
            // project-binding carriers that need a frontier use full replay.
            session
                .archive
                .replay_frame(container, frame.object, payload, 0)?;
            continue;
        }
        let envelope = reliquary_container::decode_publication(payload)?;
        let body = envelope.as_ref().map_or(payload, |value| value.body);
        if body.starts_with(super::super::identity_receipt::PREFIX) {
            return Ok(false);
        }
        if let Some(id) = body_id_in_frame(body)?
            && session.common.memories.references_body(id)
            && !session.common.memories.contains_body_payload(id)
        {
            // Previously unbacked revisions can now change mention spans and
            // Dream eligibility. Admit those cross-store effects from the start.
            return Ok(false);
        }
        if native_memory_frame(body) || (envelope.is_none() && management_frame(payload)) {
            memory_changed = true;
        } else {
            return Ok(false);
        }
    }

    if memory_changed {
        return replay_memory_suffix(session, container, checkpoint.covered_end);
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

fn replay_memory_suffix(
    session: &mut RelSession,
    container: &Container,
    covered_end: u64,
) -> Result<bool, OwnerSessionError> {
    let common = &mut session.common;
    let mut memory_head = common.memories.memory_version();
    let mut touched = BTreeSet::new();
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

    let mut frames = container.scan_frames_from(covered_end)?;
    while let Some(frame) = frames.next_frame()? {
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
            let envelope = reliquary_container::decode_publication(frame.payload)?;
            track_memory_change(
                envelope.as_ref().map_or(frame.payload, |value| value.body),
                &mut memory_head,
                &mut touched,
            )?;
        }
    }
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
    common.refresh_dream_index(&touched.into_iter().collect::<Vec<_>>())?;
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
