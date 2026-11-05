use super::*;

/// Hard ceilings for owner-authorized evidence reads. Search reports whether
/// its bounded prefix may have omitted later matches.
pub const MAX_EVIDENCE_SEARCH_SCAN: usize = 4096;
pub const MAX_EVIDENCE_SEARCH_BYTES: usize = 1 << 20;
pub const MAX_EVIDENCE_TURNS: usize = 64;
pub const MAX_EVIDENCE_RESULT_BYTES: usize = 128 << 10;
pub const MAX_EVIDENCE_SEARCH_RESULTS: usize = 5;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveSearchResult {
    pub turns: Vec<(String, ResolvedTurn)>,
    pub truncated: bool,
}

impl Archive {
    /// Rehydrate an Episode only when both its turn and text budgets fit.
    /// Nothing is silently omitted from an Episode range.
    pub fn bounded_episode_turns(
        &self,
        id: EpisodeId,
        max_turns: usize,
        max_bytes: usize,
    ) -> Result<Option<Vec<ResolvedTurn>>, ArchiveError> {
        let Some(episode) = self.episodes.get(&id) else {
            return Ok(None);
        };
        let selected = self.bounded_ancestor_range(
            &episode.conversation_id,
            &episode.start_node_id,
            &episode.end_node_id,
            max_turns,
            max_bytes,
        )?;
        self.resolve_path(selected).map(Some)
    }

    /// Rehydrate an exact ancestor-to-descendant node range in one conversation.
    pub fn bounded_conversation_range(
        &self,
        conversation_id: &str,
        start_node_id: &str,
        end_node_id: &str,
        max_turns: usize,
        max_bytes: usize,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        let selected = self.bounded_ancestor_range(
            conversation_id,
            start_node_id,
            end_node_id,
            max_turns,
            max_bytes,
        )?;
        self.resolve_path(selected)
    }

    /// Search accepted turns in first-accept order with a fixed scan and output
    /// budget. Results are restricted to the requested conversation when set.
    pub fn bounded_search(
        &self,
        conversation_id: Option<&str>,
        query: &str,
        limit: usize,
        through_timestamp_ns: Option<i64>,
    ) -> Result<ArchiveSearchResult, ArchiveError> {
        if query.trim().is_empty()
            || query.len() > 512
            || limit == 0
            || limit > MAX_EVIDENCE_SEARCH_RESULTS
            || conversation_id.is_some_and(|id| id.trim().is_empty())
        {
            return Err(ArchiveError::InvalidField("evidence search"));
        }
        let query = query.to_lowercase();
        let mut turns = Vec::new();
        let mut scanned = 0usize;
        let mut scanned_bytes = 0usize;
        let mut returned_bytes = 0usize;
        let mut truncated = false;
        for key in &self.accepted_node_order {
            if scanned == MAX_EVIDENCE_SEARCH_SCAN {
                truncated = true;
                break;
            }
            scanned += 1;
            let Some(node) = self.nodes.get(key) else {
                continue;
            };
            if conversation_id.is_some_and(|id| id != node.conversation_id) {
                continue;
            }
            if through_timestamp_ns.is_some_and(|cutoff| node.timestamp_ns > cutoff) {
                continue;
            }
            let content = self
                .content
                .get(&node.content_id)
                .ok_or(ArchiveError::MissingContent)?;
            let next_scan_bytes = scanned_bytes
                .checked_add(content.len())
                .ok_or(ArchiveError::EvidenceLimitExceeded)?;
            if next_scan_bytes > MAX_EVIDENCE_SEARCH_BYTES {
                truncated = true;
                break;
            }
            scanned_bytes = next_scan_bytes;
            if !content.to_lowercase().contains(&query) {
                continue;
            }
            if turns.len() == limit {
                truncated = true;
                break;
            }
            let next_returned_bytes = returned_bytes
                .checked_add(content.len())
                .ok_or(ArchiveError::EvidenceLimitExceeded)?;
            if next_returned_bytes > MAX_EVIDENCE_RESULT_BYTES {
                truncated = true;
                break;
            }
            returned_bytes = next_returned_bytes;
            turns.push((
                node.conversation_id.clone(),
                ResolvedTurn {
                    node_id: node.id.clone(),
                    role: node.role.clone(),
                    principal_id: node.principal_id.clone(),
                    timestamp_ns: node.timestamp_ns,
                    content: content.clone(),
                },
            ));
        }
        Ok(ArchiveSearchResult { turns, truncated })
    }

    /// Resolve one accepted turn only when its content fits the caller's byte budget.
    pub fn bounded_resolved_turn(
        &self,
        conversation_id: &str,
        node_id: &str,
        max_bytes: usize,
    ) -> Result<Option<ResolvedTurn>, ArchiveError> {
        let Some(node) = self.node(conversation_id, node_id) else {
            return Ok(None);
        };
        if max_bytes > MAX_EVIDENCE_RESULT_BYTES {
            return Err(ArchiveError::EvidenceLimitExceeded);
        }
        let content = self
            .content
            .get(&node.content_id)
            .ok_or(ArchiveError::MissingContent)?;
        if content.len() > max_bytes {
            return Err(ArchiveError::EvidenceLimitExceeded);
        }
        Ok(Some(ResolvedTurn {
            node_id: node.id.clone(),
            role: node.role.clone(),
            principal_id: node.principal_id.clone(),
            timestamp_ns: node.timestamp_ns,
            content: content.clone(),
        }))
    }

    fn bounded_ancestor_range(
        &self,
        conversation_id: &str,
        start_node_id: &str,
        end_node_id: &str,
        max_turns: usize,
        max_bytes: usize,
    ) -> Result<Vec<&Node>, ArchiveError> {
        if max_turns > MAX_EVIDENCE_TURNS || max_bytes > MAX_EVIDENCE_RESULT_BYTES {
            return Err(ArchiveError::EvidenceLimitExceeded);
        }
        let mut reverse = Vec::with_capacity(max_turns.min(8));
        let mut current = Some(end_node_id);
        while let Some(node_id) = current {
            let node = self.require_node(conversation_id, node_id)?;
            reverse.push(node);
            if node.id == start_node_id {
                reverse.reverse();
                validate_evidence_nodes(&reverse, &self.content, max_turns, max_bytes)?;
                return Ok(reverse);
            }
            if reverse.len() > max_turns {
                return Err(ArchiveError::EvidenceLimitExceeded);
            }
            current = node.parent_id.as_deref();
        }
        Err(ArchiveError::InvalidEpisodeRange)
    }
}

fn validate_evidence_nodes(
    nodes: &[&Node],
    content: &HashMap<ContentId, String>,
    max_turns: usize,
    max_bytes: usize,
) -> Result<(), ArchiveError> {
    if max_turns > MAX_EVIDENCE_TURNS
        || nodes.len() > max_turns
        || max_bytes > MAX_EVIDENCE_RESULT_BYTES
    {
        return Err(ArchiveError::EvidenceLimitExceeded);
    }
    let mut bytes = 0usize;
    for node in nodes {
        let text = content
            .get(&node.content_id)
            .ok_or(ArchiveError::MissingContent)?;
        bytes = bytes
            .checked_add(text.len())
            .ok_or(ArchiveError::EvidenceLimitExceeded)?;
        if bytes > max_bytes {
            return Err(ArchiveError::EvidenceLimitExceeded);
        }
    }
    Ok(())
}
