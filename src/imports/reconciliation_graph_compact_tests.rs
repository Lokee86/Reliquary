use super::*;
use crate::freshness_storage::HistoricalImportFreshnessSettlement;
use crate::graph_codec::{GraphMutationPayload, GraphVersionPayload, encode_version};
use crate::graph_compact_codec::{encode_batch, encode_mutation};
use crate::graph_compact_contract::GraphOrdinalMutation;
use crate::imports::reconciliation_interval::Coordinates;
use crate::imports::{ImportComplete, ImportStart, ImportWatermarks};
use crate::{
    Cva, GraphRelationKind, GraphRelationOrigin, MemoryDraft, MemoryId, SemanticGraphRelationKind,
    SemanticNodeRef,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("reconcile-compact-graph-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn draft(name: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: name.into(),
        content: format!("body {name}"),
        scope: "project".into(),
        lifecycle_state: "knowledge".into(),
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
        mutation_id: name.into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

fn publish_pair(cva: &mut Cva, names: (&str, &str)) -> (MemoryId, MemoryId) {
    (
        cva.publish_memory(None, 0, draft(names.0)).unwrap().0.id,
        cva.publish_memory(None, 0, draft(names.1)).unwrap().0.id,
    )
}

fn append_compact(cva: &mut Cva, graph_version: u64, a: MemoryId, b: MemoryId) {
    let mutation = GraphOrdinalMutation {
        source: cva.graph.node_id(a).unwrap().0,
        target: cva.graph.node_id(b).unwrap().0,
        kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
        active: true,
        origin: GraphRelationOrigin::User,
    };
    let carrier = cva.container.append(&encode_mutation(mutation)).unwrap();
    let global_version = cva
        .container
        .allocate_version_with_transaction_time(Some(20_000 + graph_version as i64))
        .unwrap();
    cva.container
        .append(&encode_version(GraphVersionPayload {
            global_version,
            graph_version,
            mutation: carrier,
        }))
        .unwrap();
}

fn append_legacy(cva: &mut Cva, graph_version: u64, a: MemoryId, b: MemoryId) {
    let carrier = cva
        .container
        .append(&crate::graph_codec::encode_mutation(GraphMutationPayload {
            source: SemanticNodeRef::memory(a),
            target: SemanticNodeRef::memory(b),
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
            active: true,
            origin: GraphRelationOrigin::User,
        }))
        .unwrap();
    let global_version = cva
        .container
        .allocate_version_with_transaction_time(Some(20_000 + graph_version as i64))
        .unwrap();
    cva.container
        .append(&encode_version(GraphVersionPayload {
            global_version,
            graph_version,
            mutation: carrier,
        }))
        .unwrap();
}

fn epoch(first: usize, end: usize) -> super::CompletedEpoch {
    let start = ImportStart {
        owner_uuid: [1; 16],
        import_id: "epoch".into(),
        import_key: "key".into(),
        source_namespace: "fixture".into(),
        source_format: "fixture".into(),
        started_at_ns: 1,
        pre: ImportWatermarks::default(),
    };
    let complete = ImportComplete {
        owner_uuid: start.owner_uuid,
        import_id: start.import_id.clone(),
        start_digest: String::new(),
        receipt_digest: String::new(),
        completed_at_ns: 2,
        post: ImportWatermarks::default(),
        historical_freshness_settlement_digest: String::new(),
    };
    let settlement = HistoricalImportFreshnessSettlement {
        epoch: None,
        semantic: None,
        epoch_prefix_digest: [0; 32],
        owner: [1; 16],
        import_id: [2; 32],
        start_evidence_digest: [3; 32],
        corpus_digest: [4; 32],
        manifest_digest: [5; 32],
        input_digest: [6; 32],
        cohort: BTreeSet::new(),
        base_cut: 0,
        settlement_cut: 0,
        global_version: 0,
        input_bytes: 0,
        archive_version: 0,
        graph_version: 0,
        memory_graph_version: 0,
        records: BTreeMap::new(),
        superseded: BTreeSet::new(),
    };
    super::CompletedEpoch {
        start,
        complete,
        settlement,
        first,
        end,
        completions: Vec::new(),
        archive_records: Vec::new(),
        events: Vec::new(),
        dream: Vec::new(),
        owner_evidence: Vec::new(),
    }
}

#[test]
fn retained_compact_graph_interval_maps_source_local_graph_cut_to_destination_cut() {
    let scratch = Scratch::new();
    let source_path = scratch.file("source.rel");
    let mut source = Cva::create(&source_path).unwrap();
    let (a, b) = publish_pair(&mut source, ("source-a", "source-b"));
    source
        .set_memory_relation_with_origin(
            a,
            b,
            GraphRelationKind::Factual,
            true,
            GraphRelationOrigin::User,
            source.graph_version(),
        )
        .unwrap();
    let source_boundary = source.container.chunks().unwrap().len();
    append_compact(&mut source, 2, a, b);
    let source_end = source.container.chunks().unwrap().len();

    let destination_path = scratch.file("destination.rel");
    let mut destination = Cva::create(&destination_path).unwrap();
    let (x, y) = publish_pair(&mut destination, ("destination-a", "destination-b"));
    destination
        .set_memory_relation_with_origin(
            x,
            y,
            GraphRelationKind::Factual,
            true,
            GraphRelationOrigin::User,
            destination.graph_version(),
        )
        .unwrap();
    // A destination-only publication shifts the source tail's local cut.
    destination
        .set_memory_relation_with_origin(
            y,
            x,
            GraphRelationKind::Topical,
            true,
            GraphRelationOrigin::User,
            destination.graph_version(),
        )
        .unwrap();
    let destination_boundary = destination.container.chunks().unwrap().len();
    append_legacy(&mut destination, 3, x, y);
    let destination_end = destination.container.chunks().unwrap().len();

    let mut coordinates = Coordinates::shared(&mut source, source_boundary).unwrap();
    coordinates
        .retained_epoch(
            &mut source,
            &mut destination,
            &epoch(source_boundary, source_end),
            &epoch(destination_boundary, destination_end),
        )
        .unwrap();
    let rebased = coordinates
        .proof(crate::FreshnessEventProof {
            graph_version: 2,
            community_generation: None,
            community_graph_version: None,
        })
        .unwrap();
    assert_eq!(rebased.graph_version, 3);
}

#[test]
fn replay_interval_classifier_accepts_valid_compact_graph_and_fails_closed_on_bad_frames() {
    let scratch = Scratch::new();
    let valid = encode_batch(&[
        GraphOrdinalMutation {
            source: 0,
            target: 1,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
            active: true,
            origin: GraphRelationOrigin::Dream,
        },
        GraphOrdinalMutation {
            source: 1,
            target: 0,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Topical),
            active: true,
            origin: GraphRelationOrigin::User,
        },
    ])
    .unwrap();
    for (name, payload, accepted) in [
        ("valid-compact.rel", valid.as_slice(), true),
        ("unknown-compact-version.rel", b"CVAGCM02".as_slice(), false),
        ("truncated-compact.rel", b"CVAGCM01".as_slice(), false),
    ] {
        let path = scratch.file(name);
        let mut cva = Cva::create(&path).unwrap();
        let boundary = cva.container.chunks().unwrap().len();
        cva.container.append(payload).unwrap();
        assert_eq!(
            super::validate_replay_interval(&mut cva, boundary).is_ok(),
            accepted,
            "{name}"
        );
    }
}

fn last_graph_evidence(path: &std::path::Path) -> (Vec<u8>, Vec<u8>) {
    let mut owner = Cva::open(path).unwrap();
    let mut last = None;
    for object in owner.container.chunks().unwrap() {
        if let Some(version) =
            crate::graph_codec::decode_version(&owner.container.read(object).unwrap()).unwrap()
        {
            last = Some(version);
        }
    }
    let version = last.unwrap();
    let raw = owner.container.read(version.mutation).unwrap();
    (
        raw,
        super::canonical_graph_publication(&mut owner, version).unwrap(),
    )
}

#[test]
fn completed_import_graph_evidence_is_stable_across_wire_formats_and_independent_mapping_scopes() {
    let scratch = Scratch::new();
    let base_path = scratch.file("evidence-base.rel");
    let mut base = Cva::create(&base_path).unwrap();
    let (a, b) = publish_pair(&mut base, ("evidence-a", "evidence-b"));
    base.sync().unwrap();
    drop(base);
    let left_path = scratch.file("evidence-left.rel");
    let right_path = scratch.file("evidence-right.rel");
    let legacy_path = scratch.file("evidence-legacy.rel");
    for path in [&left_path, &right_path, &legacy_path] {
        fs::copy(&base_path, path).unwrap();
    }
    for (path, reverse, legacy) in [
        (&left_path, false, false),
        (&right_path, true, false),
        (&legacy_path, false, true),
    ] {
        let mut owner = Cva::open(path).unwrap();
        let (from, to) = if reverse { (b, a) } else { (a, b) };
        owner
            .set_memory_relation_with_origin(
                from,
                to,
                GraphRelationKind::Factual,
                true,
                GraphRelationOrigin::User,
                0,
            )
            .unwrap();
        if legacy {
            append_legacy(&mut owner, 2, a, b);
        } else {
            append_compact(&mut owner, 2, a, b);
        }
        owner.sync().unwrap();
    }
    let (left_raw, left) = last_graph_evidence(&left_path);
    let (right_raw, right) = last_graph_evidence(&right_path);
    let (legacy_raw, legacy) = last_graph_evidence(&legacy_path);
    assert_ne!(
        left_raw, right_raw,
        "same stable edge uses source-local ordinals"
    );
    assert_ne!(left_raw, legacy_raw, "carrier formats differ");
    assert_eq!(left, right);
    assert_eq!(left, legacy);
    let mut owner = Cva::open(&right_path).unwrap();
    append_compact(&mut owner, 3, b, a);
    owner.sync().unwrap();
    drop(owner);
    let (collision_raw, collision) = last_graph_evidence(&right_path);
    assert_eq!(
        left_raw, collision_raw,
        "identical wire ordinals can name different stable edges"
    );
    assert_ne!(
        left, collision,
        "import authority must bind stable endpoints"
    );
}
