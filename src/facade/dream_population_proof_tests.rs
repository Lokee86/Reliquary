use crate::{Cva, MemoryDraft, MemoryId};
use std::{collections::BTreeSet, fs, path::PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("dream-population-parity-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProofSnapshot {
    fingerprint: crate::historical_fingerprint::HistoricalFingerprint,
    completed: std::collections::BTreeMap<MemoryId, crate::InitialDreamPass>,
}

fn snapshot(cva: &Cva) -> ProofSnapshot {
    ProofSnapshot {
        fingerprint: crate::historical_dream_proof_fingerprint::dream_proof_state(cva).unwrap(),
        completed: cva.historical_completed_initial_dream_passes().unwrap(),
    }
}

fn publish(cva: &mut Cva, name: &str) -> MemoryId {
    let draft = MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: name.into(),
        content: format!("Dream population parity fixture: {name}"),
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
        source_time_ns: Some(0),
        mutation_id: name.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    };
    let (memory, created) = cva.publish_memory(None, 0, draft).unwrap();
    assert!(created);
    memory.id
}

fn begin_current(
    path: &PathBuf,
    owner: [u8; 16],
    pass_id: &str,
    source: MemoryId,
    turn: u64,
    graph_version_before: u64,
    ids: BTreeSet<MemoryId>,
    compact: bool,
) -> Cva {
    let mut cva = Cva::open(path).unwrap();
    let population = if compact {
        crate::dream_begin_codec::BeginPopulation::CompactCut {
            memory_cut: cva.memories.memory_version(),
            digest: crate::dream_population_digest::digest(&ids),
        }
    } else {
        crate::dream_begin_codec::BeginPopulation::ExplicitSet(ids)
    };
    let payload = crate::dream_begin_codec::encode(&crate::dream_begin_codec::BeginRecord {
        owner_uuid: owner,
        pass_id: pass_id.to_owned(),
        source_id: source,
        turn,
        graph_version_before,
        population,
    })
    .unwrap();
    cva.container.append(&payload).unwrap();
    cva.sync().unwrap();
    drop(cva);
    Cva::open(path).unwrap()
}

#[test]
fn literal_compact_and_explicit_begin_have_identical_historical_proofs_at_each_stage() {
    let fixture = Fixture::new();
    let base_path = fixture.path("base.rel");
    let mut base = Cva::create(&base_path).unwrap();
    let root = publish(&mut base, "parity-root");
    let source = publish(&mut base, "parity-source");
    base.sync().unwrap();
    let owner = base.owner_uuid().unwrap();
    let turn = base.rel_turn_count();
    let graph_version_before = base.graph_version();
    drop(base);

    let literal_path = fixture.path("literal.rel");
    let compact_path = fixture.path("compact.rel");
    let explicit_path = fixture.path("explicit.rel");
    fs::copy(&base_path, &literal_path).unwrap();
    fs::copy(&base_path, &compact_path).unwrap();
    fs::copy(&base_path, &explicit_path).unwrap();

    let pass_id = crate::dream_pass_identity::initial_pass_id(owner, source);
    let initial_roots = BTreeSet::from([root]);
    let mut literal = Cva::open(&literal_path).unwrap();
    let literal_pass = crate::InitialDreamPass {
        pass_id: pass_id.clone(),
        source_id: source,
        first_revision: 1,
        turn,
        accepted_settlement_turn: None,
        preexisting_ids: initial_roots.clone(),
        graph_version_before,
        roots: BTreeSet::new(),
    };
    let literal_payload = crate::dream_current_fixture::explicit(owner, &literal_pass);
    literal.container.append(&literal_payload).unwrap();
    literal
        .dream_freshness
        .ingest_materialized_begin(owner, literal_pass.clone())
        .unwrap();
    let mut compact = begin_current(
        &compact_path,
        owner,
        &pass_id,
        source,
        turn,
        graph_version_before,
        initial_roots.clone(),
        true,
    );
    let mut explicit = begin_current(
        &explicit_path,
        owner,
        &pass_id,
        source,
        turn,
        graph_version_before,
        initial_roots.clone(),
        false,
    );

    let mut literal_stages = vec![snapshot(&literal)];
    let mut compact_stages = vec![snapshot(&compact)];
    let mut explicit_stages = vec![snapshot(&explicit)];

    for cva in [&mut literal, &mut compact, &mut explicit] {
        cva.set_memory_relation_with_origin(
            source,
            root,
            crate::GraphRelationKind::Topical,
            true,
            crate::GraphRelationOrigin::Dream,
            cva.graph_version(),
        )
        .unwrap();
        cva.record_initial_dream_roots(&pass_id, initial_roots.clone())
            .unwrap();
    }
    literal_stages.push(snapshot(&literal));
    compact_stages.push(snapshot(&compact));
    explicit_stages.push(snapshot(&explicit));

    for cva in [&mut literal, &mut compact, &mut explicit] {
        cva.accept_initial_dream_settlement(&pass_id, turn).unwrap();
    }
    literal_stages.push(snapshot(&literal));
    compact_stages.push(snapshot(&compact));
    explicit_stages.push(snapshot(&explicit));

    for cva in [&mut literal, &mut compact, &mut explicit] {
        crate::dream_freshness::settle_initial_pass(cva, source, false, 1).unwrap();
        cva.sync().unwrap();
    }
    literal_stages.push(snapshot(&literal));
    compact_stages.push(snapshot(&compact));
    explicit_stages.push(snapshot(&explicit));

    assert_eq!(literal_stages, compact_stages, "CompactCut parity by stage");
    assert_eq!(
        literal_stages, explicit_stages,
        "ExplicitSet parity by stage"
    );
}
