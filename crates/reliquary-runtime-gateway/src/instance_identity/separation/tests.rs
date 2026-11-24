use super::*;
use reliquary_application::create_direct_memory;
use reliquary_owner_session::{
    EntityMentionCommit,
    model::{
        EntityId, EntityResolutionReason, MemoryBodyId, MemoryEntityMentionKey,
        MemoryEntityResolutionStatus, MemoryTextField,
    },
};

fn seed_endpoint(path: &Path, owner: [u8; 16], name: &str) -> EntityId {
    let mut writer = RelWriter::create_current_path(path, owner).unwrap();
    let memory = create_direct_memory(
        &mut writer,
        "operator",
        name.into(),
        name.into(),
        "fact".into(),
        "project".into(),
        10,
    )
    .unwrap();
    let bytes = |hex: &str| {
        std::array::from_fn(|index| u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap())
    };
    let resolution = writer
        .commit_entity_mention(EntityMentionCommit {
            owner_id: owner,
            memory_revision: memory.revision,
            body_id: MemoryBodyId(bytes(&memory.body_id)),
            key: MemoryEntityMentionKey {
                memory_id: bytes(&memory.id),
                field: MemoryTextField::Content,
                start_byte: 0,
                end_byte: name.len() as u32,
            },
            surface: name.into(),
            expected_revision: 0,
            updated_at_ns: 11,
            status: MemoryEntityResolutionStatus::Unresolved {
                reason: EntityResolutionReason::NamedReferent,
            },
            target_revisions: vec![],
            new_entity: Some(("person".into(), "Authored referent".into())),
        })
        .unwrap();
    let MemoryEntityResolutionStatus::Resolved { entity_id, .. } = resolution.status else {
        panic!("fixture did not resolve the Entity");
    };
    entity_id
}

#[test]
fn expiration_and_action_conflict_preserve_only_the_durable_result() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.test-tmp")
        .join(format!("gateway-separation-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&path).unwrap();
    let local_path = path.join("local.rel");
    let peer_path = path.join("peer.rel");
    let pair = IdentityPair {
        local_owner: [10; 16],
        local_entity: seed_endpoint(&local_path, [10; 16], "Rene"),
        peer_owner: [20; 16],
        peer_entity: seed_endpoint(&peer_path, [20; 16], "Charlie"),
    };
    let gateway = Gateway::new();
    let local = gateway
        .mount_writable_reliquary_current(&local_path)
        .unwrap();
    let peer = gateway.mount_reliquary_current(&peer_path).unwrap();
    let instance = gateway.instance("operator", [local, peer], []).unwrap();
    let control = gateway.identity_separation_control();
    let identity = IdentityLifecycle::default();
    assert_eq!(
        identity.compare(&instance, pair).unwrap().outcome,
        IdentityOutcome::Unresolved,
    );
    let prepare = |action_id| {
        control
            .prepare_separation(
                &identity,
                &instance,
                pair,
                IdentitySeparationRequest::Confirm,
                action_id,
            )
            .unwrap()
    };
    let mut expired = prepare([1; 16]);
    expired.expires_at = Instant::now();
    assert!(matches!(
        control.commit_separation(&identity, &instance, &expired),
        Err(GatewayError::Management(ManagementError::Stale))
    ));
    assert!(
        control
            .lookup_separation(&instance, pair, [1; 16])
            .unwrap()
            .is_none()
    );

    let mut fresh = prepare([2; 16]);
    let committed = control
        .commit_separation(&identity, &instance, &fresh)
        .unwrap();
    assert_eq!(committed.state, IdentitySeparationState::Applies);
    fresh.expires_at = Instant::now();
    assert_eq!(
        control
            .commit_separation(&identity, &instance, &fresh)
            .unwrap(),
        committed
    );

    let competing = prepare([3; 16]);
    let undo = control
        .prepare_separation(
            &identity,
            &instance,
            pair,
            IdentitySeparationRequest::Undo { target_revision: 1 },
            [3; 16],
        )
        .unwrap();
    let withdrawn = control
        .commit_separation(&identity, &instance, &undo)
        .unwrap();
    assert_eq!(withdrawn.state, IdentitySeparationState::Withdrawn);
    assert_eq!(
        withdrawn.withdrawn_confirmation,
        Some(committed.event.clone())
    );
    assert!(matches!(
        control.commit_separation(&identity, &instance, &competing),
        Err(GatewayError::Management(ManagementError::Conflict))
    ));
    let retry = control
        .commit_separation(&identity, &instance, &fresh)
        .unwrap();
    assert_eq!(retry.event, committed.event);
    assert_eq!(retry.state, IdentitySeparationState::Historical);

    drop(instance);
    drop(gateway);
    std::fs::remove_dir_all(path).unwrap();
}
