use crate::Cva;

impl Cva {
    /// Consumes the complete retained corpus in Archive's first-accepted clock.
    /// Timestamps remain provenance and cannot reorder accepted positions.
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
        let positions = self
            .archive
            .nodes
            .iter()
            .filter(|node| matches!(node.role.as_str(), "user" | "assistant"))
            .map(|node| SourceKey {
                conversation_id: node.conversation_id.clone(),
                node_id: node.id.clone(),
            })
            .filter_map(|key| {
                self.archive
                    .activity_position_for_turn(&key.conversation_id, &key.node_id)
                    .map(|position| (key.clone(), position))
            })
            .collect();
        HistoricalTimeline::from_accepted_archive(
            sources,
            &positions,
            self.archive.rel_turn_count(),
        )
    }
}

#[cfg(test)]
#[path = "historical_clock_tests.rs"]
mod historical_clock_tests;
