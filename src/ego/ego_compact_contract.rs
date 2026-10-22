//! Ego-owned compact publications. Wire layout and store replay are separate.
use crate::compact_delta::FieldChange;
use crate::ego_codec::{EgoRecord, record_ego_version, validate_anchor_text, validate_text};
use crate::{EgoAnchorId, EgoAnchorPriority, EgoError, EgoIdentityId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EgoDocumentKey {
    Identity(EgoIdentityId),
    Anchor(EgoAnchorId),
    Personality,
    Synthesis,
}

pub(crate) fn document_key(record: &EgoRecord) -> Option<EgoDocumentKey> {
    match record {
        EgoRecord::Identity { id, .. } => Some(EgoDocumentKey::Identity(*id)),
        EgoRecord::Anchor { id, .. } => Some(EgoDocumentKey::Anchor(*id)),
        EgoRecord::Personality { .. } => Some(EgoDocumentKey::Personality),
        EgoRecord::Synthesis { .. } => Some(EgoDocumentKey::Synthesis),
        EgoRecord::IdentityActive { .. } => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EgoCreation {
    pub record: EgoRecord,
}

impl EgoCreation {
    pub(crate) fn new(record: EgoRecord) -> Result<Self, EgoError> {
        validate(&record)?;
        if revision(&record) != Some(1) || is_terminal(&record) {
            return Err(invalid("Ego creation needs a live first revision"));
        }
        Ok(Self { record })
    }
}

/// A terminal can be the first admitted revision in V1 input. It still consumes
/// one Ego sequence position; owner replay decides whether its lifecycle is legal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EgoTerminal {
    pub record: EgoRecord,
}

impl EgoTerminal {
    pub(crate) fn new(record: EgoRecord) -> Result<Self, EgoError> {
        validate(&record)?;
        if !is_terminal(&record) {
            return Err(invalid("Ego terminal needs a deleted Identity or Anchor"));
        }
        Ok(Self { record })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum EgoChanges {
    Identity {
        id: EgoIdentityId,
        // This is publication metadata, not a state field to inherit. A raw
        // historical update may activate even when its content is unchanged.
        activate: bool,
        name: FieldChange<String>,
        text: FieldChange<String>,
    },
    Anchor {
        id: EgoAnchorId,
        priority: FieldChange<EgoAnchorPriority>,
        text: FieldChange<String>,
    },
    Personality {
        source_memory_version: FieldChange<u64>,
        text: FieldChange<String>,
    },
    Synthesis {
        source_memory_version: FieldChange<u64>,
        text: FieldChange<String>,
    },
}

impl EgoChanges {
    pub(crate) fn document_key(&self) -> EgoDocumentKey {
        match self {
            Self::Identity { id, .. } => EgoDocumentKey::Identity(*id),
            Self::Anchor { id, .. } => EgoDocumentKey::Anchor(*id),
            Self::Personality { .. } => EgoDocumentKey::Personality,
            Self::Synthesis { .. } => EgoDocumentKey::Synthesis,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EgoDelta {
    pub ego_version: u64,
    pub previous_revision: u64,
    pub revision: u64,
    pub changes: EgoChanges,
}

impl EgoDelta {
    pub(crate) fn between(previous: &EgoRecord, next: &EgoRecord) -> Result<Self, EgoError> {
        let previous = previous.clone();
        let next = next.clone();
        validate(&previous)?;
        validate(&next)?;
        let previous_revision =
            revision(&previous).ok_or_else(|| invalid("activation has no delta"))?;
        let next_revision = previous_revision
            .checked_add(1)
            .ok_or_else(|| invalid("Ego revision exhausted"))?;
        if revision(&next) != Some(next_revision)
            || record_ego_version(&next) <= record_ego_version(&previous)
            || is_terminal(&previous)
            || is_terminal(&next)
        {
            return Err(invalid("invalid Ego delta predecessor or lifecycle"));
        }
        let changes = match (&previous, &next) {
            (
                EgoRecord::Identity {
                    id: a,
                    name: an,
                    text: at,
                    ..
                },
                EgoRecord::Identity {
                    id: b,
                    activate,
                    name: bn,
                    text: bt,
                    ..
                },
            ) if a == b => EgoChanges::Identity {
                id: *b,
                activate: *activate,
                name: FieldChange::between(an, bn),
                text: FieldChange::between(at, bt),
            },
            (
                EgoRecord::Anchor {
                    id: a,
                    priority: ap,
                    text: at,
                    ..
                },
                EgoRecord::Anchor {
                    id: b,
                    priority: bp,
                    text: bt,
                    ..
                },
            ) if a == b => EgoChanges::Anchor {
                id: *b,
                priority: FieldChange::between(ap, bp),
                text: FieldChange::between(at, bt),
            },
            (
                EgoRecord::Personality {
                    source_memory_version: a,
                    text: at,
                    ..
                },
                EgoRecord::Personality {
                    source_memory_version: b,
                    text: bt,
                    ..
                },
            ) => EgoChanges::Personality {
                source_memory_version: FieldChange::between(a, b),
                text: FieldChange::between(at, bt),
            },
            (
                EgoRecord::Synthesis {
                    source_memory_version: a,
                    text: at,
                    ..
                },
                EgoRecord::Synthesis {
                    source_memory_version: b,
                    text: bt,
                    ..
                },
            ) => EgoChanges::Synthesis {
                source_memory_version: FieldChange::between(a, b),
                text: FieldChange::between(at, bt),
            },
            _ => return Err(invalid("Ego delta changes document identity or kind")),
        };
        // A zero field mask is meaningful for already-admitted raw publications.
        Ok(Self {
            ego_version: record_ego_version(&next),
            previous_revision,
            revision: next_revision,
            changes,
        })
    }
}

pub(crate) fn revision(record: &EgoRecord) -> Option<u64> {
    match record {
        EgoRecord::Identity { revision, .. }
        | EgoRecord::Anchor { revision, .. }
        | EgoRecord::Personality { revision, .. }
        | EgoRecord::Synthesis { revision, .. } => Some(*revision),
        EgoRecord::IdentityActive { .. } => None,
    }
}

pub(crate) fn is_terminal(record: &EgoRecord) -> bool {
    matches!(
        record,
        EgoRecord::Identity { deleted: true, .. } | EgoRecord::Anchor { deleted: true, .. }
    )
}

/// Shape validation only. Dense owner sequence, prior document existence,
/// activation, terminal resurrection and Memory bounds belong to EgoStore.
pub(crate) fn validate(record: &EgoRecord) -> Result<(), EgoError> {
    if record_ego_version(record) == 0 || revision(record) == Some(0) {
        return Err(invalid("zero Ego publication or document revision"));
    }
    match record {
        EgoRecord::Identity {
            deleted: true,
            activate,
            name,
            text,
            ..
        } => {
            if *activate || !name.is_empty() || !text.is_empty() {
                return Err(invalid("deleted identity must be empty and inactive"));
            }
        }
        EgoRecord::Identity { name, text, .. } => {
            validate_text("identity name", name)?;
            validate_text("identity", text)?;
        }
        EgoRecord::Anchor {
            deleted: true,
            text,
            ..
        } => {
            if !text.is_empty() {
                return Err(invalid("deleted anchor must have an empty body"));
            }
        }
        EgoRecord::Anchor { text, .. } => validate_anchor_text(text)?,
        EgoRecord::Personality { text, .. } => validate_text("personality", text)?,
        EgoRecord::Synthesis { text, .. } => validate_text("synthesis", text)?,
        EgoRecord::IdentityActive { .. } => {}
    }
    Ok(())
}

fn invalid(message: &str) -> EgoError {
    EgoError::InvalidRecord(message.into())
}
