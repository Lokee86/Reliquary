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
        let facts = crate::history::source_proof::timeline_facts(&self.archive);
        let sources = facts
            .sources
            .into_iter()
            .map(|node| HistoricalSource {
                key: SourceKey {
                    conversation_id: node.conversation_id,
                    node_id: node.id,
                },
                parent_id: node.parent_id,
                role: node.role,
                timestamp_ns: node.timestamp_ns,
            })
            .collect();
        let positions = facts
            .positions
            .into_iter()
            .map(|((conversation_id, node_id), position)| {
                (
                    SourceKey {
                        conversation_id,
                        node_id,
                    },
                    position,
                )
            })
            .collect();
        HistoricalTimeline::from_accepted_archive(sources, &positions, facts.activity_count)
    }
}

#[cfg(test)]
#[path = "historical_clock_tests.rs"]
mod historical_clock_tests;
