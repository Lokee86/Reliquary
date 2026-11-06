//! Reachability for disposable native external-index snapshots.
//! Replays the index's reset semantics, keeping the final namespace heads
//! and the latest discard/damage barrier required by its sticky damage flag.
use super::*;
use crate::records::external_index::compact_index::{self, Record};
use std::collections::BTreeMap;

pub(super) struct IndexReachability {
    pub(super) discard_offsets: BTreeSet<u64>,
    pub(super) packed_ids: BTreeSet<[u8; 32]>,
}

pub(super) fn scan_indexes(source: &Container) -> Result<IndexReachability, String> {
    let mut all = BTreeSet::new();
    let mut last_barrier = None;
    let mut heads = BTreeMap::new();
    for_each_frame(source, |object, bytes| {
        if !bytes.starts_with(compact_index::MAGIC) {
            return Ok(());
        }
        all.insert(object.offset);
        match compact_index::decode(bytes) {
            Ok(Record::Snapshot(state)) => {
                heads.insert(state.namespace.clone(), (object.offset, state));
            }
            Ok(Record::Discard) | Err(_) => {
                heads.clear();
                last_barrier = Some(object.offset);
            }
        }
        Ok(())
    })?;
    let mut keep = heads
        .values()
        .map(|(offset, _)| *offset)
        .collect::<BTreeSet<_>>();
    if let Some(offset) = last_barrier {
        keep.insert(offset);
    }
    let packed_ids = heads
        .values()
        .flat_map(|(_, state)| &state.messages)
        .flat_map(|message| &message.chunks)
        .map(|chunk| chunk.matrix)
        .collect();
    Ok(IndexReachability {
        discard_offsets: all.difference(&keep).copied().collect(),
        packed_ids,
    })
}
