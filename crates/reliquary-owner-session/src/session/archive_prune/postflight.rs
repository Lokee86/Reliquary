//! Compare Archive's retained source and historical projections after reopening.
use super::*;

pub(super) fn verify_reopened(
    before: &RelSession,
    after: &RelSession,
    selected: &BTreeSet<String>,
) -> Result<(), String> {
    let archive = &before.archive;
    let retained = &after.archive;
    if retained.archive_version() != archive.archive_version()
        || retained.rel_turn_count() != archive.rel_turn_count()
        || retained.accepted_turn_sources() != archive.accepted_turn_sources()
        || retained.branches() != archive.branches()
        || retained.episodes() != archive.episodes()
        || retained.fragment_definitions() != archive.fragment_definitions()
        || retained.import_receipts() != archive.import_receipts()
        || after.archive_vectors.infos() != before.archive_vectors.infos()
        || after.generations.generations() != before.generations.generations()
        || after.common.packed_vectors.infos() != before.common.packed_vectors.infos()
        || after.common.memory_vectors.infos() != before.common.memory_vectors.infos()
    {
        return Err("Archive pruning changed retained owner state or version cuts".into());
    }
    for source in archive.accepted_turn_sources() {
        let conversation = &source.conversation_id;
        let id = &source.node_id;
        if archive.node(conversation, id) != retained.node(conversation, id)
            || archive.node_archive_version(conversation, id)
                != retained.node_archive_version(conversation, id)
            || archive.activity_position_for_turn(conversation, id)
                != retained.activity_position_for_turn(conversation, id)
            || archive.files_for_source(conversation, id)
                != retained.files_for_source(conversation, id)
        {
            return Err("Archive pruning changed retained source history".into());
        }
        if selected.contains(conversation) {
            if retained.content_for_node(conversation, id).is_some()
                || retained.resolved_turn(conversation, id).is_ok()
            {
                return Err("Archive pruning retained terminal transcript text".into());
            }
        } else if archive.content_for_node(conversation, id)
            != retained.content_for_node(conversation, id)
        {
            return Err("Archive pruning changed unselected transcript text".into());
        }
    }
    for conversation in selected {
        if archive.conversation_tombstone(conversation)
            != retained.conversation_tombstone(conversation)
        {
            return Err("Archive pruning changed terminal lifecycle evidence".into());
        }
    }
    Ok(())
}
