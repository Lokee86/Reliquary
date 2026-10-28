use crate::{TemporalAnalysis, TemporalDetection, TemporalIndication};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TemporalResolutionStatus {
    NoTemporalMaterial,
    FullyResolved,
    Unresolved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalResolution {
    pub status: TemporalResolutionStatus,
    pub unresolved_indications: Vec<TemporalIndication>,
}

impl TemporalResolution {
    pub fn needs_inference(&self) -> bool {
        self.status == TemporalResolutionStatus::Unresolved
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalAssessment {
    pub detection: TemporalDetection,
    pub analysis: TemporalAnalysis,
    pub resolution: TemporalResolution,
}
