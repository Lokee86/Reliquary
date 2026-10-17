use crate::{Archive, ArchiveError, Container, ConversationSummary, ResolvedTurn};
use std::collections::{HashMap, HashSet};

impl Archive {
    pub fn conversation_summaries(&self) -> Vec<ConversationSummary> {
        let mut grouped = HashMap::<&str, Vec<_>>::new();
        for node in self.nodes.iter() {
            if self.is_conversation_tombstoned(&node.conversation_id) {
                continue;
            }
            grouped
                .entry(node.conversation_id.as_str())
                .or_default()
                .push(node);
        }

        let mut summaries = grouped
            .into_iter()
            .map(|(conversation_id, nodes)| {
                let metadata = self.conversation_metadata(conversation_id);
                let title = metadata.and_then(|metadata| metadata.title.clone());
                let active = metadata.is_some_and(|metadata| metadata.active);
                summarize(conversation_id, title, active, nodes)
            })
            .collect::<Vec<_>>();
        summaries.sort_by(|left, right| {
            right
                .latest_timestamp_ns
                .cmp(&left.latest_timestamp_ns)
                .then_with(|| left.conversation_id.cmp(&right.conversation_id))
        });
        summaries
    }

    pub(crate) fn conversation_turns(
        &self,
        container: &mut Container,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.require_current_conversation_id(conversation_id)?;
        self.historical_conversation_turns(container, conversation_id, leaf_node_id)
    }

    pub(crate) fn historical_conversation_turns(
        &self,
        container: &mut Container,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.branch_nodes(conversation_id, leaf_node_id)?
            .into_iter()
            .map(|node| self.resolve_turn(container, node))
            .collect()
    }

    pub(crate) fn conversation_turn_page(
        &self,
        container: &mut Container,
        conversation_id: &str,
        end_node_id: &str,
        limit: usize,
    ) -> Result<(Vec<ResolvedTurn>, Option<String>), ArchiveError> {
        let limit = limit.max(1);
        self.require_current_conversation_id(conversation_id)?;
        let mut nodes = Vec::with_capacity(limit);
        let mut seen = HashSet::new();
        let mut current = Some(end_node_id.to_owned());
        while nodes.len() < limit {
            let Some(id) = current else { break };
            if !seen.insert(id.clone()) {
                return Err(ArchiveError::NodeCycle);
            }
            let node = self
                .require_node(conversation_id, &id, ArchiveError::MissingNode)?
                .clone();
            current = node.parent_id.clone();
            nodes.push(node);
        }
        let older_cursor = current;
        nodes.reverse();
        let turns = nodes
            .into_iter()
            .map(|node| self.resolve_turn(container, node))
            .collect::<Result<Vec<_>, _>>()?;
        Ok((turns, older_cursor))
    }

    fn resolve_turn(
        &self,
        container: &mut Container,
        node: crate::Node,
    ) -> Result<ResolvedTurn, ArchiveError> {
        Ok(ResolvedTurn {
            node_id: node.id,
            role: node.role,
            principal_id: node.principal_id,
            timestamp_ns: node.timestamp_ns,
            content: self.content(container, node.content_id)?,
        })
    }

    pub(crate) fn conversation_branch_start_node_ids(
        &self,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<String>, ArchiveError> {
        self.require_current_conversation_id(conversation_id)?;
        let path = self.branch_nodes(conversation_id, leaf_node_id)?;
        let mut child_counts = HashMap::<&str, usize>::new();
        for node in self
            .nodes
            .iter()
            .filter(|node| node.conversation_id == conversation_id)
        {
            if let Some(parent_id) = node.parent_id.as_deref() {
                *child_counts.entry(parent_id).or_default() += 1;
            }
        }
        Ok(path
            .into_iter()
            .filter(|node| {
                node.parent_id.as_deref().is_some_and(|parent_id| {
                    child_counts.get(parent_id).copied().unwrap_or_default() > 1
                })
            })
            .map(|node| node.id)
            .collect())
    }
}

fn summarize(
    conversation_id: &str,
    title: Option<String>,
    active: bool,
    nodes: Vec<&crate::Node>,
) -> ConversationSummary {
    let parents = nodes
        .iter()
        .filter_map(|node| node.parent_id.as_deref())
        .collect::<HashSet<_>>();
    let mut leaves = nodes
        .iter()
        .filter(|node| !parents.contains(node.id.as_str()))
        .copied()
        .collect::<Vec<_>>();
    leaves.sort_by(|left, right| {
        right
            .timestamp_ns
            .cmp(&left.timestamp_ns)
            .then_with(|| left.id.cmp(&right.id))
    });

    ConversationSummary {
        conversation_id: conversation_id.to_owned(),
        title,
        active,
        leaf_node_ids: leaves.into_iter().map(|node| node.id.clone()).collect(),
        turn_count: nodes.len(),
        latest_timestamp_ns: nodes
            .iter()
            .map(|node| node.timestamp_ns)
            .max()
            .unwrap_or_default(),
    }
}
