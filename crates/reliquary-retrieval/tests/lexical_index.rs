use reliquary_archive::{Fragment, FragmentId};
use reliquary_memory_core::MemoryId;
use reliquary_retrieval::{LexicalIndex, lexical_terms};

fn fragment(id: u8) -> Fragment {
    Fragment {
        id: FragmentId([id; 32]),
        conversation_id: format!("conversation-{id}"),
        start_node_id: format!("node-{id}"),
        end_node_id: format!("node-{id}"),
    }
}

#[test]
fn fragment_postings_accept_incremental_adds_and_sort_by_version_then_identity() {
    let mut index = LexicalIndex::default();
    let terms = lexical_terms("shared cedar");

    index.add_fragment(fragment(9), 4, "shared cedar evidence");
    let first = index.search(&terms, 10);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].fragment.id, FragmentId([9; 32]));

    index.add_fragment(fragment(7), 3, "shared cedar evidence");
    index.add_fragment(fragment(3), 4, "shared cedar evidence");
    let hits = index.search(&terms, 10);
    assert_eq!(
        hits.iter().map(|hit| hit.fragment.id).collect::<Vec<_>>(),
        vec![
            FragmentId([3; 32]),
            FragmentId([9; 32]),
            FragmentId([7; 32])
        ]
    );
}

#[test]
fn memory_postings_incrementally_add_and_deactivate_without_losing_stable_order() {
    let mut index = LexicalIndex::default();
    let terms = lexical_terms("shared cedar");
    let first = MemoryId([1; 32]);
    let second = MemoryId([2; 32]);

    index.add_memory(second, "Shared", "cedar note");
    assert_eq!(
        index
            .complete_memory_matches(&terms)
            .iter()
            .map(|hit| hit.memory_id)
            .collect::<Vec<_>>(),
        vec![second]
    );

    index.add_memory(first, "Shared", "cedar note");
    let both = index.complete_memory_matches(&terms);
    assert_eq!(
        both.iter().map(|hit| hit.memory_id).collect::<Vec<_>>(),
        vec![first, second]
    );
    assert_eq!(both[0].score.to_bits(), both[1].score.to_bits());

    index.set_memory_active(first, false);
    assert_eq!(
        index
            .complete_memory_matches(&terms)
            .iter()
            .map(|hit| hit.memory_id)
            .collect::<Vec<_>>(),
        vec![second]
    );

    index.set_memory_active(first, true);
    assert_eq!(
        index
            .complete_memory_matches(&terms)
            .iter()
            .map(|hit| hit.memory_id)
            .collect::<Vec<_>>(),
        vec![first, second]
    );
}
