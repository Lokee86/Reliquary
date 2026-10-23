use crate::{Cva, CvaReconcileError};

pub(crate) use crate::memory_ordered_replay::{
    MemoryReplayDestination, MemoryReplayResult, MemoryReplaySource, MemoryTail,
};

pub(crate) fn read_memory_tail(
    owner: &mut Cva,
    start_chunk: usize,
) -> Result<MemoryTail, CvaReconcileError> {
    read_memory_range(owner, start_chunk, usize::MAX)
}

/// Read a finite source interval without moving later epochs ahead of it.
pub(crate) fn read_memory_range(
    owner: &mut Cva,
    start_chunk: usize,
    end_chunk: usize,
) -> Result<MemoryTail, CvaReconcileError> {
    crate::memory_ordered_replay::read_memory_range(
        MemoryReplaySource {
            container: &mut owner.container,
            memories: &owner.memories,
        },
        start_chunk,
        end_chunk,
    )
}

pub(crate) fn replay_memory_tail(
    owner: &mut Cva,
    tail: MemoryTail,
) -> Result<MemoryReplayResult, CvaReconcileError> {
    crate::memory_ordered_replay::replay_memory_tail(
        MemoryReplayDestination {
            archive: &owner.archive,
            memories: &mut owner.memories,
            insomnia: &mut owner.insomnia,
            container: &mut owner.container,
        },
        tail,
    )
}
