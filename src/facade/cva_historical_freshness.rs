use crate::Cva;

impl Cva {
    /// Plans the complete retained historical corpus only when its canonical
    /// chronology is identical to Archive's first-accepted REL activity clock.
    /// This read-only gate neither rebases existing cuts nor publishes scores.
    pub fn historical_freshness_timeline(
        &self,
    ) -> Result<
        crate::freshness::historical_planner::HistoricalTimeline,
        crate::freshness::historical_planner::PlannerError,
    > {
        use crate::freshness::historical_planner::{
            HistoricalSource, HistoricalTimeline, SourceKey,
        };
        let sources = self
            .archive
            .nodes
            .iter()
            .map(|node| HistoricalSource {
                key: SourceKey {
                    conversation_id: node.conversation_id.clone(),
                    node_id: node.id.clone(),
                },
                parent_id: node.parent_id.clone(),
                role: node.role.clone(),
                timestamp_ns: node.timestamp_ns,
            })
            .collect();
        let timeline = HistoricalTimeline::plan(sources)?;
        let positions = timeline
            .activity_order
            .iter()
            .filter_map(|key| {
                self.archive
                    .activity_position_for_turn(&key.conversation_id, &key.node_id)
                    .map(|position| (key.clone(), position))
            })
            .collect();
        timeline.validate_archive_clock(&positions, self.archive.rel_turn_count())?;
        Ok(timeline)
    }
}

#[cfg(test)]
#[path = "historical_clock_tests.rs"]
mod historical_clock_tests;
