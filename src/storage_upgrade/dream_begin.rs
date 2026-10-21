//! Temporary V1 Dream Begin conversion at unchanged physical evidence slots.
use super::StorageUpgradeError;
use crate::dream_begin_codec::{self, BeginPopulation, BeginRecord};
use crate::{Container, FileKind, ObjectRef};
use std::collections::BTreeMap;

pub(super) fn prepare(
    source: &mut Container,
) -> Result<BTreeMap<ObjectRef, Vec<u8>>, StorageUpgradeError> {
    if source
        .identity()
        .is_none_or(|identity| identity.file_kind != FileKind::Reliquary)
    {
        return Ok(BTreeMap::new());
    }
    let rel = crate::Cva::open_v1_read_only(source.path())?;
    let owner = rel
        .owner_uuid()
        .ok_or(StorageUpgradeError::MissingOwnerIdentity)?;
    let mut result = BTreeMap::new();
    let mut prefix = crate::cva_dream_population::DreamPopulationPrefix::new(owner);
    let end = source.physical_length()?;
    source.visit_versioned_payloads_through(
        end,
        |object, payload, frontier| -> Result<(), StorageUpgradeError> {
            if let Some(pass) =
                crate::freshness_dream_journal::DreamFreshnessJournal::decode_explicit_begin(
                    payload, owner,
                )
                .map_err(StorageUpgradeError::Migration)?
            {
                let cut = prefix.memory_head();
                let source_present = rel
                    .memories
                    .first_publication_prefix(cut)?
                    .iter()
                    .any(|(id, _)| *id == pass.source_id);
                let mut population = BeginPopulation::ExplicitSet(pass.preexisting_ids.clone());
                if source_present && !pass.preexisting_ids.is_empty() {
                    let ids =
                        prefix.materialize(&rel.memories, Some(owner), cut, pass.source_id)?;
                    if ids == pass.preexisting_ids {
                        population = BeginPopulation::CompactCut {
                            memory_cut: cut,
                            digest: crate::dream_population_digest::digest(&ids),
                        };
                    }
                }
                let bytes = dream_begin_codec::encode(&BeginRecord {
                    owner_uuid: owner,
                    pass_id: pass.pass_id,
                    source_id: pass.source_id,
                    turn: pass.turn,
                    graph_version_before: pass.graph_version_before,
                    population,
                })
                .map_err(StorageUpgradeError::Migration)?;
                result.insert(object, bytes);
            }
            prefix.ingest(&rel.archive, &rel.memories, object, payload, frontier)?;
            Ok(())
        },
    )?;
    Ok(result)
}
