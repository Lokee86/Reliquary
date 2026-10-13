//! Owner-derived bounded missing genuine-source plan. Caller data is never evidence.
use super::embedding::{Expected, check_cancel};
use super::embedding_model::*;
use super::input;
use crate::{CompatibilityProfile, Cva, Fragment};
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;

pub(super) struct MissingRow {
    pub fragment: Fragment,
    pub text: String,
    pub materialize: bool,
}
pub(super) struct Plan {
    pub profile: CompatibilityProfile,
    pub missing: Vec<MissingRow>,
    pub rows: BTreeMap<[u8; 32], Vec<u8>>,
    pub expected: Expected,
}
pub(super) fn bounded(n: usize, cap: usize, name: &str) -> Result<(), Stop> {
    if n > cap { Err(budget(name)) } else { Ok(()) }
}
fn mul(a: usize, b: usize) -> Result<usize, Stop> {
    a.checked_mul(b).ok_or_else(|| budget("work overflow"))
}
fn add(a: usize, b: usize) -> Result<usize, Stop> {
    a.checked_add(b)
        .ok_or_else(|| budget("allocation overflow"))
}

pub(super) fn prepare(
    cva: &mut Cva,
    limits: HistoricalEmbeddingLimits,
    cancel: &AtomicBool,
    report: &mut HistoricalEmbeddingReport,
) -> Result<Plan, Stop> {
    limits.validate().map_err(|e| budget(&e))?;
    check_cancel(cancel)?;
    report.owner_uuid = Some(cva.historical_reconstruction_owner().map_err(unsupported)?);
    bounded(
        usize::try_from(cva.rel_turn_count()).map_err(|_| budget("source overflow"))?,
        limits.sources,
        "sources",
    )?;
    bounded(cva.archive.fragments.len(), limits.fragments, "fragments")?;
    bounded(
        cva.archive.nodes.len(),
        limits.inspections,
        "Archive nodes/ancestry",
    )?;
    let input_bytes = usize::try_from(
        std::fs::metadata(cva.container.path())
            .map_err(unsupported)?
            .len(),
    )
    .map_err(|_| budget("prefix bytes overflow"))?;
    bounded(
        mul(input_bytes, 16)?,
        limits.temporary_bytes,
        "owner proof buffers",
    )?;
    let generation = cva
        .vector_generations
        .generations()
        .iter()
        .max_by_key(|g| g.vector_version)
        .copied();
    let profile = if let Some(g) = generation {
        cva.compatibility_profile(g.compatibility_profile_id)
            .map_err(unsupported)?
    } else {
        let mut profiles = cva.compatibility_profiles();
        if profiles.len() != 1 {
            return Err(unsupported(
                "fill requires one retained profile when no generation exists",
            ));
        }
        profiles.remove(0)
    };
    bounded(profile.dimensions as usize, limits.dimensions, "dimensions")?;
    if profile.dimensions == 0 {
        return Err(unsupported("zero profile dimensions"));
    }
    report.profile_id = Some(profile.id);
    report.initial_generation = generation.map(|g| g.id);
    report.final_generation = report.initial_generation;
    report.initial_source_archive_version = generation.map(|g| g.source_archive_version);
    report.final_source_archive_version = report.initial_source_archive_version;

    // Cheap row-size checks precede both packed hydration and the full owner proof.
    if let Some(g) = generation {
        let set = cva
            .archive_vectors
            .info(g.archive_vector_id)
            .ok_or_else(|| unsupported("missing source binding"))?;
        bounded(
            usize::try_from(set.rows).map_err(|_| budget("rows overflow"))?,
            limits.generation_rows,
            "retained rows",
        )?;
        let packed = cva
            .packed_vectors
            .info(set.packed_vector_id)
            .ok_or_else(|| unsupported("missing matrix"))?;
        bounded(
            usize::try_from(packed.byte_len).map_err(|_| budget("matrix overflow"))?,
            limits.matrix_bytes,
            "retained matrix",
        )?;
        bounded(
            add(
                mul(
                    usize::try_from(packed.byte_len).map_err(|_| budget("matrix overflow"))?,
                    4,
                )?,
                mul(input_bytes, 16)?,
            )?,
            limits.temporary_bytes,
            "retained hydration buffers",
        )?;
    }
    let mut metadata = 0usize;
    for node in cva.archive.nodes.iter() {
        metadata = add(
            metadata,
            add(
                512,
                add(
                    node.id.len(),
                    add(
                        node.conversation_id.len(),
                        node.parent_id.as_ref().map_or(0, String::len),
                    )?,
                )?,
            )?,
        )?;
        metadata = add(
            metadata,
            add(
                node.role.len(),
                node.principal_id.as_ref().map_or(0, String::len),
            )?,
        )?;
        bounded(metadata, limits.temporary_bytes / 8, "source metadata")?;
    }
    for fragment in cva.archive.fragments.iter() {
        metadata = add(
            metadata,
            add(
                512,
                add(
                    fragment.conversation_id.len(),
                    add(fragment.start_node_id.len(), fragment.end_node_id.len())?,
                )?,
            )?,
        )?;
        bounded(metadata, limits.temporary_bytes / 8, "fragment metadata")?;
    }
    // Includes repeated readiness/H1 proof scans; never assume only unique fragments cost work.
    let scan = mul(
        cva.archive.nodes.len(),
        add(
            cva.archive.fragments.len(),
            limits.missing_rows.min(cva.archive.nodes.len()),
        )?,
    )?;
    bounded(
        mul(scan, 4)?,
        limits.inspections,
        "source/fragment inspection work",
    )?;
    bounded(
        add(mul(metadata, 4)?, mul(input_bytes, 16)?)?,
        limits.temporary_bytes,
        "owner metadata buffers",
    )?;
    input::required_nodes(cva, cva.rel_turn_count()).map_err(unsupported)?;
    let timeline = cva.historical_freshness_timeline().map_err(unsupported)?;
    report.sources = timeline.activity_order.len();
    report.fragments = cva.archive.fragments.len();
    report.import_id = Some(crate::freshness_storage::historical_import_id(
        report.owner_uuid.unwrap(),
        input::corpus_digest(cva, cva.rel_turn_count()).map_err(unsupported)?,
    ));

    let mut rows = BTreeMap::new();
    if let Some(g) = generation {
        let set = cva
            .archive_vectors(g.archive_vector_id)
            .map_err(unsupported)?;
        let packed = cva
            .packed_vectors(set.packed_vector_id)
            .map_err(unsupported)?;
        if packed.schema().dimensions != profile.dimensions
            || packed.schema().scalar != crate::ScalarType::F32
        {
            return Err(unsupported("incompatible retained matrix"));
        }
        for (row, id) in set.fragment_ids.iter().enumerate() {
            rows.insert(
                id.0,
                packed
                    .row(row)
                    .ok_or_else(|| unsupported("missing retained row"))?
                    .to_vec(),
            );
        }
    }
    report.reused_rows = rows.len();
    let mut missing = Vec::new();
    let mut new_fragments = 0usize;
    let mut planned_bytes = crate::compatibility_profile_probe::QUERY_PROBES
        .iter()
        .chain(crate::compatibility_profile_probe::DOCUMENT_PROBES.iter())
        .map(|s| s.len())
        .sum::<usize>();
    bounded(planned_bytes, limits.text_bytes, "probe text bytes")?;
    for key in &timeline.activity_order {
        check_cancel(cancel)?;
        let fragment = Fragment {
            id: crate::fragment_store::fragment_id(
                &key.conversation_id,
                &key.node_id,
                &key.node_id,
            ),
            conversation_id: key.conversation_id.clone(),
            start_node_id: key.node_id.clone(),
            end_node_id: key.node_id.clone(),
        };
        if rows.contains_key(&fragment.id.0) {
            continue;
        }
        bounded(
            add(missing.len(), 1)?,
            limits.missing_rows,
            "missing sources/rows",
        )?;
        let materialize = match cva.archive.fragments.get(fragment.id) {
            Some(existing) if existing != &fragment => {
                return Err(unsupported("conflicting exact fragment"));
            }
            Some(_) => false,
            None => true,
        };
        if materialize {
            new_fragments += 1;
            bounded(new_fragments, limits.new_fragments, "new fragments")?;
        }
        let text = cva
            .archive
            .exact_fragment_text_bounded(&mut cva.container, &fragment, limits.source_bytes)
            .map_err(|e| match e {
                crate::ArchiveError::Container(crate::ContainerError::ChunkTooLarge)
                | crate::ArchiveError::FieldTooLarge => budget("source text bytes"),
                _ => unsupported(e),
            })?;
        planned_bytes = add(planned_bytes, text.len())?;
        bounded(planned_bytes, limits.text_bytes, "total text bytes")?;
        missing.push(MissingRow {
            fragment,
            text,
            materialize,
        });
    }
    // Keep planning separate from durable-progress accounting.
    report.missing_rows = missing.len();
    let final_rows = add(rows.len(), missing.len())?;
    bounded(final_rows, limits.generation_rows, "resulting rows")?;
    bounded(
        add(report.fragments, new_fragments)?,
        limits.fragments,
        "resulting fragments",
    )?;
    let width = mul(profile.dimensions as usize, 4)?;
    let matrix = mul(final_rows, width)?;
    bounded(matrix, limits.matrix_bytes, "resulting matrix")?;
    // Four matrix-sized buffers cover old hydration, ordered union, encoding and
    // duplicate-object validation; metadata/text/protocol decoding have separate reserves.
    let temporary = add(
        add(mul(matrix, 4)?, mul(metadata, 4)?)?,
        add(mul(planned_bytes, 4)?, 32 * 1024 * 1024)?,
    )?;
    bounded(temporary, limits.temporary_bytes, "temporary buffers")?;
    // Count every full union publication rather than just the new vectors.
    let mut cumulative_rows = rows.len();
    let mut published = rows.len();
    let mut appended_bytes = 0usize;
    for batch in missing.chunks(limits.batch_inputs) {
        published = add(published, batch.len())?;
        cumulative_rows = add(cumulative_rows, published)?;
        appended_bytes = add(appended_bytes, add(mul(published, add(width, 32)?)?, 2048)?)?;
    }
    bounded(
        mul(
            mul(
                add(
                    cumulative_rows,
                    mul(report.sources, add(rows.len(), final_rows)?)?,
                )?,
                profile.dimensions as usize,
            )?,
            4,
        )?,
        limits.components,
        "cumulative matrix work",
    )?;
    bounded(
        add(
            temporary,
            mul(
                add(input_bytes, add(appended_bytes, mul(metadata, 2)?)?)?,
                16,
            )?,
        )?,
        limits.temporary_bytes,
        "final owner proof buffers",
    )?;
    bounded(
        missing.len().div_ceil(limits.batch_inputs),
        limits.source_calls,
        "source calls",
    )?;
    // All native provenance, generation-complete and replacement-isolation gates
    // execute before provider I/O. This provisional proof is discarded, never committed.
    cva.freeze_historical_import().map_err(unsupported)?;
    cva.container.sync().map_err(unsupported)?;
    let expected = Expected::read(cva)?;
    Ok(Plan {
        profile,
        missing,
        rows,
        expected,
    })
}
