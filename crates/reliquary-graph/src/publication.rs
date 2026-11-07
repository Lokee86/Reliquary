use super::*;

const MARKER: &[u8; 8] = b"RELGPB01";
const PREFIX: &[u8] = b"RELGPB";
const HEADER_BYTES: usize = 8 + 16 + 8 + 4;
const RELATION_BYTES: usize = 66 + 4;

/// One atomic Graph version with its accepted Container clock.
#[derive(Debug, Eq, PartialEq)]
pub struct ClockedGraphPublication {
    pub owner: [u8; 16],
    pub graph_version: u64,
    pub global_version: u64,
    pub bound_reservation: bool,
    pub transaction_time_ns_in_envelope: Option<i64>,
    pub relations: Vec<Relation>,
}

/// Encode an accepted Graph batch for the Container's native publication envelope.
pub fn encode_clocked_graph_body(
    owner: [u8; 16],
    version: u64,
    changes: &[RelationChange],
) -> Result<Vec<u8>, GraphError> {
    if version == 0 || changes.is_empty() {
        return Err(GraphError::CorruptRecord(
            "empty or unversioned Graph publication",
        ));
    }
    let count = u32::try_from(changes.len())
        .map_err(|_| GraphError::CorruptRecord("Graph publication count"))?;
    let capacity = changes
        .len()
        .checked_mul(RELATION_BYTES)
        .and_then(|size| size.checked_add(HEADER_BYTES))
        .ok_or(GraphError::CorruptRecord("Graph publication length"))?;
    let mut output = Vec::with_capacity(capacity);
    output.extend_from_slice(MARKER);
    output.extend_from_slice(&owner);
    output.extend_from_slice(&version.to_le_bytes());
    output.extend_from_slice(&count.to_le_bytes());
    let mut keys = BTreeSet::new();
    for change in changes {
        validate_relation(change.source, change.target, change.kind, change.origin)?;
        if !keys.insert((change.source, change.target, change.kind)) {
            return Err(GraphError::CorruptRecord(
                "repeated relation in Graph publication",
            ));
        }
        put_node(&mut output, change.source);
        put_node(&mut output, change.target);
        output.extend_from_slice(&kind_code(change.kind).to_le_bytes());
        output.push(u8::from(change.active));
        output.push(origin_code(change.origin));
    }
    Ok(output)
}

/// Inspect the owner and transaction bound to a native Graph publication.
pub fn clocked_graph_in_frame(
    payload: &[u8],
) -> Result<Option<ClockedGraphPublication>, GraphError> {
    if payload.starts_with(PREFIX) {
        return Err(GraphError::CorruptRecord(
            "Graph publication requires a clock envelope",
        ));
    }
    let Some(envelope) = reliquary_container::decode_publication(payload)? else {
        return Ok(None);
    };
    if !envelope.body.starts_with(PREFIX) {
        return Ok(None);
    }
    let data = envelope.body;
    if !data.starts_with(MARKER) || data.len() < HEADER_BYTES {
        return Err(GraphError::CorruptRecord(
            "unsupported Graph publication marker",
        ));
    }
    let mut offset = 8;
    let owner = take::<16>(data, &mut offset)?;
    let graph_version = u64::from_le_bytes(take::<8>(data, &mut offset)?);
    let count = u32::from_le_bytes(take::<4>(data, &mut offset)?) as usize;
    let remaining = data.len() - offset;
    if graph_version == 0
        || count == 0
        || remaining % RELATION_BYTES != 0
        || remaining / RELATION_BYTES != count
    {
        return Err(GraphError::CorruptRecord(
            "Graph publication count or version",
        ));
    }
    let mut relations = Vec::with_capacity(count);
    let mut keys = BTreeSet::new();
    for _ in 0..count {
        let source = get_node(data, &mut offset)?;
        let target = get_node(data, &mut offset)?;
        let kind = decode_kind(u16::from_le_bytes(take::<2>(data, &mut offset)?))?;
        let active = match take::<1>(data, &mut offset)?[0] {
            0 => false,
            1 => true,
            _ => return Err(GraphError::CorruptRecord("Graph publication active flag")),
        };
        let origin = decode_origin(take::<1>(data, &mut offset)?[0])?;
        validate_relation(source, target, kind, origin)?;
        if !keys.insert((source, target, kind)) {
            return Err(GraphError::CorruptRecord(
                "repeated relation in Graph publication",
            ));
        }
        relations.push(Relation {
            source,
            target,
            kind,
            active,
            origin,
            graph_version,
        });
    }
    Ok(Some(ClockedGraphPublication {
        owner,
        graph_version,
        global_version: envelope.version,
        bound_reservation: envelope.bound_reservation,
        transaction_time_ns_in_envelope: envelope.transaction_time_ns,
        relations,
    }))
}

impl GraphStore {
    pub(super) fn replay_clocked(
        &mut self,
        publication: ClockedGraphPublication,
        admission: &GraphAdmission,
    ) -> Result<(), GraphError> {
        if publication.owner != self.owner {
            return Err(GraphError::WrongOwner);
        }
        if publication.graph_version
            != self
                .graph_version
                .checked_add(1)
                .ok_or(GraphError::VersionExhausted)?
        {
            return Err(GraphError::CorruptRecord(
                "noncontiguous Graph publication version",
            ));
        }
        for relation in &publication.relations {
            validate_change(
                admission,
                RelationChange {
                    source: relation.source,
                    target: relation.target,
                    kind: relation.kind,
                    active: relation.active,
                    origin: relation.origin,
                },
                true,
            )?;
        }
        for relation in publication.relations {
            self.apply_relation(relation)?;
        }
        Ok(())
    }
}
