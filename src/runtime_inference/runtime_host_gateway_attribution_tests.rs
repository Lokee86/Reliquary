use super::super::super::owner_operations::ActorClass;
use super::*;

#[test]
fn two_principals_in_one_conversation_recover_without_live_instances() {
    let state = test_path("attribution-state");
    let archive = test_path("attribution.rel");
    let first = gateway_at(state.clone());
    let owner = first.mount_rel(seeded_runtime_at(archive.clone())).unwrap();
    let caps = [OwnerGrant::ViewRel, OwnerGrant::SubmitConversation];
    let a = first
        .attach_instance("principal-A".into(), None, grants(&owner, &caps))
        .unwrap();
    let b = first
        .attach_instance("principal-B".into(), None, grants(&owner, &caps))
        .unwrap();
    first
        .submit_conversation(
            &a,
            &owner,
            submission("conversation-one", "from-A", "root-one"),
        )
        .unwrap();
    first
        .process_next_conversation_submission(&a, &owner, "conversation-one", "from-A")
        .unwrap();
    let mut second = submission("conversation-one", "from-B", "from-A");
    second.expected_conversation_revision = first
        .conversation
        .journal(&owner)
        .unwrap()
        .lock()
        .unwrap()
        .conversation_revision(&ConversationId::new("conversation-one").unwrap());
    first.submit_conversation(&b, &owner, second).unwrap();
    first
        .process_next_conversation_submission(&b, &owner, "conversation-one", "from-B")
        .unwrap();
    let before = {
        let journal = first.conversation.owner_journal(&owner).unwrap();
        let mut journal = journal.lock().unwrap();
        let a = journal
            .source_attribution("conversation-one", "from-A", ActorClass::User)
            .unwrap()
            .clone();
        let b = journal
            .source_attribution("conversation-one", "from-B", ActorClass::User)
            .unwrap()
            .clone();
        assert_eq!(a.actor_principal.as_deref(), Some("principal-A"));
        assert_eq!(b.actor_principal.as_deref(), Some("principal-B"));
        assert_ne!(a.generation_id, b.generation_id);
        assert_eq!(a.generation_initiator.as_deref(), Some("principal-A"));
        assert_eq!(b.generation_initiator.as_deref(), Some("principal-B"));
        assert!(
            journal
                .source_attribution("conversation-one", "from-B", ActorClass::Agent)
                .is_none()
        );
        assert!(
            journal
                .source_attribution("conversation-one", "root-one", ActorClass::User)
                .is_none()
        );
        // A's generation result may arrive after B's turn. Bind the explicit
        // initiating operation, without consulting the now-current B leaf.
        journal
            .bind_agent_source(&a.generation_id, "agent-result-a", "agent-node-a")
            .unwrap();
        let agent = journal
            .source_attribution("conversation-one", "agent-node-a", ActorClass::Agent)
            .unwrap()
            .clone();
        assert_eq!(agent.generation_initiator, a.generation_initiator);
        assert_eq!(agent.admission, a.admission);
        (a, b, agent)
    };
    first.detach_instance(&a).unwrap();
    first.detach_instance(&b).unwrap();
    drop(first.unmount_rel(&owner).unwrap());
    drop(first);
    let restarted = gateway_at(state.clone());
    assert_eq!(
        restarted
            .mount_rel(InteractionRuntime::new(Cva::open(&archive).unwrap()))
            .unwrap(),
        owner
    );
    let journal = restarted.conversation.owner_journal(&owner).unwrap();
    let journal = journal.lock().unwrap();
    assert_eq!(
        journal.source_attribution("conversation-one", "from-A", ActorClass::User),
        Some(&before.0)
    );
    assert_eq!(
        journal.source_attribution("conversation-one", "from-B", ActorClass::User),
        Some(&before.1)
    );
    assert_eq!(
        journal.source_attribution("conversation-one", "agent-node-a", ActorClass::Agent),
        Some(&before.2)
    );
    drop(journal);
    drop(restarted.unmount_rel(&owner).unwrap());
}

#[test]
fn retry_from_another_same_principal_attachment_keeps_original_admission() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let caps = [OwnerGrant::ViewRel, OwnerGrant::SubmitConversation];
    let a = gateway
        .attach_instance("same-principal".into(), None, grants(&owner, &caps))
        .unwrap();
    let b = gateway
        .attach_instance("same-principal".into(), None, grants(&owner, &caps))
        .unwrap();
    let request = submission("conversation-one", "one-source", "root-one");
    gateway
        .submit_conversation(&a, &owner, request.clone())
        .unwrap();
    gateway.submit_conversation(&b, &owner, request).unwrap();
    let journal = gateway.conversation.owner_journal(&owner).unwrap();
    let journal = journal.lock().unwrap();
    assert_eq!(journal.last_sequence(), 1);
    assert_eq!(
        journal
            .source_attribution("conversation-one", "one-source", ActorClass::User)
            .unwrap()
            .admission
            .instance_id,
        a.instance_id().to_string()
    );
    drop(journal);
    gateway.detach_instance(&a).unwrap();
    gateway.detach_instance(&b).unwrap();
    drop(gateway.unmount_rel(&owner).unwrap());
}

#[test]
fn corrupt_owner_journal_blocks_mount_and_missing_journal_never_backfills() {
    let state = test_path("attribution-loss-state");
    let archive = test_path("attribution-loss.rel");
    let first = gateway_at(state.clone());
    let owner = first.mount_rel(seeded_runtime_at(archive.clone())).unwrap();
    let caps = [OwnerGrant::ViewRel, OwnerGrant::SubmitConversation];
    let a = first
        .attach_instance("principal-A".into(), None, grants(&owner, &caps))
        .unwrap();
    first
        .submit_conversation(
            &a,
            &owner,
            submission("conversation-one", "source-A", "root-one"),
        )
        .unwrap();
    first
        .process_next_conversation_submission(&a, &owner, "conversation-one", "source-A")
        .unwrap();
    first.detach_instance(&a).unwrap();
    drop(first.unmount_rel(&owner).unwrap());
    drop(first);
    let path = state
        .join("owner-operations")
        .join(format!("{}.roj", owner.strip_prefix("rel-").unwrap()));
    let mut bytes = std::fs::read(&path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    std::fs::write(&path, bytes).unwrap();
    let corrupt = gateway_at(state.clone());
    assert!(
        corrupt
            .mount_rel(InteractionRuntime::new(Cva::open(&archive).unwrap()))
            .is_err()
    );
    assert!(corrupt.conversation.journal(&owner).is_err());
    drop(corrupt);
    std::fs::remove_file(&path).unwrap();
    let missing = gateway_at(state);
    missing
        .mount_rel(InteractionRuntime::new(Cva::open(&archive).unwrap()))
        .unwrap();
    assert!(
        missing
            .conversation
            .owner_journal(&owner)
            .unwrap()
            .lock()
            .unwrap()
            .source_attribution("conversation-one", "source-A", ActorClass::User)
            .is_none()
    );
    drop(missing.unmount_rel(&owner).unwrap());
}
