//! Graph-owned V1-to-envelope transport at accepted marker slots.
use super::StorageUpgradeError;
use crate::container::publication_envelope::{self, PublicationClock, PublicationEnvelope};
use crate::graph_compact_codec::publication::{self, GraphPublication, GraphTransaction};
use crate::{Container, FileKind, ObjectRef};
use std::collections::{BTreeMap, HashSet};

#[derive(Default)]
pub(super) struct GraphEnvelopePlan {
    pub(super) markers: BTreeMap<ObjectRef, Vec<u8>>,
    pub(super) carriers: HashSet<ObjectRef>,
}

pub(super) fn prepare(source: &mut Container) -> Result<GraphEnvelopePlan, StorageUpgradeError> {
    let graph = match source.identity().map(|identity| identity.file_kind) {
        Some(FileKind::Reliquary) => crate::Cva::open_v1_read_only(source.path())?.graph,
        Some(FileKind::Phylactery) => {
            crate::Phylactery::open_v1_read_only(source.path())
                .map_err(|error| StorageUpgradeError::Migration(error.to_string()))?
                .graph
        }
        _ => return Err(StorageUpgradeError::MissingOwnerIdentity),
    };

    let accepted_globals = graph.transaction_global_versions();
    let mut next_local_version = 1_u64;
    let mut next_global_index = 0_usize;
    let mut plan = GraphEnvelopePlan::default();
    for marker in source.chunks()? {
        let payload = source.read(marker)?;
        let Some(version) = crate::graph_codec::decode_version(&payload)? else {
            continue;
        };
        let global_index = usize::try_from(version.graph_version.saturating_sub(1))
            .map_err(|_| graph_migration("Graph local version exceeds addressable range"))?;
        if version.graph_version != next_local_version
            || global_index != next_global_index
            || accepted_globals.get(global_index) != Some(&version.global_version)
        {
            return Err(graph_migration(
                "Graph marker differs from admitted transaction coordinates",
            ));
        }
        let raw = source.read(version.mutation)?;
        let (single, mutations) = graph
            .decode_stored_transaction(&raw)?
            .ok_or_else(|| graph_migration("Graph marker lacks its mutation carrier"))?;
        if mutations.is_empty() || (single && mutations.len() != 1) {
            return Err(graph_migration(
                "Graph marker has invalid single or batch cardinality",
            ));
        }
        let compact = graph.encode_compact_mutations(&mutations, single)?;
        let ordinals = crate::graph_compact_codec::decode(&compact)?
            .ok_or_else(|| graph_migration("Graph mapping produced no compact mutations"))?;
        let transaction = if single {
            GraphTransaction::Single(ordinals[0])
        } else {
            GraphTransaction::Batch(ordinals)
        };
        let body = publication::encode(&GraphPublication {
            graph_version: version.graph_version,
            transaction,
        })?;
        let envelope = publication_envelope::encode(PublicationEnvelope {
            global_version: version.global_version,
            clock: PublicationClock::BindReserved,
            body: &body,
        })?;
        if !plan.carriers.insert(version.mutation) {
            return Err(graph_migration(
                "Graph carrier has repeated publication markers",
            ));
        }
        plan.markers.insert(marker, envelope);
        next_local_version = next_local_version
            .checked_add(1)
            .ok_or_else(|| graph_migration("Graph local version exhausted"))?;
        next_global_index = next_global_index
            .checked_add(1)
            .ok_or_else(|| graph_migration("Graph global index exhausted"))?;
    }
    if next_global_index != accepted_globals.len() {
        return Err(graph_migration(
            "admitted Graph transaction lacks a source marker",
        ));
    }
    Ok(plan)
}

fn graph_migration(message: &'static str) -> StorageUpgradeError {
    StorageUpgradeError::Migration(message.into())
}
