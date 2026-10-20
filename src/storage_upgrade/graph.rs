//! Graph-owned V1 carrier translation. Existing mappings and markers retain
//! their authority; this transport never allocates a semantic publication.
use super::StorageUpgradeError;
use crate::{Container, FileKind, ObjectRef};
use std::collections::BTreeMap;

pub(super) fn prepare(
    source: &Container,
) -> Result<BTreeMap<ObjectRef, Vec<u8>>, StorageUpgradeError> {
    // Resolve through the fully admitted source owner. This also rejects a
    // mapping that arrives after its publication marker.
    let graph = match source.identity().map(|identity| identity.file_kind) {
        Some(FileKind::Reliquary) => crate::Cva::open_v1_read_only(source.path())?.graph,
        Some(FileKind::Phylactery) => {
            crate::Phylactery::open_v1_read_only(source.path())
                .map_err(|error| StorageUpgradeError::Migration(error.to_string()))?
                .graph
        }
        _ => return Err(StorageUpgradeError::MissingOwnerIdentity),
    };
    let mut scan = Container::open_v1_read_only(source.path())?;
    let mut objects = Container::open_v1_read_only(source.path())?;
    let mut encoded = BTreeMap::new();
    scan.visit_payloads::<StorageUpgradeError>(|_, payload| {
        let Some(version) = crate::graph_codec::decode_version(payload)? else {
            return Ok(());
        };
        let raw = objects.read(version.mutation)?;
        let values = graph.decode_stored_mutations(&raw)?.ok_or_else(|| {
            StorageUpgradeError::Migration("Graph marker lacks its mutation carrier".into())
        })?;
        let single = crate::graph_codec::decode_mutation(&raw)?.is_some();
        if !single && crate::graph_codec::decode_batch(&raw)?.is_none() {
            return Err(StorageUpgradeError::Migration(
                "V1 Graph publication lacks a legacy carrier".into(),
            ));
        }
        let compact = graph.encode_compact_mutations(&values, single)?;
        if encoded.insert(version.mutation, compact).is_some() {
            return Err(StorageUpgradeError::Migration(
                "Graph carrier has repeated publication markers".into(),
            ));
        }
        Ok(())
    })?;
    Ok(encoded)
}
