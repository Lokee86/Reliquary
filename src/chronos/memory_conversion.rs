use crate::{TemporalInference, TemporalInferenceResolution};

pub(crate) fn to_memory_inference(
    value: TemporalInference,
) -> reliquary_memory_core::TemporalInference {
    reliquary_memory_core::TemporalInference {
        model: value.model,
        contract_version: value.contract_version,
        resolutions: value
            .resolutions
            .into_iter()
            .map(
                |resolution| reliquary_memory_core::TemporalInferenceResolution {
                    start_byte: resolution.start_byte,
                    end_byte: resolution.end_byte,
                    kind: to_memory_kind(resolution.kind),
                    evidence: resolution.evidence,
                    canonical_expression: resolution.canonical_expression,
                },
            )
            .collect(),
    }
}

pub(crate) fn from_memory_inference(
    value: reliquary_memory_core::TemporalInference,
) -> TemporalInference {
    TemporalInference {
        model: value.model,
        contract_version: value.contract_version,
        resolutions: value
            .resolutions
            .into_iter()
            .map(|resolution| TemporalInferenceResolution {
                start_byte: resolution.start_byte,
                end_byte: resolution.end_byte,
                kind: from_memory_kind(resolution.kind),
                evidence: resolution.evidence,
                canonical_expression: resolution.canonical_expression,
            })
            .collect(),
    }
}

fn to_memory_kind(
    value: crate::TemporalIndicationKind,
) -> reliquary_memory_core::TemporalIndicationKind {
    match value {
        crate::TemporalIndicationKind::Explicit => {
            reliquary_memory_core::TemporalIndicationKind::Explicit
        }
        crate::TemporalIndicationKind::Calendar => {
            reliquary_memory_core::TemporalIndicationKind::Calendar
        }
        crate::TemporalIndicationKind::Relative => {
            reliquary_memory_core::TemporalIndicationKind::Relative
        }
        crate::TemporalIndicationKind::Boundary => {
            reliquary_memory_core::TemporalIndicationKind::Boundary
        }
        crate::TemporalIndicationKind::Recurrence => {
            reliquary_memory_core::TemporalIndicationKind::Recurrence
        }
        crate::TemporalIndicationKind::Duration => {
            reliquary_memory_core::TemporalIndicationKind::Duration
        }
    }
}

fn from_memory_kind(
    value: reliquary_memory_core::TemporalIndicationKind,
) -> crate::TemporalIndicationKind {
    match value {
        reliquary_memory_core::TemporalIndicationKind::Explicit => {
            crate::TemporalIndicationKind::Explicit
        }
        reliquary_memory_core::TemporalIndicationKind::Calendar => {
            crate::TemporalIndicationKind::Calendar
        }
        reliquary_memory_core::TemporalIndicationKind::Relative => {
            crate::TemporalIndicationKind::Relative
        }
        reliquary_memory_core::TemporalIndicationKind::Boundary => {
            crate::TemporalIndicationKind::Boundary
        }
        reliquary_memory_core::TemporalIndicationKind::Recurrence => {
            crate::TemporalIndicationKind::Recurrence
        }
        reliquary_memory_core::TemporalIndicationKind::Duration => {
            crate::TemporalIndicationKind::Duration
        }
    }
}
