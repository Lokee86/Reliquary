use crate::fragment_store::fragment_id;
use crate::{Archive, ArchiveError, Fragment, FragmentConfig};
use reliquary_container::Container;

impl Archive {
    pub fn materialize_branch_fragments(
        &mut self,
        container: &mut Container,
        conversation_id: &str,
        branch_id: &str,
        config: FragmentConfig,
        close_tail: bool,
    ) -> Result<Vec<Fragment>, ArchiveError> {
        let branch = self
            .branches
            .get(conversation_id, branch_id)
            .ok_or(ArchiveError::MissingBranch)?
            .clone();
        self.materialize_path_fragments(
            container,
            conversation_id,
            &branch.leaf_node_id,
            config,
            close_tail,
        )
    }

    pub fn materialize_path_fragments(
        &mut self,
        container: &mut Container,
        conversation_id: &str,
        leaf_node_id: &str,
        config: FragmentConfig,
        close_tail: bool,
    ) -> Result<Vec<Fragment>, ArchiveError> {
        let nodes = self.branch_nodes(conversation_id, leaf_node_id)?;
        let mut created = Vec::new();
        for fragment in path_fragment_windows(&nodes, config, close_tail)? {
            if self.put_fragment(container, fragment.clone())? {
                created.push(fragment);
            }
        }
        Ok(created)
    }
}

pub fn path_fragment_windows(
    nodes: &[crate::Node],
    config: FragmentConfig,
    close_tail: bool,
) -> Result<Vec<Fragment>, ArchiveError> {
    validate_config(config)?;
    let mut windows = Vec::new();
    if nodes.len() >= config.turns {
        let stride = config.turns - config.overlap;
        for start in (0..=nodes.len() - config.turns).step_by(stride) {
            windows.push(fragment_for(nodes, start, start + config.turns - 1));
        }
    }
    if close_tail && !nodes.is_empty() {
        let start = nodes.len().saturating_sub(config.turns);
        let tail = fragment_for(nodes, start, nodes.len() - 1);
        if windows.last().is_none_or(|existing| existing.id != tail.id) {
            windows.push(tail);
        }
    }
    Ok(windows)
}

fn fragment_for(nodes: &[crate::Node], start: usize, end: usize) -> Fragment {
    let first = &nodes[start];
    let last = &nodes[end];
    Fragment {
        id: fragment_id(&first.conversation_id, &first.id, &last.id),
        conversation_id: first.conversation_id.clone(),
        start_node_id: first.id.clone(),
        end_node_id: last.id.clone(),
    }
}

fn validate_config(config: FragmentConfig) -> Result<(), ArchiveError> {
    if config.turns == 0 || config.overlap >= config.turns {
        return Err(ArchiveError::InvalidFragmentConfig);
    }
    Ok(())
}
