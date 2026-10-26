use crate::{
    GraphError, GraphRelationKind, GraphRelationOrigin, MemoryId, SemanticGraphRelationKind,
    SemanticNodeKind, SemanticNodeRef,
};

const MUTATION_V1_MAGIC: &[u8; 8] = b"CVAGMUT1";
const MUTATION_V2_MAGIC: &[u8; 8] = b"CVAGMUT2";
const BATCH_V1_MAGIC: &[u8; 8] = b"CVAGBAT1";
const BATCH_V2_MAGIC: &[u8; 8] = b"CVAGBAT2";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphMutationPayload {
    pub source: SemanticNodeRef,
    pub target: SemanticNodeRef,
    pub kind: SemanticGraphRelationKind,
    pub active: bool,
    pub origin: GraphRelationOrigin,
}

pub fn encode_mutation(mutation: GraphMutationPayload) -> Vec<u8> {
    if let Some((source, target, kind)) = memory_comparison_parts(mutation) {
        let mut out = vec![0_u8; 76];
        out[..8].copy_from_slice(MUTATION_V1_MAGIC);
        out[8..40].copy_from_slice(&source);
        out[40..72].copy_from_slice(&target);
        out[72..74].copy_from_slice(&kind.code().to_le_bytes());
        out[74] = u8::from(mutation.active);
        out[75] = mutation.origin.code();
        return out;
    }

    let mut out = vec![0_u8; 78];
    out[..8].copy_from_slice(MUTATION_V2_MAGIC);
    write_node_ref(&mut out[8..41], mutation.source);
    write_node_ref(&mut out[41..74], mutation.target);
    out[74..76].copy_from_slice(&mutation.kind.code().to_le_bytes());
    out[76] = u8::from(mutation.active);
    out[77] = mutation.origin.code();
    out
}

pub fn decode_mutation(bytes: &[u8]) -> Result<Option<GraphMutationPayload>, GraphError> {
    if bytes.starts_with(MUTATION_V1_MAGIC) {
        if bytes.len() != 76 {
            return Err(GraphError::CorruptRecord("relationship mutation"));
        }
        return Ok(Some(decode_memory_comparison_body(bytes, 8)?));
    }
    if !bytes.starts_with(MUTATION_V2_MAGIC) {
        return Ok(None);
    }
    if bytes.len() != 78 {
        return Err(GraphError::CorruptRecord("semantic relationship mutation"));
    }
    Ok(Some(decode_typed_body(bytes, 8)?))
}

pub fn encode_batch(mutations: &[GraphMutationPayload]) -> Vec<u8> {
    if mutations
        .iter()
        .all(|mutation| memory_comparison_parts(*mutation).is_some())
    {
        let mut out = Vec::with_capacity(12 + mutations.len() * 68);
        out.extend_from_slice(BATCH_V1_MAGIC);
        out.extend_from_slice(&(mutations.len() as u32).to_le_bytes());
        for mutation in mutations {
            let (source, target, kind) = memory_comparison_parts(*mutation).unwrap();
            out.extend_from_slice(&source);
            out.extend_from_slice(&target);
            out.extend_from_slice(&kind.code().to_le_bytes());
            out.push(u8::from(mutation.active));
            out.push(mutation.origin.code());
        }
        return out;
    }

    let mut out = Vec::with_capacity(12 + mutations.len() * 70);
    out.extend_from_slice(BATCH_V2_MAGIC);
    out.extend_from_slice(&(mutations.len() as u32).to_le_bytes());
    for mutation in mutations {
        let start = out.len();
        out.resize(start + 70, 0);
        write_node_ref(&mut out[start..start + 33], mutation.source);
        write_node_ref(&mut out[start + 33..start + 66], mutation.target);
        out[start + 66..start + 68].copy_from_slice(&mutation.kind.code().to_le_bytes());
        out[start + 68] = u8::from(mutation.active);
        out[start + 69] = mutation.origin.code();
    }
    out
}

pub fn decode_batch(bytes: &[u8]) -> Result<Option<Vec<GraphMutationPayload>>, GraphError> {
    if bytes.starts_with(BATCH_V1_MAGIC) {
        return decode_memory_comparison_batch(bytes).map(Some);
    }
    if bytes.starts_with(BATCH_V2_MAGIC) {
        return decode_typed_batch(bytes).map(Some);
    }
    Ok(None)
}

fn decode_memory_comparison_batch(bytes: &[u8]) -> Result<Vec<GraphMutationPayload>, GraphError> {
    if bytes.len() < 12 {
        return Err(GraphError::CorruptRecord("relationship batch"));
    }
    let count = read_u32(bytes, 8)? as usize;
    if count == 0 {
        return Err(GraphError::CorruptRecord("relationship batch"));
    }
    let expected = 12
        + count
            .checked_mul(68)
            .ok_or(GraphError::CorruptRecord("relationship batch"))?;
    if bytes.len() != expected {
        return Err(GraphError::CorruptRecord("relationship batch"));
    }
    (0..count)
        .map(|index| decode_memory_comparison_body(bytes, 12 + index * 68))
        .collect()
}

fn decode_typed_batch(bytes: &[u8]) -> Result<Vec<GraphMutationPayload>, GraphError> {
    if bytes.len() < 12 {
        return Err(GraphError::CorruptRecord("semantic relationship batch"));
    }
    let count = read_u32(bytes, 8)? as usize;
    let expected = 12
        + count
            .checked_mul(70)
            .ok_or(GraphError::CorruptRecord("semantic relationship batch"))?;
    if count == 0 || bytes.len() != expected {
        return Err(GraphError::CorruptRecord("semantic relationship batch"));
    }
    (0..count)
        .map(|index| decode_typed_body(bytes, 12 + index * 70))
        .collect()
}

fn decode_memory_comparison_body(
    bytes: &[u8],
    offset: usize,
) -> Result<GraphMutationPayload, GraphError> {
    let source: [u8; 32] = bytes[offset..offset + 32]
        .try_into()
        .map_err(|_| GraphError::CorruptRecord("relationship mutation"))?;
    let target: [u8; 32] = bytes[offset + 32..offset + 64]
        .try_into()
        .map_err(|_| GraphError::CorruptRecord("relationship mutation"))?;
    let kind_code = read_u16(bytes, offset + 64)?;
    let kind = GraphRelationKind::from_code(kind_code)
        .ok_or(GraphError::UnknownRelationKind(kind_code))?;
    let active = read_active(bytes, offset + 66)?;
    let origin = read_origin(bytes, offset + 67)?;
    Ok(GraphMutationPayload {
        source: SemanticNodeRef::memory(source),
        target: SemanticNodeRef::memory(target),
        kind: SemanticGraphRelationKind::Memory(kind),
        active,
        origin,
    })
}

fn decode_typed_body(bytes: &[u8], offset: usize) -> Result<GraphMutationPayload, GraphError> {
    let kind_code = read_u16(bytes, offset + 66)?;
    Ok(GraphMutationPayload {
        source: read_node_ref(bytes, offset)?,
        target: read_node_ref(bytes, offset + 33)?,
        kind: SemanticGraphRelationKind::from_code(kind_code)
            .ok_or(GraphError::UnknownRelationKind(kind_code))?,
        active: read_active(bytes, offset + 68)?,
        origin: read_origin(bytes, offset + 69)?,
    })
}

fn memory_comparison_parts(
    mutation: GraphMutationPayload,
) -> Option<(MemoryId, MemoryId, GraphRelationKind)> {
    Some((
        mutation.source.as_memory()?,
        mutation.target.as_memory()?,
        match mutation.kind {
            SemanticGraphRelationKind::Memory(kind) => kind,
            SemanticGraphRelationKind::EntityAssociation
            | SemanticGraphRelationKind::PrincipalAssociation => return None,
        },
    ))
}

fn write_node_ref(out: &mut [u8], node: SemanticNodeRef) {
    out[0] = node.kind.code();
    out[1..33].copy_from_slice(&node.id);
}

fn read_node_ref(bytes: &[u8], offset: usize) -> Result<SemanticNodeRef, GraphError> {
    let kind_code = *bytes
        .get(offset)
        .ok_or(GraphError::CorruptRecord("semantic node reference"))?;
    let kind = SemanticNodeKind::from_code(kind_code)
        .ok_or(GraphError::CorruptRecord("semantic node kind"))?;
    let id = bytes
        .get(offset + 1..offset + 33)
        .ok_or(GraphError::CorruptRecord("semantic node reference"))?
        .try_into()
        .unwrap();
    Ok(SemanticNodeRef { kind, id })
}

fn read_active(bytes: &[u8], offset: usize) -> Result<bool, GraphError> {
    match bytes.get(offset).copied() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => Err(GraphError::CorruptRecord("relationship active flag")),
    }
}

fn read_origin(bytes: &[u8], offset: usize) -> Result<GraphRelationOrigin, GraphError> {
    let code = *bytes
        .get(offset)
        .ok_or(GraphError::CorruptRecord("relationship origin"))?;
    GraphRelationOrigin::from_code(code).ok_or(GraphError::CorruptRecord("relationship origin"))
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, GraphError> {
    let raw = bytes
        .get(offset..offset + 2)
        .ok_or(GraphError::CorruptRecord("integer"))?;
    Ok(u16::from_le_bytes(raw.try_into().unwrap()))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, GraphError> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or(GraphError::CorruptRecord("integer"))?;
    Ok(u32::from_le_bytes(raw.try_into().unwrap()))
}
