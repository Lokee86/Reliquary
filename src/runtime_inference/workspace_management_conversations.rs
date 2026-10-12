use super::*;
use crate::{ConversationPolicy, ConversationSummary, ResolvedInteractionTurn};

impl WorkspaceManagement {
    pub fn conversations(
        &self,
        owner: &str,
        offset: usize,
        limit: usize,
    ) -> Result<ManagementPage<ConversationSummary>, ManagementError> {
        validate_page(offset, limit)?;
        let token = self.token(owner)?;
        let mut values = self
            .gateway
            .conversation_summaries_for(&self.instance, owner)
            .map_err(unavailable)?;
        values.sort_by(|a, b| a.conversation_id.cmp(&b.conversation_id));
        let result = page(values, offset, limit);
        text_bound(
            result
                .items
                .iter()
                .map(|x| {
                    x.conversation_id.len()
                        + x.title.as_ref().map_or(0, String::len)
                        + x.leaf_node_ids.iter().map(String::len).sum::<usize>()
                })
                .sum(),
        )?;
        self.deliver(&token, result)
    }
    /// Opening/selection is instance-local viewing; it never admits a writer.
    pub fn select_conversation(
        &self,
        owner: &str,
        conversation: &str,
        branch: &str,
    ) -> Result<(), ManagementError> {
        self.gateway
            .select_managed_conversation(&self.instance, owner, conversation, branch)
            .map(|_| ())
            .map_err(unavailable)
    }
    pub fn select_leaf(
        &self,
        owner: &str,
        conversation: &str,
        leaf: &str,
    ) -> Result<(), ManagementError> {
        self.gateway
            .select_managed_conversation_leaf(&self.instance, owner, conversation, leaf)
            .map_err(unavailable)
    }
    pub fn clear_selection(&self, owner: &str) -> Result<(), ManagementError> {
        self.gateway
            .clear_managed_conversation_selection(&self.instance, owner)
            .map_err(unavailable)
    }
    pub fn inspect_conversation(
        &self,
        owner: &str,
        conversation: &str,
        offset: usize,
        limit: usize,
    ) -> Result<ConversationInspection, ManagementError> {
        validate_page(offset, limit)?;
        let token = self.token(owner)?;
        let observation = self
            .gateway
            .conversation_observation_snapshot(&self.instance, owner, conversation)
            .map_err(unavailable)?;
        let selection = self
            .gateway
            .instance_managed_state(&self.instance, owner)
            .map_err(unavailable)?;
        let mut branches = observation
            .branches
            .into_iter()
            .map(|b| ManagedBranch {
                branch_id: b.branch_id,
                head_node_id: b.head_node_id,
                fork_parent_node_id: b.fork_parent_node_id,
                display_label: b.display_label,
            })
            .collect::<Vec<_>>();
        branches.sort_by(|a, b| a.branch_id.cmp(&b.branch_id));
        let branches = page(branches, offset, limit);
        text_bound(
            branches
                .items
                .iter()
                .map(|b| {
                    b.branch_id.len()
                        + b.head_node_id.as_ref().map_or(0, String::len)
                        + b.fork_parent_node_id.as_ref().map_or(0, String::len)
                        + b.display_label.as_ref().map_or(0, String::len)
                })
                .sum(),
        )?;
        let result = ConversationInspection {
            conversation_id: observation.conversation_id.as_str().to_owned(),
            mount_epoch: observation.mount_epoch,
            journal_sequence: observation.durable_journal_sequence,
            policy: observation.policy,
            policy_revision: observation.policy_revision,
            selection,
            branches,
            submissions: observation.submissions.len(),
            native_generations: observation.generations.len(),
            host_operations: observation.host_operations.len(),
        };
        self.deliver(&token, result)
    }
    pub fn transcript(
        &self,
        owner: &str,
        conversation: &str,
        leaf: &str,
        offset: usize,
        limit: usize,
    ) -> Result<ManagementPage<ResolvedInteractionTurn>, ManagementError> {
        validate_page(offset, limit)?;
        let token = self.token(owner)?;
        let values = self
            .gateway
            .conversation_transcript_for(&self.instance, owner, conversation, leaf)
            .map_err(unavailable)?;
        let result = page(values, offset, limit);
        text_bound(
            result
                .items
                .iter()
                .map(|t| {
                    t.content.len()
                        + t.message_id.len()
                        + t.role.len()
                        + t.principal_id.as_ref().map_or(0, String::len)
                })
                .sum(),
        )?;
        self.deliver(&token, result)
    }
    /// Explicit writer branch preparation, separate from selection.
    pub fn prepare_branch(
        &self,
        owner: &str,
        conversation: &str,
        leaf: &str,
    ) -> Result<String, ManagementError> {
        self.gateway
            .prepare_conversation_branch_at_leaf(&self.instance, owner, conversation, leaf)
            .map_err(unavailable)
    }
    pub fn set_title(
        &self,
        owner: &str,
        conversation: &str,
        title: String,
    ) -> Result<bool, ManagementError> {
        self.gateway
            .set_conversation_title_for(&self.instance, owner, conversation, title)
            .map_err(unavailable)
    }
    pub fn set_policy(
        &self,
        owner: &str,
        conversation: &str,
        policy: ConversationPolicy,
    ) -> Result<bool, ManagementError> {
        self.gateway
            .set_conversation_policy(&self.instance, owner, conversation, policy)
            .map_err(unavailable)
    }
}
