use super::*;

const MARKER: &[u8; 8] = b"RELEPB01";
const PREFIX: &[u8] = b"RELEPB";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RetainedEntity {
    Revision {
        entity_version: u64,
        entity: Entity,
        global_id: Option<[u8; 16]>,
    },
    Terminal {
        terminal: EntityTombstone,
        replacement: Option<EntityId>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClockedEntityPublication {
    pub owner: [u8; 16],
    pub global_version: u64,
    pub bound_reservation: bool,
    pub transaction_time_ns_in_envelope: Option<i64>,
    pub accepted: RetainedEntity,
}

/// Encode a current owner-local Entity publication for the native clock envelope.
pub fn encode_retained_entity_body(
    owner: [u8; 16],
    accepted: &RetainedEntity,
) -> Result<Vec<u8>, PerceptionError> {
    let mut output = MARKER.to_vec();
    match accepted {
        RetainedEntity::Revision {
            entity_version,
            entity,
            global_id,
        } => {
            if *entity_version == 0 || entity.revision == 0 {
                return Err(PerceptionError::CorruptRecord(
                    "retained Entity revision coordinate",
                ));
            }
            validate_entity(entity)?;
            output.push(1);
            optional_id(&mut output, *global_id);
            output.extend(
                Record::Entity {
                    version: *entity_version,
                    owner,
                    value: entity.clone(),
                }
                .encode()?,
            );
        }
        RetainedEntity::Terminal {
            terminal,
            replacement,
        } => {
            if terminal.perception_version == 0
                || terminal.revision < 2
                || *replacement == Some(terminal.id)
            {
                return Err(PerceptionError::CorruptRecord(
                    "retained Entity terminal coordinate",
                ));
            }
            output.push(2);
            optional_id(&mut output, replacement.map(|id| id.0));
            output.extend(encode_entity_tombstone(owner, *terminal));
        }
    }
    Ok(output)
}

/// Read one exact Entity publication and its accepted physical clock binding.
pub fn clocked_entity_in_frame(
    payload: &[u8],
) -> Result<Option<ClockedEntityPublication>, PerceptionError> {
    if payload.starts_with(PREFIX) {
        return Err(PerceptionError::CorruptRecord(
            "retained Entity requires a clock envelope",
        ));
    }
    let Some(envelope) = reliquary_container::decode_publication(payload)? else {
        return Ok(None);
    };
    let body = envelope.body;
    if !body.starts_with(PREFIX) {
        return Ok(None);
    }
    if !body.starts_with(MARKER) {
        return Err(PerceptionError::CorruptRecord(
            "unknown retained Entity marker",
        ));
    }
    let mut offset = 8;
    let variant = get_u8(body, &mut offset)?;
    let (owner, accepted) = match variant {
        1 => {
            let global_id = read_optional_id::<16>(body, &mut offset)?;
            let Some(Record::Entity {
                version,
                owner,
                value,
            }) = Record::decode(&body[offset..])?
            else {
                return Err(PerceptionError::CorruptRecord(
                    "retained Entity revision body",
                ));
            };
            (
                owner,
                RetainedEntity::Revision {
                    entity_version: version,
                    entity: value,
                    global_id,
                },
            )
        }
        2 => {
            let replacement = read_optional_id::<32>(body, &mut offset)?.map(EntityId);
            let (owner, terminal) = decode_entity_tombstone(&body[offset..])?.ok_or(
                PerceptionError::CorruptRecord("retained Entity terminal body"),
            )?;
            if replacement == Some(terminal.id) {
                return Err(PerceptionError::CorruptRecord(
                    "retained Entity replaces itself",
                ));
            }
            (
                owner,
                RetainedEntity::Terminal {
                    terminal,
                    replacement,
                },
            )
        }
        _ => {
            return Err(PerceptionError::CorruptRecord(
                "unknown retained Entity variant",
            ));
        }
    };
    Ok(Some(ClockedEntityPublication {
        owner,
        global_version: envelope.version,
        bound_reservation: envelope.bound_reservation,
        transaction_time_ns_in_envelope: envelope.transaction_time_ns,
        accepted,
    }))
}

fn optional_id<const N: usize>(out: &mut Vec<u8>, value: Option<[u8; N]>) {
    if let Some(id) = value {
        out.push(1);
        out.extend_from_slice(&id);
    } else {
        out.push(0);
    }
}

fn read_optional_id<const N: usize>(
    bytes: &[u8],
    offset: &mut usize,
) -> Result<Option<[u8; N]>, PerceptionError> {
    match get_u8(bytes, offset)? {
        0 => Ok(None),
        1 => get_array::<N>(bytes, offset).map(Some),
        _ => Err(PerceptionError::CorruptRecord(
            "retained Entity optional identity",
        )),
    }
}

impl PerceptionStore {
    pub(super) fn replay_retained_entity(
        &mut self,
        publication: ClockedEntityPublication,
    ) -> Result<(), PerceptionError> {
        if publication.owner != self.owner_id {
            return Err(PerceptionError::OwnerMismatch);
        }
        match publication.accepted {
            RetainedEntity::Revision {
                entity_version,
                entity,
                ..
            } => self.apply(
                Record::Entity {
                    version: entity_version,
                    owner: publication.owner,
                    value: entity,
                },
                // Historical publications could explicitly supply the first stable Entity ID.
                true,
            ),
            RetainedEntity::Terminal {
                terminal,
                replacement,
            } => {
                if replacement.is_some_and(|id| !self.entities.contains_key(&id)) {
                    return Err(PerceptionError::CorruptRecord(
                        "retained Entity replacement is absent",
                    ));
                }
                self.apply_tombstone_revision(terminal, 1)
            }
        }
    }
}
