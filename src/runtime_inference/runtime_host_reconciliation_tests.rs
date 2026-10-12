use crate::{
    Cva, CvaRelation, EmbeddingEndpoint, EpisodePolicy, FragmentConfig, GatewayRuntime,
    InsomniaWorkerConfig, ReliquaryRuntimeRoutes, SimulatedEmbeddingEndpoint, VectorNormalization,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

fn dir() -> PathBuf {
    let path = std::env::temp_dir().join(format!("reliquary-reconcile-{}", Uuid::new_v4()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn host(embedding: Option<Arc<dyn EmbeddingEndpoint + Send + Sync>>) -> GatewayRuntime {
    GatewayRuntime::new(
        dir(),
        ReliquaryRuntimeRoutes::new(None, None, None, None, embedding),
        InsomniaWorkerConfig::default(),
        EpisodePolicy::default(),
    )
}

fn create_workspace(path: &Path) {
    Cva::create_project(path).unwrap().sync().unwrap();
}

fn append(path: &Path, id: &str, content: &str) {
    let mut cva = Cva::open(path).unwrap();
    cva.append_node(
        id.into(),
        format!("conversation-{id}"),
        None,
        "user".into(),
        1,
        content,
    )
    .unwrap();
    cva.sync().unwrap();
}

fn seed_archive_vectors(path: &Path) {
    let mut cva = Cva::open(path).unwrap();
    for index in 0..8 {
        cva.append_node(
            format!("seed-{index}"),
            "seed-conversation".into(),
            (index > 0).then(|| format!("seed-{}", index - 1)),
            if index % 2 == 0 { "user" } else { "assistant" }.into(),
            index,
            "seed vector content",
        )
        .unwrap();
    }
    cva.materialize_path_fragments(
        "seed-conversation",
        "seed-7",
        FragmentConfig::default(),
        false,
    )
    .unwrap();
    let endpoint = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 41);
    let profile = cva.establish_compatibility_profile(&endpoint).unwrap();
    cva.build_archive_vector_generation(profile.id, &endpoint)
        .unwrap();
    cva.sync().unwrap();
}

#[test]
fn candidate_extension_is_discovered_and_fast_forwarded() {
    let root = dir();
    let canonical = root.join("project.prj.rel");
    let candidate = root.join("project (conflicted copy).prj.rel");
    create_workspace(&canonical);
    fs::copy(&canonical, &candidate).unwrap();
    append(&candidate, "right", "candidate change");

    let report = host(None).reconcile_rel_file(&canonical).unwrap();
    assert!(report.canonical_changed);
    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.candidates[0].status, "fast_forwarded");
    assert_eq!(
        Cva::compare(&canonical, &candidate).unwrap().relation,
        CvaRelation::Identical
    );
}

#[test]
fn divergent_candidate_is_semantically_merged_and_vectors_are_rebuilt() {
    let root = dir();
    let canonical = root.join("project.prj.rel");
    let candidate = root.join("project - laptop.prj.rel");
    create_workspace(&canonical);
    seed_archive_vectors(&canonical);
    fs::copy(&canonical, &candidate).unwrap();
    append(&canonical, "left", "canonical change");
    append(&candidate, "right", "candidate change");

    let endpoint: Arc<dyn EmbeddingEndpoint + Send + Sync> = Arc::new(
        SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 41),
    );
    let report = host(Some(endpoint)).reconcile_rel_file(&canonical).unwrap();

    assert!(report.canonical_changed);
    assert!(report.vector_rebuild_required);
    assert!(report.vector_rebuild_completed);
    assert_eq!(report.candidates[0].status, "merged");
    let reopened = Cva::open(&canonical).unwrap();
    assert!(reopened.archive_version() > 8);
    assert!(reopened.vector_generation_stats().generations > 0);
}

#[test]
fn retired_vectors_rebuild_on_a_later_reconciliation_when_embedding_becomes_available() {
    let root = dir();
    let canonical = root.join("project.prj.rel");
    let candidate = root.join("project - offline copy.prj.rel");
    create_workspace(&canonical);
    seed_archive_vectors(&canonical);
    fs::copy(&canonical, &candidate).unwrap();
    append(&canonical, "left", "canonical change");
    append(&candidate, "right", "candidate change");

    let pending = host(None).reconcile_rel_file(&canonical).unwrap();
    assert!(pending.canonical_changed);
    assert!(pending.vector_rebuild_required);
    assert!(!pending.vector_rebuild_completed);
    assert_eq!(
        Cva::open(&canonical)
            .unwrap()
            .vector_generation_stats()
            .generations,
        0
    );

    let endpoint: Arc<dyn EmbeddingEndpoint + Send + Sync> = Arc::new(
        SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 41),
    );
    let recovered = host(Some(endpoint)).reconcile_rel_file(&canonical).unwrap();
    assert!(recovered.vector_rebuild_required);
    assert!(recovered.vector_rebuild_completed);

    let reopened = Cva::open(&canonical).unwrap();
    assert!(reopened.vector_generation_stats().generations > 0);
}

#[test]
fn unrelated_workspace_is_ignored() {
    let root = dir();
    let canonical = root.join("project.prj.rel");
    let unrelated = root.join("other.prj.rel");
    create_workspace(&canonical);
    create_workspace(&unrelated);
    append(&unrelated, "other", "unrelated change");

    let before = fs::read(&canonical).unwrap();
    let report = host(None).reconcile_rel_file(&canonical).unwrap();
    assert!(report.candidates.is_empty());
    assert!(!report.canonical_changed);
    assert_eq!(fs::read(&canonical).unwrap(), before);
}

#[test]
fn semantic_conflict_is_reported_without_replacing_canonical() {
    let root = dir();
    let canonical = root.join("project.prj.rel");
    let candidate = root.join("project-conflict.prj.rel");
    create_workspace(&canonical);
    fs::copy(&canonical, &candidate).unwrap();
    append(&canonical, "same", "left content");
    append(&candidate, "same", "right content");
    let before = fs::read(&canonical).unwrap();

    let report = host(None).reconcile_rel_file(&canonical).unwrap();
    assert_eq!(report.candidates[0].status, "conflict");
    assert_eq!(
        report.candidates[0].conflict_kind.as_deref(),
        Some("source_turn")
    );
    assert_eq!(fs::read(&canonical).unwrap(), before);
}

#[test]
fn absorbed_candidate_is_a_byte_preserving_noop() {
    let root = dir();
    let canonical = root.join("project.prj.rel");
    let candidate = root.join("project-conflicted.prj.rel");
    create_workspace(&canonical);
    fs::copy(&canonical, &candidate).unwrap();
    append(&canonical, "left", "canonical change");
    append(&candidate, "right", "candidate change");

    host(None).reconcile_rel_file(&canonical).unwrap();
    let before = fs::read(&canonical).unwrap();
    let report = host(None).reconcile_rel_file(&canonical).unwrap();

    assert!(!report.canonical_changed);
    assert_eq!(report.candidates[0].status, "absorbed");
    assert_eq!(fs::read(&canonical).unwrap(), before);
}
