use super::*;
use crate::{FragmentConfig, SimulatedEmbeddingEndpoint, VectorNormalization};

struct Fixture {
    path: std::path::PathBuf,
    cva: Option<Cva>,
}
impl Fixture {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("semantic-source-{}.rel", uuid::Uuid::new_v4()));
        Self {
            cva: Some(Cva::create(&path).unwrap()),
            path,
        }
    }
    fn cva(&mut self) -> &mut Cva {
        self.cva.as_mut().unwrap()
    }
    fn turn(&mut self, id: &str, parent: Option<&str>, role: &str) -> Node {
        self.cva()
            .append_node(
                id.into(),
                "c".into(),
                parent.map(str::to_owned),
                role.into(),
                0,
                "genuine accepted content",
            )
            .unwrap()
    }
    fn generation(&mut self) -> VectorGeneration {
        let endpoint = SimulatedEmbeddingEndpoint::new(12, VectorNormalization::L2, 123);
        let profile = self
            .cva()
            .establish_compatibility_profile(&endpoint)
            .unwrap();
        self.cva()
            .build_archive_vector_generation(profile.id, &endpoint)
            .unwrap()
    }
    fn fragments(&mut self, leaf: &str, turns: usize) {
        self.cva()
            .materialize_path_fragments("c", leaf, FragmentConfig { turns, overlap: 0 }, true)
            .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        drop(self.cva.take());
        std::fs::remove_file(&self.path).unwrap();
    }
}
fn ready(outcome: SemanticSourceReadiness) -> AcceptedSemanticSource {
    match outcome {
        SemanticSourceReadiness::Ready(source) => source,
        _ => panic!("not ready"),
    }
}

#[test]
fn pending_is_not_empty_completion_and_non_turns_are_rejected() {
    let mut f = Fixture::new();
    let node = f.turn("n", None, "user");
    assert_eq!(
        f.cva()
            .prepare_accepted_semantic_source(&node, None)
            .unwrap(),
        SemanticSourceReadiness::Pending
    );
    let system = f.turn("sys", Some("n"), "system");
    assert!(
        f.cva()
            .prepare_accepted_semantic_source(&system, None)
            .is_err()
    );
    let mut conflict = node.clone();
    conflict.content_id.0[0] ^= 1;
    assert!(
        f.cva()
            .prepare_accepted_semantic_source(&conflict, None)
            .is_err()
    );
}

#[test]
fn exact_source_retries_and_reopen_preserve_selected_evidence_without_writes() {
    let mut f = Fixture::new();
    let node = f.turn("n", None, "assistant");
    f.fragments("n", 1);
    let generation = f.generation();
    let before = (
        f.cva().latest_global_version(),
        f.cva().archive_version(),
        f.cva().vector_version(),
    );
    let first = ready(
        f.cva()
            .prepare_accepted_semantic_source(&node, Some(generation.id))
            .unwrap(),
    );
    assert_eq!(first.activity_position, 1);
    assert_eq!(first.vectors.len(), 1);
    assert_eq!(first.owner_uuid, f.cva().owner_uuid().unwrap());
    assert_eq!(
        first.policy_digest,
        SemanticPolicy::default().digest().unwrap()
    );
    assert_eq!(
        before,
        (
            f.cva().latest_global_version(),
            f.cva().archive_version(),
            f.cva().vector_version()
        )
    );
    f.cva().sync().unwrap();
    drop(f.cva.take());
    f.cva = Some(Cva::open(&f.path).unwrap());
    let retry = ready(
        f.cva()
            .prepare_accepted_semantic_source(&node, Some(generation.id))
            .unwrap(),
    );
    assert_eq!(retry, first);
    f.turn("later", Some("n"), "user");
    assert_eq!(
        ready(
            f.cva()
                .prepare_accepted_semantic_source(&node, Some(generation.id))
                .unwrap()
        ),
        first
    );
}

#[test]
fn mixed_turn_generation_is_explicit_empty_not_a_genuine_turn_vector() {
    let mut f = Fixture::new();
    let first = f.turn("a", None, "user");
    f.turn("b", Some("a"), "assistant");
    f.fragments("b", 2);
    let generation = f.generation();
    let source = ready(
        f.cva()
            .prepare_accepted_semantic_source(&first, Some(generation.id))
            .unwrap(),
    );
    assert!(source.vectors.is_empty());
    assert_eq!(source.excluded_mixed_fragments, 1);
}

#[test]
fn stale_generation_cannot_claim_a_future_source_and_rotation_does_not_rebind() {
    let mut f = Fixture::new();
    let first = f.turn("a", None, "user");
    f.fragments("a", 1);
    let old = f.generation();
    let pinned = ready(
        f.cva()
            .prepare_accepted_semantic_source(&first, Some(old.id))
            .unwrap(),
    );
    let later = f.turn("b", Some("a"), "assistant");
    assert_eq!(
        f.cva()
            .prepare_accepted_semantic_source(&later, Some(old.id))
            .unwrap(),
        SemanticSourceReadiness::Pending
    );
    f.fragments("b", 1);
    f.generation();
    assert_eq!(
        ready(
            f.cva()
                .prepare_accepted_semantic_source(&first, Some(old.id))
                .unwrap()
        ),
        pinned
    );
}

#[test]
fn incomplete_mapping_stays_pending_even_with_one_valid_source_vector() {
    let mut f = Fixture::new();
    let node = f.turn("a", None, "user");
    f.turn("b", Some("a"), "assistant");
    f.fragments("b", 1);
    let old = f.generation();
    f.fragments("b", 2);
    // Existing generation is valid but does not cover the mixed fragment at this newer cut.
    let source_archive_version = f.cva().archive_version();
    let newer = f
        .cva()
        .publish_vector_generation(
            old.compatibility_profile_id,
            old.archive_vector_id,
            source_archive_version,
        )
        .unwrap();
    assert_eq!(
        f.cva()
            .prepare_accepted_semantic_source(&node, Some(newer.id))
            .unwrap(),
        SemanticSourceReadiness::Pending
    );
}
