use super::dream_proof_state;
use crate::{Cva, MemoryDraft, MemoryId};
use std::{collections::BTreeSet, fs, path::PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("dream-proof-history-{}", uuid::Uuid::new_v4()));
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

fn publish(cva: &mut Cva, name: &str) -> MemoryId {
    let draft = MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: name.into(),
        content: format!("Dream proof fixture: {name}"),
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

#[test]
fn pending_accepted_and_completed_proofs_are_deterministic_and_survive_reopen() {
    let fixture = Fixture::new();
    let path = fixture.path("proof-lifecycle.rel");
    let mut cva = Cva::create(&path).unwrap();
    let root = publish(&mut cva, "proof-root");
    let source = publish(&mut cva, "proof-source");
    let pass_id = crate::dream_pass_identity::initial_pass_id(cva.owner_uuid().unwrap(), source);

    cva.begin_initial_dream_pass(
        pass_id.clone(),
        source,
        1,
        cva.rel_turn_count(),
        BTreeSet::from([root]),
        cva.graph_version(),
    )
    .unwrap();
    let pending = dream_proof_state(&cva).unwrap();
    assert_eq!(pending, dream_proof_state(&cva).unwrap());

    cva.set_memory_relation_with_origin(
        source,
        root,
        crate::GraphRelationKind::Topical,
        true,
        crate::GraphRelationOrigin::Dream,
        cva.graph_version(),
    )
    .unwrap();
    cva.record_initial_dream_roots(&pass_id, BTreeSet::from([root]))
        .unwrap();
    cva.accept_initial_dream_settlement(&pass_id, cva.rel_turn_count())
        .unwrap();
    let accepted = dream_proof_state(&cva).unwrap();
    assert_ne!(
        pending, accepted,
        "accepted graph cut must enter the digest"
    );
    assert_eq!(accepted, dream_proof_state(&cva).unwrap());

    crate::dream_freshness::settle_initial_pass(&mut cva, source, false, 1).unwrap();
    let completed = dream_proof_state(&cva).unwrap();
    assert_ne!(
        accepted, completed,
        "retained completion proof must enter the digest"
    );
    cva.sync().unwrap();
    drop(cva);

    let reopened = Cva::open(&path).unwrap();
    assert_eq!(completed, dream_proof_state(&reopened).unwrap());
    assert_eq!(completed, dream_proof_state(&reopened).unwrap());
    assert_eq!(
        reopened
            .historical_completed_initial_dream_passes()
            .unwrap()[&source]
            .roots,
        BTreeSet::from([root]),
        "accepted roots are retained as proof, not inferred from a marker"
    );
}

#[test]
fn verified_empty_roots_are_not_a_bare_replay_settled_marker() {
    let fixture = Fixture::new();
    let path = fixture.path("empty-vs-marker.rel");
    let mut cva = Cva::create(&path).unwrap();
    let source = publish(&mut cva, "verified-empty-source");

    // This marker has no pending pass, accepted graph cut, or root evidence.
    // It must not be upgraded into a completed empty result by the oracle.
    cva.freshness_replay_settled_dream_pass("legacy-settled-without-proof")
        .unwrap();
    let marker_only = dream_proof_state(&cva).unwrap();
    assert!(
        !cva.historical_completed_initial_dream_passes()
            .unwrap()
            .contains_key(&source)
    );

    let pass_id = crate::dream_pass_identity::initial_pass_id(cva.owner_uuid().unwrap(), source);
    cva.begin_initial_dream_pass(
        pass_id.clone(),
        source,
        1,
        cva.rel_turn_count(),
        BTreeSet::new(),
        cva.graph_version(),
    )
    .unwrap();
    cva.record_initial_dream_roots(&pass_id, BTreeSet::new())
        .unwrap();
    cva.accept_initial_dream_settlement(&pass_id, cva.rel_turn_count())
        .unwrap();
    crate::dream_freshness::settle_initial_pass(&mut cva, source, false, 1).unwrap();

    let verified_empty = dream_proof_state(&cva).unwrap();
    assert_ne!(marker_only, verified_empty);
    assert!(
        cva.historical_completed_initial_dream_passes()
            .unwrap()
            .contains_key(&source)
    );
}

#[test]
fn nonempty_roots_are_retained_in_the_completed_proof() {
    let fixture = Fixture::new();
    let path = fixture.path("nonempty-roots.rel");
    let mut cva = Cva::create(&path).unwrap();
    let root = publish(&mut cva, "nonempty-root");
    let source = publish(&mut cva, "nonempty-source");
    let pass_id = crate::dream_pass_identity::initial_pass_id(cva.owner_uuid().unwrap(), source);

    cva.begin_initial_dream_pass(
        pass_id.clone(),
        source,
        1,
        cva.rel_turn_count(),
        BTreeSet::from([root]),
        cva.graph_version(),
    )
    .unwrap();
    cva.set_memory_relation_with_origin(
        source,
        root,
        crate::GraphRelationKind::Topical,
        true,
        crate::GraphRelationOrigin::Dream,
        cva.graph_version(),
    )
    .unwrap();
    cva.record_initial_dream_roots(&pass_id, BTreeSet::from([root]))
        .unwrap();
    cva.accept_initial_dream_settlement(&pass_id, cva.rel_turn_count())
        .unwrap();
    crate::dream_freshness::settle_initial_pass(&mut cva, source, false, 1).unwrap();

    let completed = dream_proof_state(&cva).unwrap();
    assert_eq!(
        cva.historical_completed_initial_dream_passes().unwrap()[&source].roots,
        BTreeSet::from([root])
    );
    cva.sync().unwrap();
    drop(cva);
    let reopened = Cva::open(&path).unwrap();
    assert_eq!(completed, dream_proof_state(&reopened).unwrap());
    assert_eq!(
        reopened
            .historical_completed_initial_dream_passes()
            .unwrap()[&source]
            .roots,
        BTreeSet::from([root])
    );
}

#[test]
#[cfg(feature = "history-oracle-fixtures")]
fn authoritative_rel_fixture_dream_proofs_repeat_and_reopen() {
    let root = PathBuf::from(std::env::var_os("RELIQUARY_HISTORY_FIXTURES").expect("fixture root"));
    let fixture = Fixture::new();
    for (index, corpus) in [
        "chatgpt-first14d",
        "chatgpt-first28d",
        "rhelm-david-r-ellis-pre-relationship",
    ]
    .iter()
    .enumerate()
    {
        let path = fixture.path(&format!("{index}.rel"));
        let mut source = fs::File::open(root.join(corpus).join("project.prj.rel")).unwrap();
        let mut destination = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        std::io::copy(&mut source, &mut destination).unwrap();
        drop(destination);

        let rel = Cva::open(&path).unwrap();
        let first = dream_proof_state(&rel).unwrap();
        let second = dream_proof_state(&rel).unwrap();
        let proofs = rel.historical_completed_initial_dream_passes().unwrap();
        assert_eq!(first, second, "{corpus} repeated read");
        drop(rel);

        let reopened = Cva::open(&path).unwrap();
        assert_eq!(
            first,
            dream_proof_state(&reopened).unwrap(),
            "{corpus} reopen"
        );
        assert_eq!(
            proofs,
            reopened
                .historical_completed_initial_dream_passes()
                .unwrap(),
            "{corpus} completed proof API parity"
        );
    }
}
