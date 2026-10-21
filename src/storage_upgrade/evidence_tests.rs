use super::*;
use crate::{
    EchoEvent, EchoEventKind, ProjectRepositoryKind, ProjectRepositoryRef, ProjectRevisionRef,
};

fn legacy_header(path: &Path) -> Vec<u8> {
    let mut bytes = fs::read(path).unwrap();
    bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
    fs::write(path, &bytes).unwrap();
    bytes
}

#[test]
fn latest_upgrade_preserves_every_echo_and_project_correlation_field() {
    let fixture = Fixture::new();
    let request = fixture.request();
    let mut owner = Cva::create_project(&request.source).unwrap();
    for (sequence, kind) in [
        (2, EchoEventKind::ReasoningTrace),
        (0, EchoEventKind::ReasoningSummary),
        (1, EchoEventKind::Commentary),
    ] {
        owner
            .put_echo_event(EchoEvent {
                conversation_id: "conversation-é".into(),
                message_id: "assistant-1".into(),
                sequence,
                timestamp_ns: 123 + sequence as i64,
                model_round: Some(9),
                kind,
                correlation_id: Some("correlation".into()),
                name: Some("accepted name".into()),
                content: format!("literal evidence {sequence} é"),
            })
            .unwrap();
    }
    let first = crate::ProjectRevisionCorrelation {
        sequence: 1,
        rel_cut: crate::RelSemanticCut {
            global_version: 0,
            archive_version: 0,
            memory_version: 0,
        },
        project_revision: ProjectRevisionRef {
            repository: ProjectRepositoryRef {
                kind: ProjectRepositoryKind::Lore,
                repository_id: "repo-1".into(),
                project_path: ".".into(),
            },
            revision: "revision-1".into(),
        },
        repository_management: crate::ProjectRepositoryManagement::WarlockManaged,
    };
    let legacy = crate::project_history_codec::encode_legacy_v1(&first).unwrap();
    owner.container.append(&legacy).unwrap();
    owner.project_history.ingest(&legacy).unwrap();
    for revision in ["revision-2"] {
        owner
            .correlate_project_revision(ProjectRevisionRef {
                repository: ProjectRepositoryRef {
                    kind: ProjectRepositoryKind::Lore,
                    repository_id: "repo-1".into(),
                    project_path: ".".into(),
                },
                revision: revision.into(),
            })
            .unwrap();
    }
    let echoes = owner.echo_events("conversation-é", "assistant-1");
    let correlations = owner.project_revision_correlations();
    owner.sync().unwrap();
    drop(owner);
    let bytes = legacy_header(&request.source);
    let source = Cva::open_v1_read_only(&request.source).unwrap();
    assert_eq!(source.echo_events("conversation-é", "assistant-1"), echoes);
    assert_eq!(source.project_revision_correlations(), correlations);
    drop(source);
    let plan = plan_storage_upgrade(request.clone()).unwrap();
    let stage = stage_graph_envelope_storage_upgrade(&plan).unwrap();
    let output = Cva::open(stage.test_path()).unwrap();
    assert_eq!(output.echo_events("conversation-é", "assistant-1"), echoes);
    assert_eq!(output.project_revision_correlations(), correlations);
    assert_eq!(output.container.latest_version(), 0);
    let mut scan = Container::open(stage.test_path()).unwrap();
    let mut count = 0;
    scan.visit_payloads::<StorageUpgradeError>(|_, payload| {
        assert!(!payload.starts_with(b"PRJCOR01"));
        if payload.starts_with(b"PRJCOR02") {
            count += 1;
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(count, 2);
    assert_eq!(fs::read(&request.source).unwrap(), bytes);
    assert!(!request.destination.exists());
}

#[test]
fn upgrade_unknown_authority_fails_before_output_without_source_changes() {
    for identity in [*b"CVAFUT99", *b"PRJFUT99", *b"CVDNEW99", *b"CVAERS04"] {
        for phy in [false, true] {
            let fixture = Fixture::new();
            let request = fixture.request();
            if phy {
                let mut owner = Phylactery::create(&request.source).unwrap();
                owner.container.append(&identity).unwrap();
                owner.sync().unwrap();
            } else {
                let mut owner = Cva::create(&request.source).unwrap();
                owner.container.append(&identity).unwrap();
                owner.sync().unwrap();
            }
            let bytes = legacy_header(&request.source);
            assert!(
                matches!(plan_storage_upgrade(request.clone()),Err(StorageUpgradeError::UnknownAuthority(actual)) if actual==identity)
            );
            assert_eq!(fs::read(&request.source).unwrap(), bytes);
            assert!(!request.destination.exists());
            fixture.assert_no_partial();
        }
    }
}
