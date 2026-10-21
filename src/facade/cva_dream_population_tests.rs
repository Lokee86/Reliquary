use super::*;
use crate::{Cva, MemoryDraft};

struct Temp(std::path::PathBuf);

impl Temp {
    fn new() -> Self {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join(format!("dream-population-prefix-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn owner_path(&self) -> std::path::PathBuf {
        self.0.join("owner.rel")
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn draft(mutation: &str, lifecycle_state: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: mutation.into(),
        content: format!("Dream prefix test {mutation}"),
        scope: "project".into(),
        lifecycle_state: lifecycle_state.into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        mutation_id: mutation.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn append_ordered_memory_creation(
    owner: &mut Cva,
    id: crate::MemoryId,
    mutation: &str,
    memory_version: u64,
    global_version: u64,
    lifecycle_state: &str,
) {
    let title = mutation;
    let content = format!("Dream population coordinate fixture {mutation}.");
    let body_id = crate::memory_model::memory_body_id(title, &content);
    let body = crate::memory_model::memory_body_bytes(title, &content);
    owner
        .container
        .append(&crate::memory_codec::encode_body(body_id, &body).unwrap())
        .unwrap();
    let record = crate::memory_model::MemoryRecord {
        id,
        revision: 1,
        body_id,
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "project".into(),
        lifecycle_state: lifecycle_state.into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        source_ref: None,
        temporal_inference: None,
        mutation_id: mutation.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
        global_version,
        memory_version,
    };
    let body = crate::memory_compact_codec::publication::encode(
        &crate::memory_compact_codec::publication::MemoryPublication::Creation(
            crate::memory_compact_contract::MemoryCreation::new(record).unwrap(),
        ),
    )
    .unwrap();
    let envelope = crate::container::publication_envelope::encode(
        crate::container::publication_envelope::PublicationEnvelope {
            global_version,
            clock: crate::container::publication_envelope::PublicationClock::BindReserved,
            body: &body,
        },
    )
    .unwrap();
    owner.container.append(&envelope).unwrap();
}

#[test]
fn physical_boundary_materializes_only_accepted_preceding_births_and_is_read_only() {
    let temp = Temp::new();
    let path = temp.owner_path();
    let mut owner = Cva::create(&path).unwrap();
    let mut prior_draft = draft("prefix-prior", "knowledge");
    prior_draft.archived = true;
    let prior = owner.publish_memory(None, 0, prior_draft).unwrap().0.id;
    let tombstoned = owner
        .publish_memory(None, 0, draft("prefix-tombstoned", "knowledge"))
        .unwrap()
        .0
        .id;
    owner.tombstone_memory(tombstoned, 1).unwrap();
    let source = owner
        .publish_memory(None, 0, draft("prefix-source", "extracted"))
        .unwrap()
        .0
        .id;
    let source_global = owner
        .memories
        .first_publication_prefix(owner.memories.memory_version())
        .unwrap()
        .into_iter()
        .find_map(|(id, global)| (id == source).then_some(global))
        .unwrap();
    let source_memory_cut = owner.memories.memory_version();
    let source_object = owner
        .container
        .chunks()
        .unwrap()
        .into_iter()
        .find(|chunk| {
            let payload = owner.container.read(*chunk).unwrap();
            crate::container::publication_envelope::decode(&payload)
                .unwrap()
                .is_some_and(|envelope| envelope.global_version == source_global)
        })
        .unwrap();
    let source_end = source_object.legacy_offset() + 8 + source_object.legacy_len();
    let source_start = source_object.legacy_offset();
    // A later birth exists at EOF but is beyond the accepted Begin boundary.
    owner
        .publish_memory(None, 0, draft("prefix-later", "knowledge"))
        .unwrap();
    owner.sync().unwrap();
    let bytes_before = std::fs::read(&path).unwrap();
    let clock_before = owner.latest_global_version();
    let population = owner
        .dream_population_at_prefix(source_end, source_memory_cut, source)
        .unwrap();
    assert_eq!(population, BTreeSet::from([prior, tombstoned]));
    assert_eq!(owner.latest_global_version(), clock_before);
    assert_eq!(std::fs::read(&path).unwrap(), bytes_before);

    assert!(
        owner
            .dream_population_at_prefix(source_end, source_memory_cut - 1, source)
            .is_err()
    );
    assert!(
        owner
            .dream_population_at_prefix(source_start, source_memory_cut - 1, source)
            .is_err()
    );

    let eof = std::fs::metadata(&path).unwrap().len();
    assert!(
        owner
            .dream_population_at_prefix(eof, source_memory_cut, source)
            .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes_before);
}

#[test]
fn replay_birth_after_boundary_is_visible_only_to_later_physical_prefixes() {
    let temp = Temp::new();
    let path = temp.owner_path();
    let mut owner = Cva::create(&path).unwrap();
    let candidate = owner
        .memories
        .publish_legacy_fixture(
            &mut owner.container,
            None,
            0,
            draft("replayed-prefix-candidate", "knowledge"),
        )
        .unwrap()
        .0
        .id;
    let source = owner
        .publish_memory(None, 0, draft("replayed-prefix-source", "extracted"))
        .unwrap()
        .0
        .id;
    let source_global = owner
        .memories
        .first_publication_prefix(owner.memories.memory_version())
        .unwrap()
        .into_iter()
        .find_map(|(id, global)| (id == source).then_some(global))
        .unwrap();
    let source_cut = owner.memories.memory_version();
    let source_end = owner
        .container
        .chunks()
        .unwrap()
        .into_iter()
        .find_map(|chunk| {
            let payload = owner.container.read(chunk).unwrap();
            crate::container::publication_envelope::decode(&payload)
                .unwrap()
                .filter(|envelope| envelope.global_version == source_global)
                .map(|_| chunk.legacy_offset() + 8 + chunk.legacy_len())
        })
        .unwrap();
    assert!(
        owner
            .dream_population_at_prefix(source_end, source_cut, source)
            .unwrap()
            .is_empty()
    );

    owner.freshness_replay_publication_birth(candidate).unwrap();
    owner.sync().unwrap();
    let before = std::fs::read(&path).unwrap();
    let eof = std::fs::metadata(&path).unwrap().len();
    assert_eq!(
        owner
            .dream_population_at_prefix(eof, source_cut, source)
            .unwrap(),
        BTreeSet::from([candidate])
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn physical_grouped_completion_excludes_its_siblings_from_source_population() {
    let (_path, mut owner, _) =
        crate::insomnia::completion_tests::setup("dream-population-grouped-prefix.cva");
    let prior = owner
        .publish_memory(None, 0, draft("grouped-prefix-prior", "knowledge"))
        .unwrap()
        .0
        .id;
    let completion = crate::insomnia::completion_tests::process(&mut owner, 3, 110);
    let grouped_end = owner
        .container
        .chunks()
        .unwrap()
        .into_iter()
        .find_map(|chunk| {
            let payload = owner.container.read(chunk).unwrap();
            crate::insomnia::completion::decode_completion(&payload)
                .unwrap()
                .map(|_| chunk.legacy_offset() + 8 + chunk.legacy_len())
        })
        .expect("accepted grouped completion");
    let source = completion.created.first().expect("group members").id;
    let memory_cut = owner.memories.memory_version();
    let population = owner
        .dream_population_at_prefix(grouped_end, memory_cut, source)
        .unwrap();
    assert_eq!(population, BTreeSet::from([prior]));
    assert!(
        completion
            .created
            .iter()
            .skip(1)
            .all(|sibling| !population.contains(&sibling.id))
    );
}

#[test]
fn physical_external_birth_receipt_excludes_its_cohort_siblings() {
    let temp = Temp::new();
    let path = temp.owner_path();
    let mut owner = Cva::create(&path).unwrap();
    let prior = owner
        .publish_memory(None, 0, draft("external-prefix-prior", "knowledge"))
        .unwrap()
        .0
        .id;
    let window = [73; 32];
    let created = owner
        .publish_external_window(
            window,
            &[
                super::external_birth_prefix_tests::prepared(window, 1),
                super::external_birth_prefix_tests::prepared(window, 2),
            ],
        )
        .unwrap();
    let source = created[0].id;
    let grouped_end = owner
        .container
        .chunks()
        .unwrap()
        .into_iter()
        .find_map(|chunk| {
            let payload = owner.container.read(chunk).unwrap();
            crate::external_source::publication::decode(&payload)
                .unwrap()
                .map(|_| chunk.legacy_offset() + 8 + chunk.legacy_len())
        })
        .expect("external publication");
    let memory_cut = owner.memories.memory_version();
    let population = owner
        .dream_population_at_prefix(grouped_end, memory_cut, source)
        .unwrap();
    assert_eq!(population, BTreeSet::from([prior]));
    assert!(!population.contains(&created[1].id));
}

#[test]
fn bounded_population_replays_reserved_global_order_independently_of_memory_local_order() {
    let temp = Temp::new();
    let path = temp.owner_path();
    let mut owner = Cva::create(&path).unwrap();
    for time in [10, 20, 30] {
        owner
            .container
            .allocate_version_with_transaction_time(Some(time))
            .unwrap();
    }
    let local_first_global_last = crate::MemoryId([81; 32]);
    let global_first = crate::MemoryId([82; 32]);
    let source = crate::MemoryId([83; 32]);
    append_ordered_memory_creation(
        &mut owner,
        local_first_global_last,
        "population-local-first-global-last",
        1,
        3,
        "knowledge",
    );
    append_ordered_memory_creation(
        &mut owner,
        global_first,
        "population-global-first",
        2,
        1,
        "knowledge",
    );
    append_ordered_memory_creation(
        &mut owner,
        source,
        "population-source-middle-global",
        3,
        2,
        "extracted",
    );
    owner.sync().unwrap();
    drop(owner);

    let mut owner = Cva::open(&path).unwrap();
    for id in [local_first_global_last, global_first, source] {
        owner.freshness_replay_publication_birth(id).unwrap();
    }
    let cut = owner.memories.memory_version();
    let eof = std::fs::metadata(&path).unwrap().len();
    let before = std::fs::read(&path).unwrap();
    assert_eq!(
        owner.dream_population_at_prefix(eof, cut, source).unwrap(),
        BTreeSet::from([global_first])
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
