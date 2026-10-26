//! Current-format test publications from explicit expected postimages.
//! Full deltas provide a reference for minimal changed-field publications.
use crate::ego_codec::{EgoRecord, compact, record_ego_version};
use crate::ego_compact_contract::{EgoChanges, EgoCreation, EgoDelta, EgoTerminal, revision};
use reliquary_container::codec::FieldChange;
use reliquary_ego_store::EgoError;

pub(crate) fn encode_postimage(record: &EgoRecord) -> Result<Vec<u8>, EgoError> {
    if crate::ego_compact_contract::is_terminal(record) {
        return compact::encode_terminal(&EgoTerminal::new(record.clone())?);
    }
    if let EgoRecord::IdentityActive { ego_version, id } = record {
        return Ok(crate::ego_codec::encode_activation(*ego_version, *id));
    }
    let revision = revision(record).expect("document revision");
    if revision == 1 {
        return compact::encode_creation(&EgoCreation::new(record.clone())?);
    }
    let changes = match record {
        EgoRecord::Identity {
            id,
            activate,
            name,
            text,
            ..
        } => EgoChanges::Identity {
            id: *id,
            activate: *activate,
            name: FieldChange::Set(name.clone()),
            text: FieldChange::Set(text.clone()),
        },
        EgoRecord::Anchor {
            id, priority, text, ..
        } => EgoChanges::Anchor {
            id: *id,
            priority: FieldChange::Set(*priority),
            text: FieldChange::Set(text.clone()),
        },
        EgoRecord::Personality {
            source_memory_version,
            text,
            ..
        } => EgoChanges::Personality {
            source_memory_version: FieldChange::Set(*source_memory_version),
            text: FieldChange::Set(text.clone()),
        },
        EgoRecord::Synthesis {
            source_memory_version,
            text,
            ..
        } => EgoChanges::Synthesis {
            source_memory_version: FieldChange::Set(*source_memory_version),
            text: FieldChange::Set(text.clone()),
        },
        EgoRecord::IdentityActive { .. } => unreachable!(),
    };
    compact::encode_delta(&EgoDelta {
        ego_version: record_ego_version(record),
        previous_revision: revision - 1,
        revision,
        changes,
    })
}
