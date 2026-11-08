//! Validate the selected graph before opening a writer.

use super::ancestry::ParentGraph;
use super::*;
use reliquary_owner_session::model::{ImportedBranch, ImportedMessage};
use std::collections::BTreeMap;

pub(super) struct SourceConversation {
    pub id: String,
    pub title: Option<String>,
    pub turns: Vec<SourceTurn>,
    pub branches: Vec<Branch>,
    pub episodes: Option<Vec<SourceEpisode>>,
}

pub(super) struct SourceTurn {
    pub id: String,
    pub source_id: String,
    pub parent: Option<String>,
    pub original_parent: Option<String>,
    pub role: String,
    pub timestamp_ns: i64,
    pub content: String,
    pub asset_references: usize,
    pub source_branch: Option<(String, bool)>,
}

pub(super) struct SourceEpisode {
    pub start: String,
    pub end: String,
    pub turn_count: Option<usize>,
}

impl PlanBuilder {
    pub(super) fn add_conversation(
        &mut self,
        conversation: SourceConversation,
    ) -> Result<(), NativeImportSourceError> {
        self.add_conversation_with_roles(conversation, |role| {
            matches!(role, "user" | "assistant" | "system" | "developer")
        })
    }

    pub(super) fn add_conversation_with_roles(
        &mut self,
        conversation: SourceConversation,
        accepts_role: impl Fn(&str) -> bool,
    ) -> Result<(), NativeImportSourceError> {
        let SourceConversation {
            id,
            title,
            turns,
            branches,
            episodes,
        } = conversation;
        if title
            .as_ref()
            .is_some_and(|title| title.len() > MAX_TURN_BYTES)
        {
            return Err(NativeImportSourceError::Limit(
                "source conversation title exceeds the byte limit",
            ));
        }
        if turns.is_empty() && branches.is_empty() && episodes.as_ref().is_none_or(Vec::is_empty) {
            return Ok(());
        }
        let graph = ParentGraph::new(
            turns
                .iter()
                .map(|turn| (turn.id.as_str(), turn.parent.as_deref())),
        )?;
        let tips = branch_tips(&graph, &branches)?;
        let representatives = representative_branches(&graph, &branches, &tips)?;
        let episodes = episodes.unwrap_or_else(|| graph.derive_episodes(&turns, &representatives));
        self.episodes
            .extend(graph.check_episodes(&id, &turns, episodes)?);

        let mut messages = Vec::with_capacity(turns.len());
        let mut source_ids = BTreeSet::new();
        let mut action_indices = vec![0; turns.len()];
        let mut accepted_tips = vec![false; branches.len()];
        for &index in &graph.order {
            let turn = &turns[index];
            validate_id(&turn.source_id)?;
            if !source_ids.insert(&turn.source_id) {
                return Err(invalid("duplicate source message ID"));
            }
            if let Some(parent) = &turn.original_parent {
                validate_id(parent)?;
                if graph
                    .by_id
                    .get(parent)
                    .is_some_and(|&original| Some(original) != graph.parents[index])
                {
                    return Err(invalid(
                        "retained original parent differs from native parent",
                    ));
                }
            }
            if let Some((branch_id, canonical)) = &turn.source_branch {
                let &(leaf, branch) = tips
                    .get(branch_id.as_str())
                    .ok_or_else(|| invalid("turn names an unknown source branch"))?;
                if branches[branch].canonical != *canonical || !graph.ancestor(index, leaf) {
                    return Err(invalid(
                        "turn does not belong to its declared source branch",
                    ));
                }
            }
            if !accepts_role(&turn.role) {
                return Err(invalid("unsupported visible source role"));
            }
            if turn.content.len() > MAX_TURN_BYTES {
                return Err(NativeImportSourceError::Limit(
                    "source turn exceeds the 256 KiB text limit",
                ));
            }
            if turn.original_parent != turn.parent {
                self.parent_mappings.push(NativeImportParentMapping {
                    conversation_id: id.clone(),
                    node_id: turn.id.clone(),
                    original_parent_id: turn.original_parent.clone(),
                    retained_parent_id: turn.parent.clone(),
                });
            }
            self.coverage.omitted_asset_references += turn.asset_references;
            self.coverage.text_bytes += turn.content.len();
            let fingerprint_fields = (
                &id,
                &turn.id,
                &turn.source_id,
                &turn.original_parent,
                &turn.parent,
                &turn.role,
                turn.timestamp_ns,
                &turn.content,
                turn.asset_references,
            );
            let fingerprint = serde_json::to_vec(&fingerprint_fields)
                .map_err(|_| invalid("could not fingerprint source turn"))?;
            messages.push(ImportedMessage {
                source_author: None,
                source_id: turn.source_id.clone(),
                revision: "export-snapshot-v1".into(),
                source_parent_id: graph.parents[index]
                    .map(|parent| turns[parent].source_id.clone()),
                replaces: None,
                deleted: false,
                node_id: Some(turn.id.clone()),
                fingerprint: Sha256::digest(fingerprint).into(),
                artifacts: vec![],
            });
            let input = TurnInput {
                id: turn.id.clone(),
                conversation_id: id.clone(),
                parent_id: turn.parent.clone(),
                role: turn.role.clone(),
                principal_id: None,
                timestamp_ns: turn.timestamp_ns,
                content: turn.content.clone(),
                attachments: vec![],
                project_attachments: vec![],
            };
            let branch_index = representatives[index];
            let mut branch = branches[branch_index].clone();
            accepted_tips[branch_index] |= branch.leaf_node_id == input.id;
            branch.leaf_node_id = input.id.clone();
            self.reserve_action(&input, &branch)?;
            action_indices[index] = self.turns.len();
            self.turns.push((input, branch));
        }
        for (index, branch) in branches.iter().enumerate() {
            if !accepted_tips[index] {
                let leaf = tips[branch.id.as_str()].0;
                let turn = self.turns[action_indices[leaf]].0.clone();
                self.reserve_action(&turn, branch)?;
                self.turns.push((turn, branch.clone()));
            }
        }
        self.coverage.conversations += 1;
        self.coverage.turns += turns.len();
        self.coverage.branches += branches.len();
        self.coverage.episodes = self.episodes.len();
        self.coverage.remapped_parents = self.parent_mappings.len();
        self.conversations.push(ImportedConversation {
            source_id: id.clone(),
            conversation_id: id,
            title,
            messages,
            branches: branches
                .into_iter()
                .map(|branch| ImportedBranch {
                    source_id: branch.id.clone(),
                    branch_id: branch.id,
                    leaf_node_id: Some(branch.leaf_node_id),
                    canonical: branch.canonical,
                })
                .collect(),
        });
        Ok(())
    }
}

type Tips<'a> = BTreeMap<&'a str, (usize, usize)>;

fn branch_tips<'a>(
    graph: &ParentGraph,
    branches: &'a [Branch],
) -> Result<Tips<'a>, NativeImportSourceError> {
    let mut tips = BTreeMap::new();
    let mut canonical = false;
    for (index, branch) in branches.iter().enumerate() {
        validate_id(&branch.id)?;
        let leaf = graph
            .by_id
            .get(&branch.leaf_node_id)
            .copied()
            .ok_or_else(|| invalid("source branch has a missing leaf"))?;
        if tips.insert(branch.id.as_str(), (leaf, index)).is_some() {
            return Err(invalid("duplicate source branch ID"));
        }
        if branch.canonical && std::mem::replace(&mut canonical, true) {
            return Err(invalid(
                "source conversation has multiple canonical branches",
            ));
        }
    }
    Ok(tips)
}

fn representative_branches(
    graph: &ParentGraph,
    branches: &[Branch],
    tips: &Tips<'_>,
) -> Result<Vec<usize>, NativeImportSourceError> {
    let mut representatives = vec![None; graph.order.len()];
    let key = |index: usize| (!branches[index].canonical, index);
    for &(leaf, branch) in tips.values() {
        if representatives[leaf].is_none_or(|current| key(branch) < key(current)) {
            representatives[leaf] = Some(branch);
        }
    }
    // A representative advances on just one path even when another branch shares its prefix.
    for &index in graph.order.iter().rev() {
        let branch = representatives[index]
            .ok_or_else(|| invalid("source turn is outside all declared branches"))?;
        if let Some(parent) = graph.parents[index]
            && representatives[parent].is_none_or(|current| key(branch) < key(current))
        {
            representatives[parent] = Some(branch);
        }
    }
    Ok(representatives.into_iter().flatten().collect())
}

#[path = "source_episodes.rs"]
mod episodes;
