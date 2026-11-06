//! Physical offset relocation and retained vector closure for Archive pruning.
use super::*;
use crate::ArchiveVectorRow;
use reliquary_archive::{Branch, TurnInput};
use reliquary_container::ReliquaryScope;
use reliquary_vector_store::{
    CompatibilityProbeReference, CompatibilityProfile, EmbeddingMode, VectorNormalization,
};
use std::path::PathBuf;

fn profile() -> CompatibilityProfile {
    let mut profile = CompatibilityProfile {
        id: reliquary_vector_store::CompatibilityProfileId([0; 32]),
        dimensions: 2,
        normalization: VectorNormalization::L2,
        probe_suite_version: 1,
        compatibility_policy_version: 3,
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
    profile.id = reliquary_vector_store::profile::compatibility_profile_id(&profile);
    profile
}
struct Fixture(PathBuf);
impl Fixture {
    fn new(label: &str) -> Self {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&directory).unwrap();
        Self(directory.join(format!(
            "archive-vector-prune-{}-{}-{label}.rel",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        )))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
fn terminal_archive_prune_relocates_retained_generation_receipts() {
    let source = Fixture::new("source");
    let output = Fixture::new("output");
    let owner = [192; 16];
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = RelWriter::create_path(&source.0, ReliquaryScope::Project, owner).unwrap();

    // A terminal conversation with no branch has no fragment bindings.
    writer
        .session
        .archive
        .accept_turn(
            &mut writer.container,
            TurnInput {
                id: "terminal-node".into(),
                conversation_id: "terminal".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: None,
                timestamp_ns: 1,
                content: "terminal transcript ".repeat(400),
                attachments: Vec::new(),
                project_attachments: Vec::new(),
            },
        )
        .unwrap();
    writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    writer
        .accept_turn_and_advance_branch(
            TurnInput {
                id: "active-node".into(),
                conversation_id: "active".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: None,
                timestamp_ns: 2,
                content: "retained source".into(),
                attachments: Vec::new(),
                project_attachments: Vec::new(),
            },
            Branch {
                id: "main".into(),
                conversation_id: "active".into(),
                leaf_node_id: "active-node".into(),
                canonical: true,
            },
        )
        .unwrap();
    let profile = writer.publish_compatibility_profile(profile()).unwrap();
    let input = writer.archive_vector_input(profile).unwrap();
    assert_eq!(input.sources().len(), 1);
    let rows = input
        .sources()
        .iter()
        .map(|source| ArchiveVectorRow {
            fragment_id: source.fragment_id,
            max_archive_version: source.max_archive_version,
            vector: vec![1.0, 0.0],
        })
        .collect::<Vec<_>>();
    let generation = writer
        .publish_archive_vector_batch(owner, profile, input.source_archive_version(), &rows)
        .unwrap()
        .unwrap();
    writer.tombstone_conversation("terminal").unwrap();
    drop(writer);

    let before = fs::read(&source.0).unwrap();
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let report = prune_terminal_archive_copy(
        &plan,
        &source.0,
        &output.0,
        &["terminal".into()],
        &admission,
    )
    .unwrap();
    assert_eq!(report.pruned_turns, 1);
    assert_eq!(fs::read(&source.0).unwrap(), before);
    let reopened = RelSession::open_path(&output.0, &admission, [], []).unwrap();
    assert_eq!(
        reopened.generations.current_generation(profile),
        Some(generation)
    );
    let binding = reopened
        .archive_vectors
        .get(generation.archive_vector_id)
        .unwrap();
    assert_eq!(
        reopened
            .common
            .packed_vectors
            .read_f32_row(binding.packed_vector_id, 0)
            .unwrap(),
        vec![1.0, 0.0],
    );
    assert!(
        reopened
            .archive
            .resolved_turn("terminal", "terminal-node")
            .is_err()
    );
    assert!(
        reopened
            .archive
            .resolved_turn("active", "active-node")
            .is_ok()
    );
}
#[test]
fn terminal_archive_prune_refuses_retained_archive_vector_sources() {
    let source = Fixture::new("vector-source");
    let output = Fixture::new("vector-output");
    let owner = [193; 16];
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = RelWriter::create_path(&source.0, ReliquaryScope::Project, owner).unwrap();
    writer
        .accept_turn_and_advance_branch(
            TurnInput {
                id: "terminal-node".into(),
                conversation_id: "terminal".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: None,
                timestamp_ns: 1,
                content: "retained by a vector generation".repeat(100),
                attachments: vec![],
                project_attachments: vec![],
            },
            Branch {
                id: "main".into(),
                conversation_id: "terminal".into(),
                leaf_node_id: "terminal-node".into(),
                canonical: true,
            },
        )
        .unwrap();
    let profile = writer.publish_compatibility_profile(profile()).unwrap();
    let input = writer.archive_vector_input(profile).unwrap();
    let rows = input
        .sources()
        .iter()
        .map(|source| ArchiveVectorRow {
            fragment_id: source.fragment_id,
            max_archive_version: source.max_archive_version,
            vector: vec![1.0, 0.0],
        })
        .collect::<Vec<_>>();
    assert!(!rows.is_empty());
    writer
        .publish_archive_vector_batch(owner, profile, input.source_archive_version(), &rows)
        .unwrap();
    writer.tombstone_conversation("terminal").unwrap();
    drop(writer);
    let before = fs::read(&source.0).unwrap();
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let error = prune_terminal_archive_copy(
        &plan,
        &source.0,
        &output.0,
        &["terminal".into()],
        &admission,
    )
    .unwrap_err();
    assert!(error.contains("retained by Archive vectors"), "{error}");
    assert_eq!(fs::read(&source.0).unwrap(), before);
    assert!(!output.0.exists());
}
