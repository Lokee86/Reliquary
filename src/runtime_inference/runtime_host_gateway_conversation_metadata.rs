//! Explicit-instance Archive metadata mutations for the external conversation consumer.
//! The REL sequencer owns publication. Warlock must never mutate a CVA directly.
use super::mutation_events::RelMutationFamily;
use super::{GatewayRuntime, InstanceRuntime, OwnerGrant, ReliquaryRuntimeHostError, operation};

impl GatewayRuntime {
    /// Current durable REL-journal revision for exactly one authorized conversation.
    /// Consumers use this value as the optimistic admission revision; no journal
    /// handle or mutable queue authority is exported.
    pub fn conversation_revision_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
    ) -> Result<u64, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ViewRel)?;
        let conversation =
            crate::multiplexing_journal::ConversationId::new(conversation_id.to_owned())
                .map_err(operation)?;
        // Validate the owner/mount before and after the journal read so an
        // unmounted/replaced REL cannot deliver a stale revision.
        self.with_rel(&token, |_| Ok(()))?;
        let revision = self
            .conversation
            .journal(owner_id)?
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .conversation_revision(&conversation);
        self.with_rel(&token, |_| Ok(revision))
    }

    pub fn conversation_branch_id_for_leaf(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<String, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ViewRel)?;
        self.with_rel(&token, |runtime| {
            runtime
                .cva()
                .archive()
                .branches()
                .into_iter()
                .find(|branch| {
                    branch.conversation_id == conversation_id && branch.leaf_node_id == leaf_node_id
                })
                .map(|branch| branch.id)
                .ok_or_else(|| operation("conversation leaf is not a current branch head"))
        })
    }

    pub fn conversation_branch_leaf_for_id(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
        branch_id: &str,
    ) -> Result<String, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ViewRel)?;
        self.with_rel(&token, |runtime| {
            runtime
                .cva()
                .archive()
                .branches()
                .into_iter()
                .find(|branch| branch.conversation_id == conversation_id && branch.id == branch_id)
                .map(|branch| branch.leaf_node_id)
                .ok_or_else(|| operation("conversation branch is not present"))
        })
    }

    pub fn mark_managed_conversation_resumed(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ViewRel)?;
        self.with_rel(&token, |_| Ok(()))?;
        let mut managed = self.instance_managed_state(instance, owner_id)?;
        if managed.conversation_id.as_deref() != Some(conversation_id) {
            return Err(operation("conversation is not selected by this instance"));
        }
        managed.reopened_session_pending = false;
        self.set_instance_managed_state(instance, owner_id, managed)
    }

    /// Branch metadata is read only from the exact owner granted to this instance.
    pub fn conversation_branch_start_node_ids_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<String>, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ViewRel)?;
        self.with_rel(&token, |runtime| {
            runtime
                .cva()
                .conversation_branch_start_node_ids(conversation_id, leaf_node_id)
                .map_err(operation)
        })
    }

    /// Change the title through REL admission, durable sync and Archive publication.
    pub fn set_conversation_title_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
        title: String,
    ) -> Result<bool, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ManageRel)?;
        self.with_rel_mutation(&token, RelMutationFamily::ConversationMetadata, |runtime| {
            let changed = runtime
                .cva
                .set_conversation_title(conversation_id, title)
                .map_err(operation)?;
            Ok((changed, changed, Vec::new()))
        })
    }

    pub fn conversation_compactions_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
    ) -> Result<Vec<crate::ConversationCompaction>, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ViewRel)?;
        self.with_rel(&token, |runtime| {
            Ok(runtime.cva().conversation_compactions(conversation_id))
        })
    }

    /// Compaction is a durable Archive adjunct, never a Tauri-managed cursor.
    pub fn put_conversation_compaction_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: String,
        through_message_id: String,
        summary: String,
        supersedes_through_message_id: Option<&str>,
    ) -> Result<crate::ConversationCompaction, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ManageRel)?;
        self.with_rel_mutation(&token, RelMutationFamily::ConversationMetadata, |runtime| {
            let record = runtime
                .cva
                .put_conversation_compaction(
                    conversation_id,
                    through_message_id,
                    summary,
                    supersedes_through_message_id,
                )
                .map_err(operation)?;
            Ok((record, true, Vec::new()))
        })
    }

    /// Echo publication is a synced Archive adjunct; it does not confer source REL access.
    pub fn put_echo_event_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        event: crate::EchoEvent,
    ) -> Result<bool, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ManageRel)?;
        self.with_rel_mutation(&token, RelMutationFamily::ConversationMetadata, |runtime| {
            let changed = runtime.cva.put_echo_event(event).map_err(operation)?;
            Ok((changed, changed, Vec::new()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_host_test_support::test_path;
    use crate::{
        Cva, EpisodePolicy, InsomniaWorkerConfig, InteractionRuntime, ReliquaryRuntimeRoutes,
    };
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn p5c_archive_metadata_requires_exact_manage_rel_and_is_durable() {
        let gateway = GatewayRuntime::new(
            test_path("p5c-conversation-metadata-state"),
            ReliquaryRuntimeRoutes::default(),
            InsomniaWorkerConfig {
                workers: 0,
                ..InsomniaWorkerConfig::default()
            },
            EpisodePolicy::default(),
        );
        let rel = gateway
            .mount_rel(InteractionRuntime::new(
                Cva::create_project(test_path("p5c-conversation-metadata.rel")).unwrap(),
            ))
            .unwrap();
        let view = gateway
            .attach_instance(
                "reader".into(),
                None,
                BTreeMap::from([(rel.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
            )
            .unwrap();
        assert_eq!(
            gateway
                .conversation_revision_for(&view, &rel, "conv")
                .unwrap(),
            0
        );
        let denied = gateway
            .attach_instance("denied".into(), None, BTreeMap::new())
            .unwrap();
        assert!(
            gateway
                .conversation_revision_for(&denied, &rel, "conv")
                .is_err()
        );
        gateway.detach_instance(&denied).unwrap();
        assert!(
            gateway
                .set_conversation_title_for(&view, &rel, "conv", "not allowed".into())
                .is_err()
        );
        assert!(
            gateway
                .put_echo_event_for(
                    &view,
                    &rel,
                    crate::EchoEvent {
                        conversation_id: "conv".into(),
                        message_id: "node".into(),
                        sequence: 0,
                        timestamp_ns: 0,
                        model_round: None,
                        kind: crate::EchoEventKind::Commentary,
                        correlation_id: None,
                        name: None,
                        content: "cannot write".into()
                    }
                )
                .is_err()
        );
        let writer = gateway
            .attach_instance(
                "writer".into(),
                None,
                BTreeMap::from([(
                    rel.clone(),
                    BTreeSet::from([
                        OwnerGrant::ViewRel,
                        OwnerGrant::ManageRel,
                        OwnerGrant::SubmitConversation,
                    ]),
                )]),
            )
            .unwrap();
        // A title mutation does not create a conversation. Seed a real
        // admitted turn before testing title publication and durability.
        assert!(
            gateway
                .set_conversation_title_for(&writer, &rel, "conv", "renamed".into())
                .is_err()
        );
        gateway
            .submit_conversation(
                &writer,
                &rel,
                crate::GatewayConversationSubmission {
                    conversation_id: "conv".into(),
                    submission_id: "metadata-user".into(),
                    branch_id: "main".into(),
                    expected_conversation_revision: 0,
                    expected_parent_node_id: None,
                    content: "create the conversation".into(),
                    attachments: Vec::new(),
                    client_timestamp_ns: None,
                },
            )
            .unwrap();
        gateway
            .process_next_conversation_submission(&writer, &rel, "conv")
            .unwrap()
            .unwrap();
        assert!(
            gateway
                .set_conversation_title_for(&writer, &rel, "conv", "renamed".into())
                .unwrap()
        );
        let reader_token = gateway.authorize(&view, &rel, OwnerGrant::ViewRel).unwrap();
        gateway
            .with_rel(&reader_token, |runtime| {
                assert_eq!(
                    runtime
                        .cva()
                        .archive()
                        .conversation_metadata("conv")
                        .unwrap()
                        .title
                        .as_deref(),
                    Some("renamed"),
                );
                Ok(())
            })
            .unwrap();
        gateway.detach_instance(&writer).unwrap();
        gateway.detach_instance(&view).unwrap();
        drop(gateway.unmount_rel(&rel).unwrap());
    }
}
