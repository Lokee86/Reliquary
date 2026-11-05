//! Deterministic source fragment windows over an accepted Archive branch.
use crate::{ArchiveError, Node};
use sha2::{Digest, Sha256};

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct FragmentId(pub [u8; 32]);

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Fragment {
    pub id: FragmentId,
    pub conversation_id: String,
    pub start_node_id: String,
    pub end_node_id: String,
}

/// Immutable proof that a deterministic fragment was published from one
/// accepted branch cut. The cut excludes this definition record itself.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FragmentDefinition {
    pub fragment: Fragment,
    pub branch_id: String,
    pub archive_cut: u64,
    pub config: FragmentConfig,
    pub close_tail: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FragmentConfig {
    pub turns: usize,
    pub overlap: usize,
}

impl Default for FragmentConfig {
    fn default() -> Self {
        Self {
            turns: 8,
            overlap: 2,
        }
    }
}

/// Stable identity retained from the reference implementation.
pub fn fragment_id(conversation: &str, start: &str, end: &str) -> FragmentId {
    let mut hash = Sha256::new();
    for value in [conversation, start, end] {
        hash.update(value.as_bytes());
        hash.update([0]);
    }
    FragmentId(hash.finalize().into())
}

/// Derive overlapping windows from one ordered, contiguous Archive path.
pub fn path_fragment_windows(
    nodes: &[Node],
    config: FragmentConfig,
    close_tail: bool,
) -> Result<Vec<Fragment>, ArchiveError> {
    if config.turns == 0 || config.overlap >= config.turns {
        return Err(ArchiveError::InvalidField("fragment configuration"));
    }
    if nodes.windows(2).any(|pair| {
        pair[0].conversation_id != pair[1].conversation_id
            || pair[1].parent_id.as_deref() != Some(pair[0].id.as_str())
    }) {
        return Err(ArchiveError::InvalidField("fragment path"));
    }
    let fragment = |start: usize, end: usize| {
        let first = &nodes[start];
        let last = &nodes[end];
        Fragment {
            id: fragment_id(&first.conversation_id, &first.id, &last.id),
            conversation_id: first.conversation_id.clone(),
            start_node_id: first.id.clone(),
            end_node_id: last.id.clone(),
        }
    };
    let mut windows = Vec::new();
    if nodes.len() >= config.turns {
        for start in (0..=nodes.len() - config.turns).step_by(config.turns - config.overlap) {
            windows.push(fragment(start, start + config.turns - 1));
        }
    }
    if close_tail && !nodes.is_empty() {
        let tail = fragment(nodes.len().saturating_sub(config.turns), nodes.len() - 1);
        if windows.last().is_none_or(|existing| existing.id != tail.id) {
            windows.push(tail);
        }
    }
    Ok(windows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContentId;
    fn node(id: &str, parent: Option<&str>) -> Node {
        Node {
            id: id.into(),
            conversation_id: "c".into(),
            parent_id: parent.map(str::to_owned),
            role: "user".into(),
            principal_id: None,
            timestamp_ns: 1,
            content_id: ContentId([0; 32]),
        }
    }
    #[test]
    fn windows_follow_reference_overlap_and_close_tail_without_duplicates() {
        let nodes = vec![
            node("a", None),
            node("b", Some("a")),
            node("c", Some("b")),
            node("d", Some("c")),
            node("e", Some("d")),
        ];
        let config = FragmentConfig {
            turns: 3,
            overlap: 1,
        };
        let windows = path_fragment_windows(&nodes, config, true).unwrap();
        assert_eq!(
            windows
                .iter()
                .map(|f| (f.start_node_id.as_str(), f.end_node_id.as_str()))
                .collect::<Vec<_>>(),
            [("a", "c"), ("c", "e")]
        );
        assert_eq!(
            path_fragment_windows(&nodes[..4], config, true)
                .unwrap()
                .iter()
                .map(|f| (f.start_node_id.as_str(), f.end_node_id.as_str()))
                .collect::<Vec<_>>(),
            [("a", "c"), ("b", "d")]
        );
        assert!(
            path_fragment_windows(&nodes[..2], config, false)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            path_fragment_windows(&nodes[..2], config, true)
                .unwrap()
                .len(),
            1
        );
        assert!(
            path_fragment_windows(
                &nodes,
                FragmentConfig {
                    turns: 1,
                    overlap: 1
                },
                false
            )
            .is_err()
        );
        assert!(path_fragment_windows(&[node("a", None), node("b", None)], config, true).is_err());
    }
    #[test]
    fn stable_fragment_id_matches_reference_golden() {
        assert_eq!(
            fragment_id("c", "a", "b").0,
            [
                163, 150, 195, 191, 138, 232, 187, 26, 132, 43, 211, 23, 127, 43, 178, 14, 111, 21,
                23, 200, 204, 32, 2, 32, 247, 88, 41, 42, 74, 216, 53, 232
            ]
        );
    }
}
