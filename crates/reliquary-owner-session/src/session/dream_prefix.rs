//! Physical-prefix proof for compact Dream populations from grouped births.
use super::*;
use crate::records::dream::freshness_journal::DirectBirthPrefix;
use crate::records::dream::{
    dream_begin_codec, dream_begin_codec::BeginPopulation, population_digest,
};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

const MAX_RELEVANT_FRAME: u64 = 128 * 1024 * 1024;

#[derive(Clone)]
struct Birth {
    global: u64,
    cohort: Option<u64>,
    mutation: Option<String>,
}

/// Verify compact Begin populations against evidence strictly before each Begin.
/// Direct, imported and journal births require additional owner proof and are
/// explicitly unsupported here rather than inferred from a caller's admission.
pub(crate) fn validate_compact_dream_prefix(
    container: &Container,
    admissions: &[DreamBeginAdmission],
) -> Result<(), OwnerSessionError> {
    let mut births = BTreeMap::<MemoryId, Birth>::new();
    let mut visible_memories = BTreeSet::new();
    let mut head = 0u64;
    let mut cohort = 0u64;
    let mut graph_head = 0u64;
    let mut direct = DirectBirthPrefix::new(container.owner_id());
    let mut imports = ImportEpochStore::prefix(container.owner_id());
    let mut import_head = None;
    let mut unsupported = false;
    let mut cursor = container.physical_cursor()?;
    while let Some(mut frame) = cursor.next_frame()? {
        if frame.object().length < 8 {
            continue;
        }
        let mut magic = [0u8; 8];
        frame
            .read_exact(&mut magic)
            .map_err(|error| OwnerSessionError::Record(error.to_string()))?;
        if magic.starts_with(b"RELDR") {
            let (version, changed) = super::dream_prefix_effect::prefix_effect(&mut frame, magic)?;
            graph_head = version.unwrap_or(graph_head);
            unsupported |= changed;
            continue;
        }
        if magic == *b"RELMCRT1" {
            if frame.object().length > MAX_RELEVANT_FRAME {
                return Err(OwnerSessionError::Record(
                    "Dream prefix frame exceeds size limit".into(),
                ));
            }
            let mut payload = Vec::with_capacity(frame.object().length as usize);
            payload.extend_from_slice(&magic);
            frame
                .read_to_end(&mut payload)
                .map_err(|error| OwnerSessionError::Record(error.to_string()))?;
            visible_memories.insert(reliquary_memory_core::decode_grouped_creation(&payload)?.id);
            unsupported = true;
            continue;
        }
        if magic.starts_with(b"RELFRS") && magic != *b"RELFRS02" {
            unsupported = true;
        } else if magic == *b"RELMDLT1"
            || magic == *b"RELMTMB1"
            || magic == *b"RELFRS02"
            || magic.starts_with(b"RELIEP")
            || magic == *b"RELIFRS1"
            || magic == *crate::records::IMPORT_PROCESSING_MAGIC
            || magic == *b"RELINSC7"
            || magic == *b"CVDREAM2"
            || matches!(&magic, b"RELGRF01" | b"RELASC01")
            || magic == *b"CVDREAM1"
            || magic.starts_with(b"RELMCOR")
            || magic.starts_with(b"RELMRET")
            || magic.starts_with(b"RELPUB")
        {
            if frame.object().length > MAX_RELEVANT_FRAME {
                return Err(OwnerSessionError::Record(
                    "Dream prefix frame exceeds size limit".into(),
                ));
            }
            let mut payload = Vec::with_capacity(frame.object().length as usize);
            payload.extend_from_slice(&magic);
            frame
                .read_to_end(&mut payload)
                .map_err(|error| OwnerSessionError::Record(error.to_string()))?;
            if let Some(record) = crate::records::user_memory::UserMemoryRecord::decode(&payload)
                .map_err(OwnerSessionError::Record)?
            {
                use crate::records::user_memory::UserMemoryEffect;
                if let UserMemoryEffect::Correction { creation, .. } = &record.effect {
                    visible_memories
                        .insert(reliquary_memory_core::decode_grouped_creation(creation)?.id);
                    graph_head = record
                        .relation()
                        .expect("correction relation")
                        .graph_version;
                    // Like an unversioned manual birth, this uses explicit populations.
                    unsupported = true;
                }
                if let Some(change) = record.history_frames().last() {
                    head = reliquary_memory_core::history_change_in_frame(change)?
                        .ok_or(OwnerSessionError::StaleDreamCut)?
                        .1;
                }
                continue;
            }
            if magic == *b"RELFRS02" {
                reliquary_freshness_store::validate_journal_frame(&payload, container.owner_id())
                    .map_err(OwnerSessionError::Record)?;
                continue;
            }
            if magic == *b"RELIFRS1" || magic == *crate::records::IMPORT_PROCESSING_MAGIC {
                imports
                    .ingest_prefix(&payload)
                    .map_err(OwnerSessionError::Record)?;
                continue;
            }
            if magic.starts_with(b"RELIEP") {
                let prior = imports.open_epoch().cloned();
                imports
                    .ingest_prefix(&payload)
                    .map_err(OwnerSessionError::Record)?;
                if let Some(start) = imports.open_epoch() {
                    if start.pre.memory_version != head {
                        unsupported = true;
                    }
                    import_head = Some(head);
                } else if let Some(start) = prior {
                    let completed = imports.status(&start.import_id).ok_or_else(|| {
                        OwnerSessionError::Record("import completion missing from prefix".into())
                    })?;
                    if let ImportEpochStatus::Completed { complete, .. } = completed {
                        if import_head != Some(head) || complete.post.memory_version != head {
                            unsupported = true;
                        }
                    }
                    import_head = None;
                }
                continue;
            }
            if magic == *b"RELMDLT1" || magic == *b"RELMTMB1" {
                let (id, version) = reliquary_memory_core::history_change_in_frame(&payload)?
                    .ok_or_else(|| {
                        OwnerSessionError::Record("missing Memory history change".into())
                    })?;
                if unsupported {
                    // An unproven earlier birth prevents compact admission,
                    // but must not reject an otherwise valid owner replay.
                    continue;
                }
                head = head.checked_add(1).ok_or_else(|| {
                    OwnerSessionError::Record("Dream Memory head exhausted".into())
                })?;
                if !visible_memories.contains(&id) || version != head {
                    return Err(OwnerSessionError::Record(
                        "Memory history change differs from its physical prefix".into(),
                    ));
                }
                continue;
            }
            if magic == *b"CVDREAM1" {
                direct.ingest(&payload).map_err(OwnerSessionError::Record)?;
                continue;
            }
            if magic.starts_with(b"RELPUB") {
                let envelope =
                    reliquary_container::decode_publication(&payload)?.ok_or_else(|| {
                        OwnerSessionError::Record("invalid publication envelope".into())
                    })?;
                if envelope.body.starts_with(b"RELMCRT1") {
                    let record = reliquary_memory_core::decode_grouped_creation(envelope.body)?;
                    visible_memories.insert(record.id);
                    head = head.checked_add(1).ok_or_else(|| {
                        OwnerSessionError::Record("Dream Memory head exhausted".into())
                    })?;
                    if record.memory_version != head {
                        unsupported = true;
                    }
                    if births
                        .insert(
                            record.id,
                            Birth {
                                global: envelope.version,
                                cohort: None,
                                mutation: Some(record.mutation_id),
                            },
                        )
                        .is_some()
                    {
                        unsupported = true;
                    }
                } else {
                    // Other publication forms need their own ordered owner proof.
                    unsupported = true;
                }
                continue;
            }
            if matches!(&magic, b"RELGRF01" | b"RELASC01") {
                let relation = reliquary_graph::relation_in_frame(&payload)?
                    .ok_or_else(|| OwnerSessionError::Record("missing Graph relation".into()))?;
                graph_head = relation.graph_version;
                continue;
            }
            if magic == *b"RELINSC7" {
                let completion = GroupedInsomniaCompletion::decode(&payload)
                    .map_err(OwnerSessionError::Record)?;
                if !completion.memory_records.is_empty()
                    && (completion.freshness_birth_turn.is_none()
                        || !completion.routing_metadata.is_empty())
                {
                    unsupported = true;
                }
                cohort = cohort
                    .checked_add(1)
                    .ok_or_else(|| OwnerSessionError::Record("Dream cohort exhausted".into()))?;
                for (index, bytes) in completion.memory_records.iter().enumerate() {
                    let record = reliquary_memory_core::decode_grouped_creation(bytes)?;
                    visible_memories.insert(record.id);
                    head = head.checked_add(1).ok_or_else(|| {
                        OwnerSessionError::Record("Dream Memory head exhausted".into())
                    })?;
                    if record.memory_version != head {
                        unsupported = true;
                    }
                    let global = completion
                        .global_version_start
                        .checked_add(u64::try_from(index).map_err(|_| {
                            OwnerSessionError::Record("Dream global version exhausted".into())
                        })?)
                        .ok_or_else(|| {
                            OwnerSessionError::Record("Dream global version exhausted".into())
                        })?;
                    if births
                        .insert(
                            record.id,
                            Birth {
                                global,
                                cohort: Some(cohort),
                                mutation: None,
                            },
                        )
                        .is_some()
                    {
                        unsupported = true;
                    }
                }
                continue;
            }
            let begin = dream_begin_codec::decode(&payload).map_err(OwnerSessionError::Record)?;
            if begin.graph_version_before > graph_head {
                return Err(OwnerSessionError::Record(
                    "Dream Begin references a Graph cut outside its physical prefix".into(),
                ));
            }
            if !visible_memories.contains(&begin.source_id) {
                return Err(OwnerSessionError::Record(
                    "Dream Begin source is absent from its physical prefix".into(),
                ));
            }
            let BeginPopulation::CompactCut { memory_cut, digest } = begin.population else {
                continue;
            };
            if imports.open_epoch().is_some() {
                return Err(OwnerSessionError::Record(
                    "Dream compact population is fenced by an open import epoch".into(),
                ));
            }
            if unsupported {
                return Err(OwnerSessionError::Record(
                    "Dream compact population requires unsupported birth-prefix proof".into(),
                ));
            }
            if memory_cut != head {
                return Err(OwnerSessionError::Record(
                    "Dream compact Memory cut differs from physical prefix".into(),
                ));
            }
            let source = births.get(&begin.source_id).ok_or_else(|| {
                OwnerSessionError::Record("Dream compact source lacks grouped birth proof".into())
            })?;
            let population: BTreeSet<_> = births
                .iter()
                .filter_map(|(id, birth)| {
                    let outside_source_cohort = source
                        .cohort
                        .is_none_or(|cohort| birth.cohort != Some(cohort));
                    let proven = match &birth.mutation {
                        None => true,
                        Some(mutation) => {
                            direct.pending_intents().get(id) == Some(mutation)
                                || direct
                                    .consumed_intents()
                                    .get(id)
                                    .is_some_and(|values| values.contains(mutation))
                                || direct.replay_births().contains(id)
                        }
                    };
                    (birth.global < source.global && outside_source_cohort && proven).then_some(*id)
                })
                .collect();
            let admission = admissions
                .iter()
                .find(|admission| admission.pass_id == begin.pass_id)
                .ok_or_else(|| {
                    OwnerSessionError::Record("Dream Begin lacks owner admission".into())
                })?;
            if begin.owner_uuid != container.owner_id()
                || admission.owner_uuid != begin.owner_uuid
                || admission.source_id != begin.source_id
                || admission.turn != begin.turn
                || admission.graph_version_before != begin.graph_version_before
                || admission.memory_cut != memory_cut
                || admission.preexisting_ids != population
                || digest != population_digest(&population)
            {
                return Err(OwnerSessionError::Record(
                    "Dream compact population differs from physical prefix".into(),
                ));
            }
        }
    }
    Ok(())
}
