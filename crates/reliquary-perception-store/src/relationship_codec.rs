//! Typed 2.1 Relationship revisions, tombstones and compact prune markers.
use super::*;
const MAGIC: &[u8; 8] = b"RELRLP01";
const MAX_FRAME: usize = 128 * 1024;
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) enum RelationshipRecord {
    Revision {
        owner: [u8; 16],
        perception_version: u64,
        value: Relationship,
    },
    Tombstone {
        owner: [u8; 16],
        perception_version: u64,
        value: RelationshipTombstone,
    },
    Prune {
        owner: [u8; 16],
        value: RelationshipDescriptionPrune,
    },
}
impl RelationshipRecord {
    pub(super) fn owner(&self) -> [u8; 16] {
        match self {
            Self::Revision { owner, .. }
            | Self::Tombstone { owner, .. }
            | Self::Prune { owner, .. } => *owner,
        }
    }
    pub(super) fn encode(&self) -> Result<Vec<u8>, PerceptionError> {
        let data = serde_json::to_vec(self)
            .map_err(|_| PerceptionError::InvalidField("Relationship record"))?;
        if data.len() > MAX_FRAME {
            return Err(PerceptionError::InvalidField("Relationship record size"));
        }
        let mut out = MAGIC.to_vec();
        out.extend_from_slice(&data);
        Ok(out)
    }
    pub(super) fn decode(raw: &[u8]) -> Result<Option<Self>, PerceptionError> {
        if !raw.starts_with(MAGIC) {
            return Ok(None);
        }
        if raw.len() > MAX_FRAME + 8 {
            return Err(PerceptionError::CorruptRecord("Relationship record size"));
        }
        let value: Self = serde_json::from_slice(&raw[8..])
            .map_err(|_| PerceptionError::CorruptRecord("Relationship record syntax"))?;
        if value
            .encode()
            .map_err(|_| PerceptionError::CorruptRecord("Relationship record encoding"))?
            != raw
        {
            return Err(PerceptionError::CorruptRecord(
                "noncanonical Relationship record",
            ));
        }
        Ok(Some(value))
    }
}
/// Replace one selected revision's description without changing semantic versions.
/// Proof is a distinct passive carrier, so replay requires owner-maintenance evidence.
pub fn redact_relationship_description_frame(
    raw: &[u8],
    selected: RelationshipId,
    manifest_digest: [u8; 32],
) -> Result<Option<(Vec<u8>, Vec<u8>, RelationshipDescriptionPrune)>, PerceptionError> {
    let Some(RelationshipRecord::Revision {
        owner,
        perception_version,
        mut value,
    }) = RelationshipRecord::decode(raw)?
    else {
        return Ok(None);
    };
    if value.id != selected {
        return Ok(None);
    }
    if value.summary.is_empty() || manifest_digest == [0; 32] {
        return Err(PerceptionError::InvalidField(
            "Relationship description prune",
        ));
    }
    let proof = RelationshipDescriptionPrune {
        id: value.id,
        revision: value.revision,
        summary_digest: Sha256::digest(value.summary.as_bytes()).into(),
        manifest_digest,
    };
    value.summary.clear();
    Ok(Some((
        RelationshipRecord::Revision {
            owner,
            perception_version,
            value,
        }
        .encode()?,
        RelationshipRecord::Prune {
            owner,
            value: proof,
        }
        .encode()?,
        proof,
    )))
}
