use crate::TemporalIndicationKind;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalInferenceResolution {
    pub start_byte: usize,
    pub end_byte: usize,
    pub kind: TemporalIndicationKind,
    pub evidence: String,
    pub canonical_expression: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalInference {
    pub model: String,
    pub contract_version: String,
    pub resolutions: Vec<TemporalInferenceResolution>,
}

impl From<TemporalInference> for reliquary_memory_core::TemporalInference {
    fn from(value: TemporalInference) -> Self {
        Self {
            model: value.model,
            contract_version: value.contract_version,
            resolutions: value
                .resolutions
                .into_iter()
                .map(|r| reliquary_memory_core::TemporalInferenceResolution {
                    start_byte: r.start_byte,
                    end_byte: r.end_byte,
                    kind: r.kind.into(),
                    evidence: r.evidence,
                    canonical_expression: r.canonical_expression,
                })
                .collect(),
        }
    }
}
impl From<TemporalIndicationKind> for reliquary_memory_core::TemporalIndicationKind {
    fn from(value: TemporalIndicationKind) -> Self {
        match value {
            TemporalIndicationKind::Explicit => Self::Explicit,
            TemporalIndicationKind::Calendar => Self::Calendar,
            TemporalIndicationKind::Relative => Self::Relative,
            TemporalIndicationKind::Boundary => Self::Boundary,
            TemporalIndicationKind::Recurrence => Self::Recurrence,
            TemporalIndicationKind::Duration => Self::Duration,
        }
    }
}

impl From<reliquary_memory_core::TemporalInference> for TemporalInference {
    fn from(value: reliquary_memory_core::TemporalInference) -> Self {
        Self {
            model: value.model,
            contract_version: value.contract_version,
            resolutions: value
                .resolutions
                .into_iter()
                .map(|r| TemporalInferenceResolution {
                    start_byte: r.start_byte,
                    end_byte: r.end_byte,
                    kind: r.kind.into(),
                    evidence: r.evidence,
                    canonical_expression: r.canonical_expression,
                })
                .collect(),
        }
    }
}
impl From<reliquary_memory_core::TemporalIndicationKind> for TemporalIndicationKind {
    fn from(value: reliquary_memory_core::TemporalIndicationKind) -> Self {
        match value {
            reliquary_memory_core::TemporalIndicationKind::Explicit => Self::Explicit,
            reliquary_memory_core::TemporalIndicationKind::Calendar => Self::Calendar,
            reliquary_memory_core::TemporalIndicationKind::Relative => Self::Relative,
            reliquary_memory_core::TemporalIndicationKind::Boundary => Self::Boundary,
            reliquary_memory_core::TemporalIndicationKind::Recurrence => Self::Recurrence,
            reliquary_memory_core::TemporalIndicationKind::Duration => Self::Duration,
        }
    }
}
