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
