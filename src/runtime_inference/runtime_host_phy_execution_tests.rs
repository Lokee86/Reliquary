use super::MentionQueue;
use std::time::Instant;

#[test]
fn personal_queue_deduplicates_and_tracks_in_flight_work() {
    let mut queue = MentionQueue::default();
    let key = crate::MemoryEntityMentionKey {
        memory_id: crate::MemoryId([7; 32]),
        field: crate::MemoryTextField::Title,
        start_byte: 0,
        end_byte: 4,
    };
    queue.push(key);
    queue.push(key);
    assert_eq!(queue.len(), 1);
    assert_eq!(queue.pop_ready(Instant::now()), Some(key));
    assert_eq!(queue.len(), 1);
    queue.finish();
    assert_eq!(queue.len(), 0);
}

#[test]
fn personal_queue_defers_without_losing_deduplication() {
    let mut queue = MentionQueue::default();
    let key = crate::MemoryEntityMentionKey {
        memory_id: crate::MemoryId([9; 32]),
        field: crate::MemoryTextField::Content,
        start_byte: 2,
        end_byte: 8,
    };
    queue.defer(key);
    assert_eq!(queue.pop_ready(Instant::now()), None);
    queue.push(key);
    assert_eq!(queue.len(), 1);
}
