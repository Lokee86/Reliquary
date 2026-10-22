#[cfg(test)]
pub(crate) use mutation::decode_mutation;
#[path = "graph_codec_mutation.rs"]
mod mutation;

pub(crate) use mutation::{GraphMutationPayload, encode_batch, encode_mutation};

use crate::{GraphError, MemoryId, ObjectRef, SemanticNodeKind, SemanticNodeRef};
use arcana::NodeId;

const FORMAT_MAGIC: &[u8; 8] = b"CVAGFMT1";
const FORMAT_SCHEMA: u32 = 1;
const NODE_V1_MAGIC: &[u8; 8] = b"CVAGNODE";
const NODE_V2_MAGIC: &[u8; 8] = b"CVAGNOD2";
const VERSION_MAGIC: &[u8; 8] = b"CVAGVER1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GraphNodePayload {
    pub semantic_node: SemanticNodeRef,
    pub node_id: NodeId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GraphVersionPayload {
    pub global_version: u64,
    pub graph_version: u64,
    pub mutation: ObjectRef,
}

pub(crate) fn encode_format() -> [u8; 12] {
    let mut out = [0_u8; 12];
    out[..8].copy_from_slice(FORMAT_MAGIC);
    out[8..].copy_from_slice(&FORMAT_SCHEMA.to_le_bytes());
    out
}

pub(crate) fn decode_format(bytes: &[u8]) -> Result<bool, GraphError> {
    if !bytes.starts_with(FORMAT_MAGIC) {
        return Ok(false);
    }
    if bytes.len() != 12 || read_u32(bytes, 8)? != FORMAT_SCHEMA {
        return Err(GraphError::CorruptRecord("format marker"));
    }
    Ok(true)
}

pub(crate) fn encode_node(node: GraphNodePayload) -> Vec<u8> {
    if let Some(memory_id) = node.semantic_node.as_memory() {
        let mut out = vec![0_u8; 44];
        out[..8].copy_from_slice(NODE_V1_MAGIC);
        out[8..40].copy_from_slice(&memory_id.0);
        out[40..44].copy_from_slice(&node.node_id.0.to_le_bytes());
        return out;
    }

    let mut out = vec![0_u8; 45];
    out[..8].copy_from_slice(NODE_V2_MAGIC);
    out[8] = node.semantic_node.kind.code();
    out[9..41].copy_from_slice(&node.semantic_node.id);
    out[41..45].copy_from_slice(&node.node_id.0.to_le_bytes());
    out
}

pub(crate) fn decode_node(bytes: &[u8]) -> Result<Option<GraphNodePayload>, GraphError> {
    if bytes.starts_with(NODE_V1_MAGIC) {
        if bytes.len() != 44 {
            return Err(GraphError::CorruptRecord("node mapping"));
        }
        return Ok(Some(GraphNodePayload {
            semantic_node: SemanticNodeRef::memory(MemoryId(bytes[8..40].try_into().unwrap())),
            node_id: NodeId(read_u32(bytes, 40)?),
        }));
    }
    if !bytes.starts_with(NODE_V2_MAGIC) {
        return Ok(None);
    }
    if bytes.len() != 45 {
        return Err(GraphError::CorruptRecord("semantic node mapping"));
    }
    let kind = SemanticNodeKind::from_code(bytes[8])
        .ok_or(GraphError::CorruptRecord("semantic node kind"))?;
    Ok(Some(GraphNodePayload {
        semantic_node: SemanticNodeRef {
            kind,
            id: bytes[9..41].try_into().unwrap(),
        },
        node_id: NodeId(read_u32(bytes, 41)?),
    }))
}

pub(crate) fn encode_version(version: GraphVersionPayload) -> [u8; 40] {
    let mut out = [0_u8; 40];
    out[..8].copy_from_slice(VERSION_MAGIC);
    out[8..16].copy_from_slice(&version.global_version.to_le_bytes());
    out[16..24].copy_from_slice(&version.graph_version.to_le_bytes());
    out[24..40].copy_from_slice(&version.mutation.legacy_bytes());
    out
}

pub(crate) fn normalized_version(global: u64, local: u64) -> [u8; 40] {
    encode_version(GraphVersionPayload {
        global_version: global,
        graph_version: local,
        mutation: ObjectRef::from_legacy_bytes([0; 16]),
    })
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, GraphError> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or(GraphError::CorruptRecord("integer"))?;
    Ok(u32::from_le_bytes(raw.try_into().unwrap()))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, GraphError> {
    let raw = bytes
        .get(offset..offset + 8)
        .ok_or(GraphError::CorruptRecord("integer"))?;
    Ok(u64::from_le_bytes(raw.try_into().unwrap()))
}
