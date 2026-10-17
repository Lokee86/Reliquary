use super::*;
#[test]
fn historical_matrix_native_missing_provenance_preserves_unsupported_and_isolated_admission() {
    let (path, mut cva, _) = historical_import_fixture("c9-provenance.rel");
    let isolated = publish(&mut cva, "C9 isolated", Some(("c1", "u0")), true);
    let unsupported = publish(&mut cva, "C9 unsupported", None, true);
    let untouched = cva.freshness_record(unsupported);
    let reopened = accept_and_reopen(&path, cva);
    assert_eq!(reopened.freshness_record(unsupported), untouched);
    assert_eq!(
        reopened
            .freshness_record(isolated)
            .unwrap()
            .admitted_at_turn,
        Some(1)
    );
    assert_eq!(reopened.freshness_record(isolated).unwrap().score, 97);
}
#[test]
fn historical_matrix_native_foreign_and_missing_declared_provenance_reject_retained_owner_records()
{
    for foreign in [false, true] {
        let (path, mut cva, id) = historical_import_fixture(&format!("c9-declared-{foreign}.rel"));
        let current = cva.memory(id).unwrap();
        let mut changed = draft(current.clone(), "c9-declared-conflict");
        changed.source_node_id = Some("u0".into());
        let mut source = crate::MemorySourceRef {
            owner_id: cva.owner_id().unwrap(),
            principal_id: None,
            source_episode_id: current.source_episode_id.unwrap(),
            source_node_id: changed.source_node_id.clone().unwrap(),
            content_source_conversation_id: current.content_source_conversation_id.clone(),
            content_source_node_id: current.content_source_node_id.clone(),
            grounding_source_conversation_id: current.grounding_source_conversation_id.clone(),
            grounding_source_node_id: current.grounding_source_node_id.clone(),
        };
        if foreign {
            source.owner_id = "rel-ffffffff-ffff-4fff-8fff-ffffffffffff".into();
        } else {
            changed.content_source_conversation_id = Some("unavailable-owner".into());
            changed.content_source_node_id = Some("missing".into());
            source.content_source_conversation_id = changed.content_source_conversation_id.clone();
            source.content_source_node_id = changed.content_source_node_id.clone();
        }
        // An actual immutable Memory revision injects incompatible retained
        // provenance below the facade, not an authored manifest or score plan.
        cva.memories
            .publish_with_source_ref(
                &mut cva.container,
                Some(id),
                current.revision,
                changed,
                Some(source),
            )
            .unwrap();
        rejected(
            &mut cva,
            &path,
            if foreign {
                "foreign"
            } else {
                "declared import source"
            },
        );
    }
}
#[test]
fn historical_matrix_native_foreign_graph_endpoint_and_body_mutation_reject_before_writes() {
    let (path, mut cva, id) = historical_import_fixture("c9-owner-guards.rel");
    cva.sync().unwrap();
    let bytes = fs::read(&path).unwrap();
    let body = cva.memory_body_id(id).unwrap();
    assert!(
        cva.set_memory_relation(
            id,
            MemoryId([255; 32]),
            GraphRelationKind::Factual,
            true,
            cva.graph_version()
        )
        .is_err()
    );
    let memory = cva.memory(id).unwrap();
    let mut changed = draft(memory.clone(), "c9-forbidden-body");
    changed
        .content
        .push_str("Unauthorized semantic replacement.");
    assert!(matches!(
        cva.publish_memory(Some(id), memory.revision, changed),
        Err(crate::MemoryError::SemanticMutation)
    ));
    assert_eq!(cva.memory_body_id(id).unwrap(), body);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    accept_and_reopen(&path, cva);
}
#[test]
fn historical_matrix_native_graph_vector_and_revision_drift_rejects_then_refreezes() {
    for mode in 0..3 {
        let (path, mut cva, id) = historical_import_fixture(&format!("c9-drift-{mode}.rel"));
        let other = publish(&mut cva, "C9 drift peer", Some(("c1", "u0")), true);
        let prepared = cva
            .freeze_historical_import()
            .unwrap()
            .prepare_native()
            .unwrap();
        match mode {
            0 => edge(
                &mut cva,
                id,
                other,
                GraphRelationKind::Factual,
                GraphRelationOrigin::User,
            ),
            1 => {
                vectors(&mut cva, false);
            }
            _ => {
                let revision = cva.memory(id).unwrap().revision;
                cva.set_memory_temporal_status(
                    id,
                    revision,
                    "historical",
                    "c9-revision-drift".into(),
                )
                .unwrap();
            }
        }
        cva.sync().unwrap();
        let bytes = fs::read(&path).unwrap();
        let records = cva.freshness_records_at(32).unwrap();
        assert!(
            cva.commit_historical_import(&prepared)
                .unwrap_err()
                .to_string()
                .contains("ImportInputDrift")
        );
        assert!(cva.freshness.import_checkpoint().is_none());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(cva.freshness_records_at(32).unwrap(), records);
        accept_and_reopen(&path, cva);
    }
}
#[test]
fn historical_matrix_native_uncovered_source_and_preexisting_use_are_not_superseded() {
    for clock in [false, true] {
        let (path, mut cva, id) =
            historical_import_fixture(&format!("c9-clock-or-use-{clock}.rel"));
        if clock {
            cva.append_node(
                "earlier".into(),
                "other".into(),
                None,
                "user".into(),
                -100,
                "Misordered historical source.",
            )
            .unwrap();
        } else {
            cva.record_accepted_memory_use(
                crate::AcceptedMemoryUseReceipt {
                    owner_uuid: cva.owner_uuid().unwrap(),
                    accepted_turn: 32,
                    use_id: "c9-genuine-use".into(),
                    memories: vec![id],
                },
                400,
            )
            .unwrap();
        }
        rejected(
            &mut cva,
            &path,
            if clock {
                "unprocessed Archive tail/branch"
            } else {
                "accepted use"
            },
        );
    }
}
#[test]
fn historical_matrix_native_malformed_retained_community_fails_reopen_without_rewriting_acceptance()
{
    let (path, mut cva, id) = historical_import_fixture("c9-malformed-community.rel");
    let peer = publish(&mut cva, "C9 Community peer", Some(("c1", "u0")), true);
    edge(
        &mut cva,
        id,
        peer,
        GraphRelationKind::Factual,
        GraphRelationOrigin::User,
    );
    let snapshot = cva.refresh_communities_leiden().unwrap();
    let mut cva = accept_and_reopen(&path, cva);
    let original = fs::read(&path).unwrap();
    let accepted = cva.freshness_record(id);
    for foreign in [false, true] {
        let mut malformed = snapshot.clone();
        malformed.generation += 1;
        if foreign {
            malformed.communities[0].members[0] = MemoryId([255; 32]);
        } else {
            let member = malformed.communities[0].members[0];
            malformed.communities[0].members.push(member);
        }
        let candidate = path.with_file_name(format!("invalid-community-{foreign}.rel"));
        let payload = crate::community_codec::encode_snapshot(&malformed).unwrap();
        let mut bytes = original.clone();
        bytes.extend((payload.len() as u64).to_le_bytes());
        bytes.extend(payload);
        fs::write(&candidate, &bytes).unwrap();
        assert!(Cva::open(&candidate).is_err());
        assert_eq!(fs::read(&candidate).unwrap(), bytes);
    }
    assert_eq!(cva.freshness_record(id), accepted);
    assert!(
        !cva.reconstruct_historical_freshness(Default::default())
            .unwrap()
            .committed
    );
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn historical_matrix_native_cross_boundary_provisional_event_rejects_without_projection() {
    let (path, mut cva, root) = historical_import_fixture("c9-event-isolation.rel");
    let bridge = publish(&mut cva, "C9 untouched bridge", None, true);
    edge(
        &mut cva,
        root,
        bridge,
        GraphRelationKind::Factual,
        GraphRelationOrigin::User,
    );
    historical_events::linked_source(&mut cva, root, false);
    rejected(&mut cva, &path, "replacement/untouched boundary");
}
