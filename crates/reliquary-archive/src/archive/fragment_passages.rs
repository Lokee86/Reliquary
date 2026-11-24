//! Source-bound passages for Archive windows that exceed the embedding budget.
use super::*;
use crate::{Fragment, FragmentDefinition, FragmentId, FragmentPassage};
use std::{collections::BTreeSet, ops::Range};

/// UTF-8 bytes, including role labels and separators in the embedding input.
const PASSAGE_BYTES: usize = 12_000;

impl Archive {
    /// Publish all passages for the supplied, already accepted fragment windows.
    pub fn define_fragment_passages(
        &mut self,
        container: &mut Container,
        parents: &[FragmentId],
    ) -> Result<usize, ArchiveError> {
        self.bind_container(container)?;
        let mut proposed = Vec::new();
        for id in parents.iter().copied().collect::<BTreeSet<_>>() {
            let parent = self.fragment_definition(id).ok_or(invalid_passage())?;
            if parent.fragment.passage.is_some()
                || self
                    .conversation_tombstones
                    .contains_key(&parent.fragment.conversation_id)
            {
                return Err(invalid_passage());
            }
            for fragment in self.passage_windows(&parent.fragment)? {
                proposed.push(FragmentDefinition {
                    fragment,
                    branch_id: parent.branch_id.clone(),
                    archive_cut: self.archive_version,
                    config: parent.config,
                    close_tail: parent.close_tail,
                });
            }
        }
        let before = self.fragment_definitions.len();
        self.put_fragment_definitions(container, proposed)?;
        Ok(self.fragment_definitions.len() - before)
    }

    pub fn fragment_passage_definitions(&self) -> impl Iterator<Item = &FragmentDefinition> {
        self.fragment_definitions
            .values()
            .filter(|definition| definition.fragment.passage.is_some())
    }

    fn passage_windows(&self, parent: &Fragment) -> Result<Vec<Fragment>, ArchiveError> {
        let nodes = self.fragment_nodes(parent)?;
        let slices = nodes
            .iter()
            .map(|node| {
                self.content_length_for_node(&node.conversation_id, &node.id)
                    .ok_or(ArchiveError::MissingContent)
                    .map(|bytes| (node.role.len() + 2, bytes))
            })
            .collect::<Result<Vec<_>, ArchiveError>>()?;
        let source_bytes = slices
            .iter()
            .map(|(role, bytes)| role + bytes)
            .sum::<usize>()
            + nodes.len().saturating_sub(1);
        if source_bytes <= PASSAGE_BYTES {
            return Ok(Vec::new());
        }

        let mut output = Vec::new();
        let mut group_start = 0;
        let mut group_bytes = 0;
        for (index, (&node, &(label_bytes, bytes))) in nodes.iter().zip(&slices).enumerate() {
            let turn_bytes = label_bytes + bytes;
            if group_bytes != 0 && group_bytes + 1 + turn_bytes > PASSAGE_BYTES {
                output.push(passage(
                    parent,
                    nodes[group_start],
                    nodes[index - 1],
                    0,
                    slices[index - 1].1,
                ));
                group_bytes = 0;
            }
            if turn_bytes <= PASSAGE_BYTES {
                if group_bytes == 0 {
                    group_start = index;
                }
                group_bytes += usize::from(group_bytes != 0) + turn_bytes;
                continue;
            }

            let budget = PASSAGE_BYTES
                .checked_sub(label_bytes)
                .filter(|&bytes| bytes >= 4)
                .ok_or(ArchiveError::EvidenceLimitExceeded)?;
            let text = self
                .content_for_node(&node.conversation_id, &node.id)?
                .ok_or(ArchiveError::MissingContent)?;
            let mut start = 0;
            while start < text.len() {
                let end = start + preferred_boundary(&text[start..], budget);
                output.push(passage(parent, node, node, start, end));
                start = end;
            }
        }
        if group_bytes > 0 {
            let index = nodes.len() - 1;
            output.push(passage(
                parent,
                nodes[group_start],
                nodes[index],
                0,
                slices[index].1,
            ));
        }
        Ok(output)
    }

    pub(super) fn validate_passage_definition(
        &self,
        definition: &FragmentDefinition,
    ) -> Result<(), ArchiveError> {
        let fragment = &definition.fragment;
        let span = fragment.passage.ok_or(invalid_passage())?;
        let parent = self
            .fragment_definition(span.parent_id)
            .ok_or(invalid_passage())?;
        if parent.fragment.passage.is_some()
            || parent.fragment.conversation_id != fragment.conversation_id
            || parent.branch_id != definition.branch_id
            || parent.config != definition.config
            || parent.close_tail != definition.close_tail
            || parent.archive_cut >= definition.archive_cut
            || span_id(fragment) != fragment.id
        {
            return Err(invalid_passage());
        }
        let parent_nodes = self.fragment_nodes(&parent.fragment)?;
        let start = parent_nodes
            .iter()
            .position(|node| node.id == fragment.start_node_id)
            .ok_or(invalid_passage())?;
        let end = parent_nodes
            .iter()
            .position(|node| node.id == fragment.end_node_id)
            .filter(|end| *end >= start)
            .ok_or(invalid_passage())?;
        let nodes = &parent_nodes[start..=end];
        let mut rendered_bytes = nodes.len() - 1;
        for (index, node) in nodes.iter().enumerate() {
            let Some(text) = self.content_for_node(&node.conversation_id, &node.id)? else {
                // A terminal-prune copy retains source identities and proofs
                // while deliberately dropping the original payload frames.
                if !self
                    .turns
                    .get(&node.conversation_id, &node.id)
                    .is_some_and(|turn| turn.prune.is_some())
                {
                    return Err(ArchiveError::MissingContent);
                }
                continue;
            };
            let range = source_range(fragment, index, nodes.len(), text.len())?;
            if text.get(range.clone()).is_none() {
                return Err(invalid_passage());
            }
            rendered_bytes += node.role.len() + 2 + range.len();
        }
        if rendered_bytes > PASSAGE_BYTES {
            return Err(invalid_passage());
        }
        Ok(())
    }

    /// Render the exact passage or whole fragment used by Archive embeddings.
    pub fn fragment_embedding_text(
        &self,
        fragment: &Fragment,
        max_bytes: usize,
    ) -> Result<String, ArchiveError> {
        let turns = self.fragment_source_text(fragment, max_bytes)?;
        let mut output = String::new();
        for (node, body) in turns {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&node.role);
            output.push_str(": ");
            output.push_str(&body);
            if output.len() > max_bytes {
                return Err(ArchiveError::EvidenceLimitExceeded);
            }
        }
        Ok(output)
    }

    pub(super) fn fragment_source_text(
        &self,
        fragment: &Fragment,
        max_bytes: usize,
    ) -> Result<Vec<(&Node, Arc<String>)>, ArchiveError> {
        let nodes = self.fragment_nodes(fragment)?;
        let mut source = Vec::with_capacity(nodes.len());
        let mut bytes = 0usize;
        for (index, node) in nodes.iter().enumerate() {
            let length = self
                .content_length_for_node(&node.conversation_id, &node.id)
                .ok_or(ArchiveError::MissingContent)?;
            let range = source_range(fragment, index, nodes.len(), length)?;
            bytes = bytes
                .checked_add(range.len())
                .ok_or(ArchiveError::EvidenceLimitExceeded)?;
            if bytes > max_bytes {
                return Err(ArchiveError::EvidenceLimitExceeded);
            }
            let text = self
                .content_for_node(&node.conversation_id, &node.id)?
                .ok_or(ArchiveError::MissingContent)?;
            let text = if range.start == 0 && range.end == length {
                text
            } else {
                Arc::new(text.get(range).ok_or(invalid_passage())?.to_owned())
            };
            source.push((*node, text));
        }
        Ok(source)
    }

    fn fragment_nodes(&self, fragment: &Fragment) -> Result<Vec<&Node>, ArchiveError> {
        self.ancestor_range(
            &fragment.conversation_id,
            &fragment.start_node_id,
            &fragment.end_node_id,
            MAX_EVIDENCE_TURNS,
        )
    }
}

fn source_range(
    fragment: &Fragment,
    index: usize,
    count: usize,
    length: usize,
) -> Result<Range<usize>, ArchiveError> {
    let Some(span) = fragment.passage else {
        return Ok(0..length);
    };
    let mut range = 0..length;
    if index == 0 {
        range.start = usize::try_from(span.start_byte).map_err(|_| invalid_passage())?;
    }
    if index + 1 == count {
        range.end = usize::try_from(span.end_byte).map_err(|_| invalid_passage())?;
    }
    if range.start > range.end || range.end > length {
        return Err(invalid_passage());
    }
    Ok(range)
}

fn preferred_boundary(text: &str, budget: usize) -> usize {
    if text.len() <= budget {
        return text.len();
    }
    let mut end = budget;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end]
        .char_indices()
        .rev()
        .take_while(|(offset, _)| *offset >= end * 3 / 4)
        .find_map(|(offset, c)| c.is_whitespace().then_some(offset + c.len_utf8()))
        .unwrap_or(end)
}

fn passage(parent: &Fragment, first: &Node, last: &Node, start: usize, end: usize) -> Fragment {
    let mut fragment = Fragment {
        id: FragmentId([0; 32]),
        conversation_id: parent.conversation_id.clone(),
        start_node_id: first.id.clone(),
        end_node_id: last.id.clone(),
        passage: Some(FragmentPassage {
            parent_id: parent.id,
            start_byte: start as u64,
            end_byte: end as u64,
        }),
    };
    fragment.id = span_id(&fragment);
    fragment
}

fn span_id(fragment: &Fragment) -> FragmentId {
    let span = fragment.passage.expect("passage source");
    let mut hash = Sha256::new();
    hash.update(b"REL Archive passage v1\0");
    hash.update(span.parent_id.0);
    for id in [&fragment.start_node_id, &fragment.end_node_id] {
        hash.update((id.len() as u64).to_le_bytes());
        hash.update(id.as_bytes());
    }
    hash.update(span.start_byte.to_le_bytes());
    hash.update(span.end_byte.to_le_bytes());
    FragmentId(hash.finalize().into())
}

fn invalid_passage() -> ArchiveError {
    ArchiveError::CorruptRecord("invalid Archive fragment passage")
}
