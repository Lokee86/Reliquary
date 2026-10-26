use crate::{MemoryError, TemporalIndicationKind};

pub(crate) fn tag(kind: TemporalIndicationKind) -> u8 {
    match kind {
        TemporalIndicationKind::Explicit => 1,
        TemporalIndicationKind::Calendar => 2,
        TemporalIndicationKind::Relative => 3,
        TemporalIndicationKind::Boundary => 4,
        TemporalIndicationKind::Recurrence => 5,
        TemporalIndicationKind::Duration => 6,
    }
}

pub(crate) fn from_tag(tag: u8) -> Result<TemporalIndicationKind, MemoryError> {
    match tag {
        1 => Ok(TemporalIndicationKind::Explicit),
        2 => Ok(TemporalIndicationKind::Calendar),
        3 => Ok(TemporalIndicationKind::Relative),
        4 => Ok(TemporalIndicationKind::Boundary),
        5 => Ok(TemporalIndicationKind::Recurrence),
        6 => Ok(TemporalIndicationKind::Duration),
        _ => Err(MemoryError::CorruptRecord(
            "invalid temporal inference indication kind",
        )),
    }
}
