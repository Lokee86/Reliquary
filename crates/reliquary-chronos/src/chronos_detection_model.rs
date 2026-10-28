#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TemporalIndicationKind {
    Explicit,
    Calendar,
    Relative,
    Boundary,
    Recurrence,
    Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalIndication {
    pub start_byte: usize,
    pub end_byte: usize,
    pub kind: TemporalIndicationKind,
    pub evidence: String,
    pub normalized: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TemporalDetection {
    pub indications: Vec<TemporalIndication>,
}

impl TemporalDetection {
    pub fn has_indications(&self) -> bool {
        !self.indications.is_empty()
    }

    pub fn has_corrections(&self) -> bool {
        self.indications
            .iter()
            .any(|indication| indication.normalized.is_some())
    }
}
