//! Iterative parent-graph validation and constant-time ancestry checks.

use super::*;
use std::collections::BTreeMap;

pub(super) struct ParentGraph {
    pub by_id: BTreeMap<String, usize>,
    pub parents: Vec<Option<usize>>,
    pub order: Vec<usize>,
    pub leaves: Vec<usize>,
    child_counts: Vec<usize>,
    spans: Vec<(usize, usize)>,
}

impl ParentGraph {
    pub fn new<'a>(
        nodes: impl Iterator<Item = (&'a str, Option<&'a str>)> + Clone,
    ) -> Result<Self, NativeImportSourceError> {
        let mut by_id = BTreeMap::new();
        for (index, (id, _)) in nodes.clone().enumerate() {
            validate_id(id)?;
            if by_id.insert(id.to_owned(), index).is_some() {
                return Err(invalid("duplicate source node ID"));
            }
        }
        let mut parents = Vec::with_capacity(by_id.len());
        let mut children = vec![Vec::new(); by_id.len()];
        let mut ready = BTreeSet::new();
        for (index, (_, parent)) in nodes.enumerate() {
            let parent = parent
                .map(|id| {
                    by_id
                        .get(id)
                        .copied()
                        .ok_or_else(|| invalid("source node has a missing parent"))
                })
                .transpose()?;
            if let Some(parent) = parent {
                children[parent].push(index);
            } else {
                ready.insert(index);
            }
            parents.push(parent);
        }
        let roots: Vec<_> = ready.iter().copied().collect();
        let mut order = Vec::with_capacity(by_id.len());
        while let Some(index) = ready.pop_first() {
            order.push(index);
            ready.extend(children[index].iter().copied());
        }
        if order.len() != parents.len() {
            return Err(invalid("source node ancestry contains a cycle"));
        }
        let mut spans = vec![(0, 0); order.len()];
        let mut clock = 0;
        let mut stack: Vec<_> = roots.into_iter().map(|root| (root, false)).collect();
        while let Some((node, exiting)) = stack.pop() {
            if exiting {
                spans[node].1 = clock;
            } else {
                spans[node].0 = clock;
                clock += 1;
                stack.push((node, true));
                stack.extend(children[node].iter().map(|child| (*child, false)));
            }
        }
        let leaves = children
            .iter()
            .enumerate()
            .filter(|(_, list)| list.is_empty())
            .map(|(index, _)| index)
            .collect();
        let child_counts = children.iter().map(Vec::len).collect();
        Ok(Self {
            by_id,
            parents,
            order,
            leaves,
            child_counts,
            spans,
        })
    }

    pub(super) fn child_count(&self, node: usize) -> usize {
        self.child_counts[node]
    }

    pub(super) fn ancestor(&self, node: usize, leaf: usize) -> bool {
        (self.spans[node].0..self.spans[node].1).contains(&self.spans[leaf].0)
    }
}
