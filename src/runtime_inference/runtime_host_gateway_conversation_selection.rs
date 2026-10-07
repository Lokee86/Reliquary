//! Per-instance historical viewing and explicit writer branch preparation.
use super::mutation_events::RelMutationFamily;
use super::*;

impl GatewayRuntime {
    /// Viewing a historical leaf changes only this instance's cursor.
    pub fn select_managed_conversation_leaf(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ViewRel)?;
        let branch_id = self.with_rel(&token, |runtime| {
            if !runtime
                .cva()
                .archive()
                .has_node(conversation_id, leaf_node_id)
            {
                return Err(operation("selected leaf is not in this conversation"));
            }
            Ok(runtime
                .cva()
                .archive()
                .branches()
                .into_iter()
                .find(|b| b.conversation_id == conversation_id && b.leaf_node_id == leaf_node_id)
                .map(|b| b.id))
        })?;
        self.select_rel(instance, owner_id)?;
        self.set_instance_managed_state(
            instance,
            owner_id,
            super::super::instances::ManagedConversationState {
                conversation_id: Some(conversation_id.to_owned()),
                branch_id,
                selected_leaf_node_id: Some(leaf_node_id.to_owned()),
                reopened_session_pending: true,
            },
        )
    }

    /// Explicit send from history creates a durable branch at the selected leaf.
    /// Reader selection alone never calls this method or creates a continuation.
    pub fn prepare_conversation_branch_at_leaf(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<String, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::SubmitConversation)?;
        self.with_rel_mutation(&token, RelMutationFamily::ConversationBranch, |runtime| {
            if !runtime
                .cva()
                .archive()
                .has_node(conversation_id, leaf_node_id)
            {
                return Err(operation("selected leaf is not in this conversation"));
            }
            if let Some(branch) =
                runtime.cva().archive().branches().into_iter().find(|b| {
                    b.conversation_id == conversation_id && b.leaf_node_id == leaf_node_id
                })
            {
                return Ok((branch.id, false, Vec::new()));
            }
            let branch_id = format!("branch-{}", uuid::Uuid::new_v4());
            runtime
                .cva
                .append_branch(crate::Branch {
                    id: branch_id.clone(),
                    conversation_id: conversation_id.to_owned(),
                    leaf_node_id: leaf_node_id.to_owned(),
                    canonical: false,
                })
                .map_err(operation)?;
            Ok((branch_id, true, Vec::new()))
        })
    }

    pub fn clear_managed_conversation_selection(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ViewRel)?;
        self.with_rel(&token, |_| Ok(()))?;
        self.set_instance_managed_state(
            instance,
            owner_id,
            super::super::instances::ManagedConversationState::default(),
        )
    }
}
