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
        crate::history::source_proof::historical_freshness_timeline(&self.archive)
    }
}

#[cfg(test)]
#[path = "historical_clock_tests.rs"]
mod historical_clock_tests;
