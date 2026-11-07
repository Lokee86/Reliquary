//! Fresh-reader comparisons for maintenance outputs.

#[cfg(test)]
use reliquary_archive::Archive;
#[cfg(test)]
use reliquary_container::Container;
#[cfg(test)]
use reliquary_freshness_store::FreshnessMemoryId;
#[cfg(test)]
use reliquary_owner_session::{RelSession, model::CompatibilityProfileId};
#[cfg(test)]
use reliquary_vector_store::generation::decode_generation;
#[cfg(test)]
use serde_json::{Value, json};
#[cfg(test)]
use std::{collections::BTreeSet, path::Path};

/// Compare retained owner authority using two independent read-only opens.
///
/// `witness` is the authored semantic completion witness. It supplies the
/// accepted import identity without relying on physical frame offsets. The destination may contain additional
/// accepted Archive turns, so source Archive authority is checked as a subset.
#[cfg(test)]
pub(crate) fn compare_preserved_authority(
    source: &Path,
    destination: &Path,
    witness: &Value,
    memory_profile: CompatibilityProfileId,
) -> Value {
    let source_container = Container::open_read_only(source).expect("open authority source");
    let destination_container =
        Container::open_read_only(destination).expect("open authority destination");
    let owner = source_container.owner_id();
    assert_eq!(
        destination_container.owner_id(),
        owner,
        "maintenance changed owner identity"
    );
    let source_rel = RelSession::open_path_current(source).expect("freshly admit source REL");
    let destination_rel =
        RelSession::open_path_current(destination).expect("freshly admit destination REL");

    let source_archive = Archive::open(&source_container).expect("open source Archive");
    let destination_archive =
        Archive::open(&destination_container).expect("open destination Archive");
    let source_episodes = source_archive.episodes();
    let destination_episodes = destination_archive.episodes();
    for episode in &source_episodes {
        assert_eq!(
            destination_episodes
                .iter()
                .find(|candidate| candidate.id == episode.id),
            Some(episode),
            "maintenance changed an authored Episode"
        );
    }
    for receipt in source_archive.import_receipts() {
        assert!(
            destination_archive.import_receipts().contains(&receipt),
            "maintenance dropped or changed an accepted import receipt"
        );
    }

    // Full Insomnia work revisions and completion carriers are immutable
    // operation records. Compare their exact decoded payloads as an ordered
    // subsequence, allowing only unrelated new owner operations to be added.
    let source_insomnia = operation_payloads(&source_container, b"RELINS");
    let destination_insomnia = operation_payloads(&destination_container, b"RELINS");
    assert!(
        is_subsequence(&source_insomnia, &destination_insomnia),
        "maintenance changed an Insomnia work history or receipt"
    );

    let (mut after, mut memory_ids) = (None, Vec::new());
    loop {
        let (_, page) = source_rel
            .memory_ids_page(after, 128)
            .expect("enumerate source Memories");
        let Some(last) = page.last().copied() else {
            break;
        };
        memory_ids.extend(page);
        after = Some(last);
    }
    let mut source_bodies = BTreeSet::new();
    for id in &memory_ids {
        let history = source_rel
            .memory_history(*id)
            .expect("read source Memory history");
        assert_eq!(
            destination_rel
                .memory_history(*id)
                .expect("read destination Memory history"),
            history,
            "maintenance changed an original Memory revision"
        );
        if let Some(history) = history {
            source_bodies.extend(
                history
                    .revisions
                    .into_iter()
                    .map(|revision| revision.record.body_id),
            );
        }
        let freshness_id = FreshnessMemoryId(id.0);
        assert_eq!(
            destination_rel
                .freshness_record(freshness_id)
                .expect("read destination Freshness"),
            source_rel
                .freshness_record(freshness_id)
                .expect("read source Freshness"),
            "maintenance changed Freshness authority for an original Memory"
        );
    }

    let source_memory_vectors = source_rel.memory_vector_infos();
    let destination_memory_vectors = destination_rel.memory_vector_infos();
    for info in &source_memory_vectors {
        assert!(
            destination_memory_vectors.contains(info),
            "maintenance dropped or changed a Memory vector mapping"
        );
    }
    for body in &source_bodies {
        let source_row = source_rel
            .memory_vector_row(memory_profile, *body)
            .expect("read source Memory vector row");
        let destination_row = destination_rel
            .memory_vector_row(memory_profile, *body)
            .expect("read destination Memory vector row");
        assert_eq!(
            destination_row, source_row,
            "maintenance changed a retained Memory vector row"
        );
    }

    let source_archive_vectors = source_rel.archive_vector_infos();
    let destination_archive_vectors = destination_rel.archive_vector_infos();
    for info in &source_archive_vectors {
        assert!(
            destination_archive_vectors.contains(info),
            "maintenance dropped or changed an Archive vector mapping"
        );
    }
    let source_generations = generation_payloads(&source_container);
    let destination_generations = generation_payloads(&destination_container);
    for generation in &source_generations {
        assert!(
            destination_generations.contains(generation),
            "maintenance dropped or changed a vector generation record"
        );
        assert_eq!(
            destination_rel.vector_generation(generation.id),
            source_rel.vector_generation(generation.id),
            "maintenance changed a retained vector generation or its accepted version"
        );
    }

    // The authored Dream frontier is keyed by its stable fixture pass. Its
    // candidate cuts plus pair-evaluated state preserve the replay frontier.
    let dream_pass = "g4-authored-first-dream";
    let source_frontier = source_rel.initial_dream_frontier(dream_pass);
    if let Some(frontier) = source_frontier.as_ref() {
        assert_eq!(
            destination_rel.initial_dream_frontier(dream_pass),
            source_frontier
        );
        assert_eq!(
            destination_rel.dream_begin(dream_pass),
            source_rel.dream_begin(dream_pass)
        );
        for candidate in &frontier.candidates {
            assert_eq!(
                destination_rel.dream_pair_evaluated(frontier.source.id, candidate.id),
                source_rel.dream_pair_evaluated(frontier.source.id, candidate.id),
                "maintenance changed Dream pair evaluation state"
            );
        }
    }
    let source_dream_records = operation_payloads(&source_container, b"CVDREAM");
    let destination_dream_records = operation_payloads(&destination_container, b"CVDREAM");
    assert!(
        is_subsequence(&source_dream_records, &destination_dream_records),
        "maintenance changed Dream frontier or receipt records"
    );
    let source_frontier_records = operation_payloads(&source_container, b"RELDR");
    let destination_frontier_records = operation_payloads(&destination_container, b"RELDR");
    assert!(
        is_subsequence(&source_frontier_records, &destination_frontier_records),
        "maintenance changed durable Dream frontier records"
    );

    let import_id = witness["native_import"]["import_id"].as_str();
    let (import_status_preserved, import_status_found) = if let Some(import_id) = import_id {
        let source_status = source_rel.import_status(import_id);
        let destination_status = destination_rel.import_status(import_id);
        assert_eq!(
            destination_status, source_status,
            "maintenance changed native import status"
        );
        assert_eq!(
            destination_rel.native_import_settlement(import_id),
            source_rel.native_import_settlement(import_id),
            "maintenance changed native import settlement"
        );
        (true, source_status.is_some())
    } else {
        (true, false)
    };

    json!({
        "owner_id": owner.iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
        "original_episodes_preserved": source_episodes.len(),
        "import_receipts_preserved": source_archive.import_receipts().len(),
        "insomnia_operation_records_preserved": source_insomnia.len(),
        "memory_histories_preserved": memory_ids.len(),
        "memory_vector_mappings_preserved": source_memory_vectors.len(),
        "memory_vector_rows_compared": source_bodies.len(),
        "archive_vector_mappings_preserved": source_archive_vectors.len(),
        "vector_generations_preserved": source_generations.len(),
        "dream_frontier_preserved": source_frontier.is_some(),
        "dream_records_preserved": source_dream_records.len(),
        "dream_frontier_records_preserved": source_frontier_records.len(),
        "native_import_status_and_settlement_preserved": import_status_preserved,
        "native_import_status_found": import_status_found,
    })
}

#[cfg(test)]
fn operation_payloads(container: &Container, magic_prefix: &[u8]) -> Vec<Vec<u8>> {
    container
        .read_frames()
        .expect("read retained owner operation frames")
        .into_iter()
        .filter(|frame| frame.payload.starts_with(magic_prefix))
        .map(|frame| frame.payload)
        .collect()
}

#[cfg(test)]
fn generation_payloads(
    container: &Container,
) -> Vec<reliquary_vector_store::generation::GenerationPayload> {
    container
        .read_frames()
        .expect("read vector generation records")
        .into_iter()
        .filter_map(|frame| decode_generation(&frame.payload).expect("decode vector generation"))
        .collect()
}

#[cfg(test)]
fn is_subsequence<T: Eq>(needle: &[T], haystack: &[T]) -> bool {
    let mut cursor = 0;
    for item in haystack {
        if needle.get(cursor) == Some(item) {
            cursor += 1;
            if cursor == needle.len() {
                return true;
            }
        }
    }
    needle.is_empty()
}
