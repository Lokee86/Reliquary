//! Materialize a compact Ego delta without changing accepted owner state.
use crate::EgoError;
use crate::ego_codec::{EgoRecord, record_ego_version};
use crate::ego_compact_contract::{EgoChanges, EgoDelta, is_terminal, revision, validate};
use reliquary_container::codec::apply_field;

pub(crate) fn materialize(previous: &EgoRecord, delta: &EgoDelta) -> Result<EgoRecord, EgoError> {
    let previous = previous.clone();
    validate(&previous)?;
    if revision(&previous) != Some(delta.previous_revision)
        || delta.previous_revision.checked_add(1) != Some(delta.revision)
        || record_ego_version(&previous) >= delta.ego_version
        || is_terminal(&previous)
    {
        return Err(invalid("invalid compact Ego predecessor"));
    }
    let ego_version = delta.ego_version;
    let revision = delta.revision;
    let record = match (&previous, &delta.changes) {
        (
            EgoRecord::Identity { id, name, text, .. },
            EgoChanges::Identity {
                id: target,
                activate,
                name: name_change,
                text: text_change,
            },
        ) if id == target => EgoRecord::Identity {
            ego_version,
            revision,
            id: *id,
            deleted: false,
            activate: *activate,
            name: apply_field(name, name_change),
            text: apply_field(text, text_change),
        },
        (
            EgoRecord::Anchor {
                id, priority, text, ..
            },
            EgoChanges::Anchor {
                id: target,
                priority: priority_change,
                text: text_change,
            },
        ) if id == target => EgoRecord::Anchor {
            ego_version,
            revision,
            id: *id,
            deleted: false,
            priority: apply_field(priority, priority_change),
            text: apply_field(text, text_change),
        },
        (
            EgoRecord::Personality {
                source_memory_version,
                text,
                ..
            },
            EgoChanges::Personality {
                source_memory_version: source_change,
                text: text_change,
            },
        ) => EgoRecord::Personality {
            ego_version,
            revision,
            source_memory_version: apply_field(source_memory_version, source_change),
            text: apply_field(text, text_change),
        },
        (
            EgoRecord::Synthesis {
                source_memory_version,
                text,
                ..
            },
            EgoChanges::Synthesis {
                source_memory_version: source_change,
                text: text_change,
            },
        ) => EgoRecord::Synthesis {
            ego_version,
            revision,
            source_memory_version: apply_field(source_memory_version, source_change),
            text: apply_field(text, text_change),
        },
        _ => return Err(invalid("compact Ego delta targets another document")),
    };
    // Reject explicit redundant Set fields; a zero mask still represents a real
    // raw-history publication and is preserved with its next revision/sequence.
    if EgoDelta::between(&previous, &record)? != *delta {
        return Err(invalid("noncanonical compact Ego field change"));
    }
    Ok(record)
}

fn invalid(message: &str) -> EgoError {
    EgoError::InvalidRecord(message.into())
}
