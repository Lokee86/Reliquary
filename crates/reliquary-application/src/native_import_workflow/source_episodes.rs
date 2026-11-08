use super::*;

impl ParentGraph {
    pub(super) fn derive_episodes(
        &self,
        turns: &[SourceTurn],
        representatives: &[usize],
    ) -> Vec<SourceEpisode> {
        let mut paths = BTreeMap::<usize, Vec<usize>>::new();
        for &index in &self.order {
            paths.entry(representatives[index]).or_default().push(index);
        }
        let mut episodes = Vec::new();
        for path in paths.values() {
            let mut remaining = path.as_slice();
            while !remaining.is_empty() {
                let mut bytes = 0;
                let count = remaining
                    .iter()
                    .take(EPISODE_TURNS)
                    .take_while(|&&index| {
                        let fits =
                            bytes == 0 || bytes + turns[index].content.len() <= EPISODE_BYTES;
                        bytes += turns[index].content.len();
                        fits
                    })
                    .count();
                episodes.push(SourceEpisode {
                    start: turns[remaining[0]].id.clone(),
                    end: turns[remaining[count - 1]].id.clone(),
                    turn_count: Some(count),
                });
                remaining = &remaining[count..];
            }
        }
        episodes
    }

    pub(super) fn check_episodes(
        &self,
        conversation_id: &str,
        turns: &[SourceTurn],
        episodes: Vec<SourceEpisode>,
    ) -> Result<Vec<NativeImportEpisode>, NativeImportSourceError> {
        let mut covered = vec![false; turns.len()];
        let mut validated = Vec::with_capacity(episodes.len());
        for episode in episodes {
            let &start = self
                .by_id
                .get(&episode.start)
                .ok_or_else(|| invalid("episode has a missing start node"))?;
            let &end = self
                .by_id
                .get(&episode.end)
                .ok_or_else(|| invalid("episode has a missing end node"))?;
            if !self.ancestor(start, end) {
                return Err(invalid(
                    "episode endpoints do not share the required ancestry",
                ));
            }
            let mut node = end;
            let mut count = 0;
            let mut bytes = 0;
            loop {
                if std::mem::replace(&mut covered[node], true) {
                    return Err(invalid("source episodes overlap on a visible turn"));
                }
                count += 1;
                bytes += turns[node].content.len();
                if node == start {
                    break;
                }
                node = self.parents[node].expect("checked episode ancestry");
            }
            if episode.turn_count.is_some_and(|declared| declared != count) {
                return Err(invalid(
                    "source episode turn count differs from its ancestry",
                ));
            }
            if count > EPISODE_TURNS || (count > 1 && bytes > EPISODE_BYTES) {
                return Err(NativeImportSourceError::Limit(
                    "source episode exceeds its text or turn budget",
                ));
            }
            validated.push(NativeImportEpisode {
                conversation_id: conversation_id.to_owned(),
                start_node_id: episode.start,
                end_node_id: episode.end,
                // The source cut is deterministic across reparse and restart.
                finalized_at_ns: turns[end].timestamp_ns.max(0),
            });
        }
        if covered.contains(&false) {
            return Err(invalid("source episode coverage omits visible turns"));
        }
        Ok(validated)
    }
}
