use crate::{
    Community, CommunityId, CommunityLineageLink, CommunityLineageTransition,
    CommunitySemanticName, CommunitySemanticNameSource, CommunitySnapshot,
    DREAM_COMMUNITY_NAME_RETAIN_JACCARD_PERMILLE,
};
use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Clone, Copy)]
struct Match {
    predecessor: CommunityId,
    successor: CommunityId,
    intersection: usize,
    predecessor_members: usize,
    successor_members: usize,
}

impl Match {
    fn union(self) -> usize {
        self.predecessor_members + self.successor_members - self.intersection
    }
}

#[derive(Clone, Copy)]
struct Best {
    index: usize,
    tied: bool,
}

pub(crate) fn derive_lineage(
    previous: &CommunitySnapshot,
    current: &CommunitySnapshot,
) -> CommunityLineageTransition {
    let mut matches = Vec::new();
    for predecessor in &previous.communities {
        for successor in &current.communities {
            let intersection = intersection_count(predecessor, successor);
            if intersection > 0 {
                matches.push(Match {
                    predecessor: predecessor.id,
                    successor: successor.id,
                    intersection,
                    predecessor_members: predecessor.members.len(),
                    successor_members: successor.members.len(),
                });
            }
        }
    }

    let predecessor_degree = degrees(&matches, |entry| entry.predecessor);
    let successor_degree = degrees(&matches, |entry| entry.successor);
    let predecessor_best = best_matches(&matches, true);
    let successor_best = best_matches(&matches, false);

    let mut links = matches
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let predecessor_choice = predecessor_best.get(&entry.predecessor);
            let successor_choice = successor_best.get(&entry.successor);
            let continuation = predecessor_choice
                .is_some_and(|choice| choice.index == index && !choice.tied)
                && successor_choice.is_some_and(|choice| choice.index == index && !choice.tied);
            CommunityLineageLink {
                predecessor: entry.predecessor,
                successor: entry.successor,
                intersection: entry.intersection,
                predecessor_members: entry.predecessor_members,
                successor_members: entry.successor_members,
                continuation,
                split: predecessor_degree
                    .get(&entry.predecessor)
                    .copied()
                    .unwrap_or(0)
                    > 1,
                merge: successor_degree.get(&entry.successor).copied().unwrap_or(0) > 1,
            }
        })
        .collect::<Vec<_>>();
    links.sort_by_key(|link| (link.predecessor, link.successor));

    let mut new_communities = current
        .communities
        .iter()
        .filter(|community| !successor_degree.contains_key(&community.id))
        .map(|community| community.id)
        .collect::<Vec<_>>();
    let mut ended_communities = previous
        .communities
        .iter()
        .filter(|community| !predecessor_degree.contains_key(&community.id))
        .map(|community| community.id)
        .collect::<Vec<_>>();
    new_communities.sort();
    ended_communities.sort();

    CommunityLineageTransition {
        from_generation: previous.generation,
        to_generation: current.generation,
        links,
        new_communities,
        ended_communities,
    }
}

pub(crate) fn resolve_semantic_name(
    snapshots: &[CommunitySnapshot],
    names: &HashMap<CommunityId, CommunitySemanticName>,
    community_id: CommunityId,
) -> Option<CommunitySemanticName> {
    if let Some(record) = names.get(&community_id) {
        return Some(record.clone());
    }

    let target_snapshot_index = snapshots
        .iter()
        .rposition(|snapshot| community(snapshot, community_id).is_some())?;
    let target = community(&snapshots[target_snapshot_index], community_id)?;
    let mut successor_id = community_id;

    for current_index in (1..=target_snapshot_index).rev() {
        let previous = &snapshots[current_index - 1];
        let current = &snapshots[current_index];
        let transition = derive_lineage(previous, current);
        let link = transition
            .links
            .iter()
            .find(|link| link.successor == successor_id && link.continuation)?;
        let predecessor_id = link.predecessor;

        if let Some(record) = names.get(&predecessor_id) {
            if record.source == CommunitySemanticNameSource::User {
                return Some(inherited(record, community_id));
            }
            let baseline = find_community(snapshots, record.baseline_community_id)?;
            if jaccard_per_mille(baseline, target) >= DREAM_COMMUNITY_NAME_RETAIN_JACCARD_PERMILLE {
                return Some(inherited(record, community_id));
            }
            return None;
        }

        successor_id = predecessor_id;
    }
    None
}

pub(crate) fn latest_lineage(
    snapshots: &[CommunitySnapshot],
) -> Option<CommunityLineageTransition> {
    let [.., previous, current] = snapshots else {
        return None;
    };
    Some(derive_lineage(previous, current))
}

fn inherited(record: &CommunitySemanticName, community_id: CommunityId) -> CommunitySemanticName {
    let mut inherited = record.clone();
    inherited.community_id = community_id;
    inherited
}

fn find_community(
    snapshots: &[CommunitySnapshot],
    community_id: CommunityId,
) -> Option<&Community> {
    snapshots
        .iter()
        .rev()
        .find_map(|snapshot| community(snapshot, community_id))
}

fn community(snapshot: &CommunitySnapshot, community_id: CommunityId) -> Option<&Community> {
    snapshot
        .communities
        .iter()
        .find(|community| community.id == community_id)
}

fn jaccard_per_mille(left: &Community, right: &Community) -> u32 {
    let intersection = intersection_count(left, right);
    let union = left.members.len() + right.members.len() - intersection;
    if union == 0 {
        return 0;
    }
    (((intersection as u128) * 1_000) / union as u128) as u32
}

fn intersection_count(left: &Community, right: &Community) -> usize {
    let mut left_index = 0;
    let mut right_index = 0;
    let mut count = 0;
    while left_index < left.members.len() && right_index < right.members.len() {
        match left.members[left_index].cmp(&right.members[right_index]) {
            Ordering::Less => left_index += 1,
            Ordering::Greater => right_index += 1,
            Ordering::Equal => {
                count += 1;
                left_index += 1;
                right_index += 1;
            }
        }
    }
    count
}

fn degrees(matches: &[Match], key: impl Fn(&Match) -> CommunityId) -> HashMap<CommunityId, usize> {
    let mut output = HashMap::new();
    for entry in matches {
        *output.entry(key(entry)).or_insert(0) += 1;
    }
    output
}

fn best_matches(matches: &[Match], by_predecessor: bool) -> HashMap<CommunityId, Best> {
    let mut output = HashMap::new();
    for (index, entry) in matches.iter().enumerate() {
        let key = if by_predecessor {
            entry.predecessor
        } else {
            entry.successor
        };
        output
            .entry(key)
            .and_modify(
                |best: &mut Best| match compare_jaccard(*entry, matches[best.index]) {
                    Ordering::Greater => *best = Best { index, tied: false },
                    Ordering::Equal => best.tied = true,
                    Ordering::Less => {}
                },
            )
            .or_insert(Best { index, tied: false });
    }
    output
}

fn compare_jaccard(left: Match, right: Match) -> Ordering {
    ((left.intersection as u128) * (right.union() as u128))
        .cmp(&((right.intersection as u128) * (left.union() as u128)))
}
