use super::*;
use lodestone_packed::{PackedVectors, ScalarType, VectorSchema};
use reliquary_vector_store::{
    AcceptedMemoryMapping, CompatibilityProbeReference, CompatibilityProfile,
    CompatibilityProfileId, CompatibilityProfileStore, EmbeddingMode, MemoryVectorStore,
    PackedVectorStore, VectorNormalization,
};
fn profile() -> CompatibilityProfile {
    let mut p = CompatibilityProfile {
        id: CompatibilityProfileId([0; 32]),
        dimensions: 2,
        normalization: VectorNormalization::L2,
        probe_suite_version: 1,
        compatibility_policy_version: 2,
        references: [
            EmbeddingMode::Query,
            EmbeddingMode::Query,
            EmbeddingMode::Document,
            EmbeddingMode::Document,
        ]
        .into_iter()
        .map(|mode| CompatibilityProbeReference {
            mode,
            vector: vec![1.0, 0.0],
        })
        .collect(),
    };
    p.id = reliquary_vector_store::profile::compatibility_profile_id(&p);
    p
}
#[test]
fn candidate_query_hydrates_at_most_129_bodies_and_rows_in_a_1025_memory_owner() {
    let path = Temp::new();
    let owner = [221; 16];
    drop(PhyWriter::create_path(&path.0, owner).unwrap());
    let mut c = Container::open_writer(&path.0).unwrap();
    let mut memories = MemoryStore::open(&c).unwrap();
    let mut records = Vec::new();
    for n in 0..1025 {
        let mut draft = memory(&format!("indexed-{n}"));
        draft.source_time_ns = Some(n + 1);
        draft.content = format!("Rollout evidence item {n}");
        draft.title = if n == 0 || n == 1024 {
            "Rollout uncommonneedle".into()
        } else {
            format!("Rollout ordinary {n}")
        };
        records.push(
            memories
                .publish(&mut c, None, 0, draft)
                .unwrap()
                .revision
                .record,
        );
    }
    let values: Vec<f32> = (0..1025)
        .flat_map(|n| {
            if n == 0 || n == 1024 {
                [1.0, 0.0]
            } else {
                [0.0, 1.0]
            }
        })
        .collect();
    let matrix = PackedVectors::from_bytes(
        VectorSchema::new(2, ScalarType::F32).unwrap(),
        values.into_iter().flat_map(f32::to_le_bytes).collect(),
    )
    .unwrap();
    let mut packed = PackedVectorStore::open(&c).unwrap();
    let packed_id = packed.put(&mut c, matrix).unwrap();
    let mut profiles = CompatibilityProfileStore::open(&c).unwrap();
    let profile = profile();
    profiles.put(&mut c, profile.clone()).unwrap();
    let mut vectors = MemoryVectorStore::open(&c, &packed, &profiles).unwrap();
    vectors
        .put(
            &mut c,
            &packed,
            &profiles,
            AcceptedMemoryMapping {
                profile: profile.id,
                packed: packed_id,
                packed_rows: 1025,
                body_ids: records
                    .iter()
                    .map(|r| reliquary_vector_store::MemoryBodyId(r.body_id.0))
                    .collect(),
            },
        )
        .unwrap();
    c.sync().unwrap();
    drop(c);
    let before = fs::read(&path.0).unwrap();
    let session = PhySession::open_path(&path.0, &graph(owner), []).unwrap();
    let (ids, report) = ranked_owner_dream_candidates_with_report(
        &EntityOwnerSession::Phy(&session),
        owner,
        records[1024].id,
        profile.id,
        reliquary_dream::DreamCandidateConfig::default(),
    )
    .unwrap();
    assert!(ids.contains(&records[0].id));
    assert!(ids.len() <= 16);
    assert!(report.identities_examined <= 128);
    assert!(report.body_reads <= 129);
    assert!(report.vector_rows <= 129);
    assert!(report.index_entries_examined <= 928);
    assert!(report.body_reads < 1025 / 4);
    drop(session);
    assert_eq!(before, fs::read(&path.0).unwrap());
}
