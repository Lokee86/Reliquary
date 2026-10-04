use crate::runtime_host_test_support::{
    BlockingEmbeddingEndpoint, memory_endpoint, one_worker, queue_memory_episode, test_path,
    wait_memory, wait_memory_revisions, wait_vectors,
};
use crate::{
    Cva, EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingMode, EpisodePolicy, FragmentConfig,
    InteractionRole, InteractionRuntime, ReliquaryRuntimeHost, ReliquaryRuntimeRoutes,
    SimulatedEmbeddingEndpoint, VectorNormalization,
};
use std::sync::{Arc, Barrier};

#[test]
fn embedding_probe_does_not_hold_the_interaction_runtime_lock() {
    let cva = Cva::create(test_path("embedding-concurrent.cva")).unwrap();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let endpoint = Arc::new(BlockingEmbeddingEndpoint::new(
        Arc::clone(&entered),
        Arc::clone(&release),
    ));
    let host = ReliquaryRuntimeHost::start(
        InteractionRuntime::new(cva),
        ReliquaryRuntimeRoutes::new(None, None, None, None, Some(endpoint)),
        one_worker(),
        EpisodePolicy::default(),
    )
    .unwrap();

    entered.wait();
    host.open_session("live".into(), None).unwrap();
    host.begin_message("live", "u1".into(), InteractionRole::User, 1)
        .unwrap();
    host.append_text("live", "u1", "Embedding does not block chat.")
        .unwrap();
    host.complete_message("live", "u1").unwrap();
    release.wait();
    host.into_cva().unwrap();
}

#[test]
fn embedding_route_can_be_attached_after_memory_creation() {
    let mut cva = Cva::create(test_path("late-embedding.cva")).unwrap();
    queue_memory_episode(&mut cva);
    let host = ReliquaryRuntimeHost::start(
        InteractionRuntime::new(cva),
        ReliquaryRuntimeRoutes::new(Some(memory_endpoint()), None, None, None, None),
        one_worker(),
        EpisodePolicy::default(),
    )
    .unwrap();

    wait_memory(&host, 1);
    assert_eq!(host.memory_vector_stats().unwrap().bindings, 0);
    host.set_routes(ReliquaryRuntimeRoutes::new(
        Some(memory_endpoint()),
        None,
        None,
        None,
        Some(Arc::new(SimulatedEmbeddingEndpoint::new(
            8,
            VectorNormalization::L2,
            7,
        ))),
    ))
    .unwrap();
    wait_vectors(&host, 1);
    wait_memory_revisions(&host, 2);
    assert_eq!(host.memory_vector_stats().unwrap().bindings, 1);
    let mut cva = host.into_cva().unwrap();
    let id = cva.memory_ids()[0];
    assert_eq!(cva.memory(id).unwrap().lifecycle_state, "canonical");
}

#[test]
fn live_transcript_search_uses_embedding_route_and_branch_scope() {
    let mut cva = Cva::create(test_path("live-search-embedding.cva")).unwrap();
    append_search_node(&mut cva, "root", None, "shared anchor");
    append_search_node(&mut cva, "left-1", Some("root"), "left ordinary");
    append_search_node(
        &mut cva,
        "left-2",
        Some("left-1"),
        "meaning-target selected branch",
    );
    append_search_node(
        &mut cva,
        "right-1",
        Some("root"),
        "meaning-target sibling branch",
    );
    append_search_node(
        &mut cva,
        "right-2",
        Some("right-1"),
        "meaning-target sibling branch again",
    );
    cva.materialize_path_fragments("live", "left-2", FragmentConfig::default(), true)
        .unwrap();
    cva.materialize_path_fragments("live", "right-2", FragmentConfig::default(), true)
        .unwrap();
    let endpoint = LiveSearchEmbedding;
    let profile = cva.establish_compatibility_profile(&endpoint).unwrap();
    cva.build_archive_vector_generation(profile.id, &endpoint)
        .unwrap();
    let host = ReliquaryRuntimeHost::start(
        InteractionRuntime::new(cva),
        ReliquaryRuntimeRoutes::new(None, None, None, None, Some(Arc::new(endpoint))),
        one_worker(),
        EpisodePolicy::default(),
    )
    .unwrap();
    host.open_session("live".into(), Some("left-2".into()))
        .unwrap();

    let hits = host
        .search_open_session("live", "semantic-topic", 10)
        .unwrap();

    assert!(!hits.is_empty());
    assert!(hits.iter().all(|hit| !hit.text.contains("sibling")));
    assert!(hits.iter().any(|hit| hit.text.contains("selected branch")));
    host.into_cva().unwrap();
}

struct LiveSearchEmbedding;

impl EmbeddingEndpoint for LiveSearchEmbedding {
    fn dimensions(&self) -> u32 {
        2
    }

    fn normalization(&self) -> VectorNormalization {
        VectorNormalization::L2
    }

    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        Ok(inputs
            .iter()
            .map(|text| match mode {
                EmbeddingMode::Query if text.contains("semantic-topic") => vec![1.0, 0.0],
                EmbeddingMode::Document if text.contains("meaning-target") => vec![1.0, 0.0],
                _ => vec![0.0, 1.0],
            })
            .collect())
    }
}

fn append_search_node(cva: &mut Cva, id: &str, parent: Option<&str>, text: &str) {
    cva.append_node(
        id.into(),
        "live".into(),
        parent.map(str::to_owned),
        "user".into(),
        cva.archive_version() as i64 + 1,
        text,
    )
    .unwrap();
}
