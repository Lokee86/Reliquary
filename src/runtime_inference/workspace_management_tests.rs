use super::*;
use crate::runtime_host_test_support::test_path;
use crate::{Branch, Cva, InsomniaWorkerConfig, InteractionRuntime};
use std::collections::{BTreeMap, BTreeSet};

struct Fixture {
    g: Arc<GatewayRuntime>,
    owner: String,
    instance: Arc<InstanceRuntime>,
    api: WorkspaceManagement,
}
impl Fixture {
    fn new(label: &str) -> Self {
        let g = Arc::new(GatewayRuntime::new(
            test_path(&format!("{label}-state")),
            Default::default(),
            InsomniaWorkerConfig {
                workers: 0,
                ..Default::default()
            },
            Default::default(),
        ));
        let mut cva = Cva::create_project(test_path(&format!("{label}.rel"))).unwrap();
        for (conv, node) in [("a", "a-root"), ("b", "b-root")] {
            cva.append_node(
                node.into(),
                conv.into(),
                None,
                "user".into(),
                1,
                "PRIVATE_TRANSCRIPT_SENTINEL",
            )
            .unwrap();
            cva.append_branch(Branch {
                id: format!("{conv}-main"),
                conversation_id: conv.into(),
                leaf_node_id: node.into(),
                canonical: true,
            })
            .unwrap();
        }
        cva.sync().unwrap();
        let owner = g.mount_rel(InteractionRuntime::new(cva)).unwrap();
        let instance = g
            .attach_instance(
                "one".into(),
                None,
                BTreeMap::from([(
                    owner.clone(),
                    BTreeSet::from([
                        OwnerGrant::ViewRel,
                        OwnerGrant::SubmitConversation,
                        OwnerGrant::ManageRel,
                        OwnerGrant::ManageProject,
                    ]),
                )]),
            )
            .unwrap();
        let api = g.workspace_management(instance.clone());
        Self {
            g,
            owner,
            instance,
            api,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.g.shutdown().unwrap();
    }
}

#[test]
fn page_bounds_order_and_exact_owner_reads() {
    let f = Fixture::new("management-pages");
    let first = f.api.conversations(&f.owner, 0, 1).unwrap();
    assert_eq!(first.items[0].conversation_id, "a");
    assert_eq!(first.next_offset, Some(1));
    assert_eq!(
        f.api.conversations(&f.owner, 1, 1).unwrap().items[0].conversation_id,
        "b"
    );
    assert_eq!(first.total_items, 2);
    assert!(
        f.api
            .conversations(&f.owner, 2, 1)
            .unwrap()
            .items
            .is_empty()
    );
    for (offset, limit) in [(0, 0), (0, MAX_MANAGEMENT_PAGE_ITEMS + 1), (usize::MAX, 1)] {
        assert_eq!(
            f.api.conversations(&f.owner, offset, limit).unwrap_err(),
            ManagementError::InvalidPage
        );
    }
    assert!(f.api.conversations("rel-ungranted", 0, 1).is_err());
}
#[test]
fn selection_is_instance_local_and_reading_never_creates_a_writer() {
    let f = Fixture::new("management-selection");
    let other =
        f.g.attach_instance(
            "two".into(),
            None,
            BTreeMap::from([(f.owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    let api = f.g.workspace_management(other.clone());
    f.api.select_conversation(&f.owner, "a", "a-main").unwrap();
    api.select_conversation(&f.owner, "b", "b-main").unwrap();
    let a = f.api.inspect_conversation(&f.owner, "a", 0, 10).unwrap();
    let b = api.inspect_conversation(&f.owner, "b", 0, 10).unwrap();
    assert_eq!(a.selection.conversation_id.as_deref(), Some("a"));
    assert_eq!(b.selection.conversation_id.as_deref(), Some("b"));
    assert_eq!(a.submissions, 0);
    assert_eq!(a.native_generations, 0);
    assert_eq!(
        f.api
            .transcript(&f.owner, "a", "a-root", 0, 1)
            .unwrap()
            .items
            .len(),
        1
    );
    assert_eq!(a.branches.items[0].branch_id, "a-main");
    api.clear_selection(&f.owner).unwrap();
    assert!(
        api.inspect_conversation(&f.owner, "b", 0, 10)
            .unwrap()
            .selection
            .conversation_id
            .is_none()
    );
    assert_eq!(
        f.api
            .inspect_conversation(&f.owner, "a", 0, 10)
            .unwrap()
            .selection
            .conversation_id
            .as_deref(),
        Some("a")
    );
}
#[test]
fn mutations_keep_concrete_grants_and_publish_invalidation() {
    let f = Fixture::new("management-events");
    let view =
        f.g.attach_instance(
            "view".into(),
            None,
            BTreeMap::from([(f.owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    let read = f.g.workspace_management(view.clone());
    assert!(read.set_title(&f.owner, "a", "forbidden".into()).is_err());
    assert!(
        read.create_memory(
            &f.owner,
            "x".into(),
            "y".into(),
            "knowledge".into(),
            "fact".into(),
            1
        )
        .is_err()
    );
    let (_, recipient) = f.g.subscribe_rel(&view, &f.owner, None, 32).unwrap();
    assert!(f.api.set_title(&f.owner, "a", "Renamed".into()).unwrap());
    assert!(f.g.next_rel_event(&view, &recipient).unwrap().is_some());
    let memory = f
        .api
        .create_memory(
            &f.owner,
            "Fact".into(),
            "evidence".into(),
            "knowledge".into(),
            "fact".into(),
            1,
        )
        .unwrap();
    assert!(f.g.next_rel_event(&view, &recipient).unwrap().is_some());
    let replacement = f
        .api
        .replace_memory(&f.owner, memory.id, "Changed".into(), "new".into(), 2)
        .unwrap();
    let retired = f.api.memory(&f.owner, memory.id).unwrap();
    assert!(retired.archived);
    assert_eq!(retired.superseded_by, Some(replacement.id));
    assert_eq!(f.api.memories(&f.owner, 0, 10).unwrap().total_items, 2);
}
#[test]
fn health_is_body_free_and_maintenance_is_explicit() {
    let f = Fixture::new("management-health");
    let health = f.api.health(&f.owner).unwrap();
    assert_eq!(health.conversations, 2);
    assert_eq!(health.branches, 2);
    assert_eq!(health.versions.memory, Some(0));
    assert!(!format!("{health:?}").contains("PRIVATE_TRANSCRIPT_SENTINEL"));
    assert!(f.api.reconcile_quiesced(&f.owner).is_err());
    f.api.quiesce(&f.owner).unwrap();
    assert!(f.api.health(&f.owner).is_err());
    assert!(f.api.conversations(&f.owner, 0, 10).is_err());
    f.api.reconcile_quiesced(&f.owner).unwrap();
    assert!(f.api.reopen(&f.owner).unwrap() > health.mount_epoch);
    assert!(f.api.health(&f.owner).is_ok());
}
#[test]
fn unsupported_destructive_operations_change_nothing() {
    let f = Fixture::new("management-unsupported");
    let before = f.api.health(&f.owner).unwrap().versions;
    for operation in [
        UnsupportedManagementOperation::FileRename,
        UnsupportedManagementOperation::FileMove,
        UnsupportedManagementOperation::FileUnlink,
        UnsupportedManagementOperation::StandaloneFileImport,
        UnsupportedManagementOperation::SelectiveImportExport,
        UnsupportedManagementOperation::MemoryDeletion,
    ] {
        assert_eq!(
            f.api.unsupported_operation(&f.owner, operation),
            Err(ManagementError::Unsupported(operation))
        );
    }
    assert_eq!(f.api.health(&f.owner).unwrap().versions, before);
    assert!(matches!(
        f.api
            .unsupported_operation("rel-ungranted", UnsupportedManagementOperation::FileUnlink),
        Err(ManagementError::Unavailable)
    ));
}
#[test]
fn revocation_and_detach_invalidate_existing_facade() {
    let f = Fixture::new("management-revoke");
    f.g.replace_instance_grants(&f.instance, BTreeMap::new())
        .unwrap();
    assert!(f.api.conversations(&f.owner, 0, 10).is_err());
    assert!(f.api.health(&f.owner).is_err());
    f.g.detach_instance(&f.instance).unwrap();
    assert!(f.api.owners(0, 10).is_err());
}

#[test]
fn exact_private_owners_are_isolated_and_binding_is_not_a_grant() {
    let f = Fixture::new("management-private");
    let a =
        f.g.mount_phy(crate::Phylactery::create(test_path("management-a.phy")).unwrap())
            .unwrap();
    let b =
        f.g.mount_phy(crate::Phylactery::create(test_path("management-b.phy")).unwrap())
            .unwrap();
    let attach = |principal: &str, phy: &str| {
        f.g.attach_instance(
            principal.into(),
            Some(phy.into()),
            BTreeMap::from([
                (f.owner.clone(), BTreeSet::from([OwnerGrant::ViewRel])),
                (phy.into(), BTreeSet::from([OwnerGrant::UseOwnPhy])),
            ]),
        )
        .unwrap()
    };
    let alice = attach("alice", &a);
    let bob = attach("bob", &b);
    let a_api = f.g.workspace_management(alice.clone());
    let b_api = f.g.workspace_management(bob);
    let private = a_api
        .create_memory(
            &a,
            "ALICE_SECRET".into(),
            "private body".into(),
            "knowledge".into(),
            "fact".into(),
            1,
        )
        .unwrap();
    assert_eq!(a_api.memories(&a, 0, 10).unwrap().items[0].id, private.id);
    assert!(b_api.memories(&a, 0, 10).is_err());
    assert!(b_api.memory(&a, private.id).is_err());
    assert!(b_api.statistics(&a).is_err());
    assert!(!format!("{:?}", b_api.owners(0, 10).unwrap()).contains(&a));
    assert!(!format!("{:?}", f.api.health(&f.owner).unwrap()).contains("ALICE_SECRET"));
    let bound =
        f.g.attach_instance("alice".into(), Some(a.clone()), BTreeMap::new())
            .unwrap();
    assert!(f.g.workspace_management(bound).memories(&a, 0, 10).is_err());
    let captured = a_api.token(&a).unwrap();
    f.g.replace_instance_grants(&alice, BTreeMap::new())
        .unwrap();
    assert!(a_api.deliver(&captured, private).is_err());
}
#[test]
fn original_delivery_token_rejects_reopened_epoch_and_regrant() {
    let f = Fixture::new("management-delivery");
    let token = f.api.token(&f.owner).unwrap();
    f.api.quiesce(&f.owner).unwrap();
    f.api.reopen(&f.owner).unwrap();
    assert!(f.api.deliver(&token, "old result").is_err());
    let token = f.api.token(&f.owner).unwrap();
    let grants = f.instance.grants_snapshot().unwrap();
    f.g.replace_instance_grants(&f.instance, BTreeMap::new())
        .unwrap();
    f.g.replace_instance_grants(&f.instance, grants).unwrap();
    assert!(f.api.deliver(&token, "old authorization").is_err());
}
#[test]
fn source_inventory_and_file_bytes_preserve_source_identity() {
    let f = Fixture::new("management-files");
    let path = test_path("management-files-source.rel");
    let mut cva = Cva::create_project(&path).unwrap();
    let ingested = cva
        .ingest_turn(crate::IncomingTurn {
            id: "source".into(),
            conversation_id: "files".into(),
            parent_id: None,
            role: "user".into(),
            principal_id: None,
            timestamp_ns: 1,
            content: "source".into(),
            attachments: vec![crate::IncomingAttachment {
                filename: "evidence.txt".into(),
                mime_type: Some("text/plain".into()),
                bytes: b"exact bytes".to_vec(),
            }],
            project_attachments: vec![],
        })
        .unwrap();
    cva.sync().unwrap();
    let owner = f.g.mount_rel(InteractionRuntime::new(cva)).unwrap();
    let reader =
        f.g.attach_instance(
            "files".into(),
            None,
            BTreeMap::from([(owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    let api = f.g.workspace_management(reader);
    let files = api.source_files(&owner, "files", "source", 0, 1).unwrap();
    assert_eq!(files.items[0].file.id, ingested.attachments[0].id);
    assert!(files.items[0].project_reference.is_none());
    assert_eq!(
        api.read_file(&owner, files.items[0].file.id, None).unwrap(),
        b"exact bytes"
    );
    assert!(
        api.source_files(&owner, "files", "other", 0, 10)
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        f.api
            .read_file(&owner, files.items[0].file.id, None)
            .is_err()
    );
}
#[test]
fn policy_and_explicit_branch_management_survive_reopen() {
    let f = Fixture::new("management-branch");
    let token =
        f.g.authorize(&f.instance, &f.owner, OwnerGrant::SubmitConversation)
            .unwrap();
    // Fixture publication belongs to Archive, never the management facade.
    f.g.with_rel_mutation(
        &token,
        super::super::mutation_events::RelMutationFamily::ConversationBranch,
        |runtime| {
            runtime
                .cva
                .append_node(
                    "a-child".into(),
                    "a".into(),
                    Some("a-root".into()),
                    "assistant".into(),
                    2,
                    "reply",
                )
                .unwrap();
            runtime
                .cva
                .append_branch(Branch {
                    id: "a-main".into(),
                    conversation_id: "a".into(),
                    leaf_node_id: "a-child".into(),
                    canonical: true,
                })
                .unwrap();
            Ok(((), true, Vec::new()))
        },
    )
    .unwrap();
    f.api.select_leaf(&f.owner, "a", "a-root").unwrap();
    assert_eq!(
        f.api
            .inspect_conversation(&f.owner, "a", 0, 10)
            .unwrap()
            .branches
            .total_items,
        1
    );
    let branch = f.api.prepare_branch(&f.owner, "a", "a-root").unwrap();
    assert_eq!(
        f.api.prepare_branch(&f.owner, "a", "a-root").unwrap(),
        branch
    );
    f.api
        .set_policy(&f.owner, "a", crate::ConversationPolicy::VisibleAutoBranch)
        .unwrap();
    f.api.quiesce(&f.owner).unwrap();
    f.api.reopen(&f.owner).unwrap();
    let inspect = f.api.inspect_conversation(&f.owner, "a", 0, 10).unwrap();
    assert_eq!(inspect.policy, crate::ConversationPolicy::VisibleAutoBranch);
    assert!(inspect.branches.items.iter().any(|b| b.branch_id == branch));
}
#[test]
fn oversized_transcript_is_rejected_and_errors_are_sanitized() {
    let f = Fixture::new("management-size");
    let path = test_path("management-size-owner.rel");
    let mut cva = Cva::create_project(&path).unwrap();
    cva.append_node(
        "large".into(),
        "large".into(),
        None,
        "user".into(),
        1,
        &"x".repeat(MAX_MANAGEMENT_PAGE_BYTES + 1),
    )
    .unwrap();
    cva.sync().unwrap();
    let owner = f.g.mount_rel(InteractionRuntime::new(cva)).unwrap();
    let reader =
        f.g.attach_instance(
            "large".into(),
            None,
            BTreeMap::from([(owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    let api = f.g.workspace_management(reader);
    assert_eq!(
        api.transcript(&owner, "large", "large", 0, 1).unwrap_err(),
        ManagementError::ResponseTooLarge
    );
    let error = unavailable(ReliquaryRuntimeHostError::Operation(
        "SECRET_PROVIDER_PATH".into(),
    ));
    assert!(!format!("{error:?}: {error}").contains("SECRET_PROVIDER_PATH"));
}
