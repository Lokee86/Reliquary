use crate::storage_reclamation_scan::ReclamationScan;
use crate::{Cva, CvaError, Phylactery, PhylacteryError};
use reliquary_container::{Container, ObjectRef};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StorageReclamationReport {
    pub copied_chunks: usize,
    pub reclaimed_chunks: usize,
    pub unpublished_backing_chunks: usize,
    pub orphan_content_chunks: usize,
    pub orphan_memory_body_chunks: usize,
    pub unactivated_archive_vector_chunks: usize,
    pub unreferenced_packed_vector_chunks: usize,
    pub free_compaction_chunks: usize,
    pub superseded_external_index_chunks: usize,
    pub reclaimed_payload_bytes: u64,
    pub source_file_bytes: u64,
    pub output_file_bytes: u64,
}

impl Cva {
    pub fn reclaim_storage(
        &mut self,
        output_path: impl AsRef<Path>,
    ) -> Result<StorageReclamationReport, CvaError> {
        if self
            .freshness
            .import_settlements()
            .iter()
            .any(|s| s.semantic.is_none())
        {
            return Err(CvaError::Repack(
                "current import settlement semantic evidence is missing".into(),
            ));
        }
        let expected = SemanticState {
            owner_uuid: self.owner_uuid(),
            global_version: self.latest_global_version(),
            memory_version: self.memory_version(),
            entity_version: self.entity_version(),
            relationship_version: self.relationship_version(),
            graph_version: self.graph_version(),
            archive_version: Some(self.archive_version()),
            vector_version: Some(self.vector_version()),
        };
        let report = reclaim_container(&mut self.container, output_path.as_ref())
            .map_err(CvaError::Repack)?;
        let reopened = Cva::open(output_path.as_ref()).map_err(|error| {
            let _ = fs::remove_file(output_path.as_ref());
            error
        })?;
        if let Err(error) = self.validate_historical_import_copy(&reopened) {
            drop(reopened);
            let _ = fs::remove_file(output_path.as_ref());
            return Err(error);
        }
        let actual = SemanticState {
            owner_uuid: reopened.owner_uuid(),
            global_version: reopened.latest_global_version(),
            memory_version: reopened.memory_version(),
            entity_version: reopened.entity_version(),
            relationship_version: reopened.relationship_version(),
            graph_version: reopened.graph_version(),
            archive_version: Some(reopened.archive_version()),
            vector_version: Some(reopened.vector_version()),
        };
        let activity_matches = self.archive.activity_history_matches(&reopened.archive);
        drop(reopened);
        if actual != expected || !activity_matches {
            let _ = fs::remove_file(output_path.as_ref());
            return Err(CvaError::Repack(
                "reclaimed REL changed owner, semantic versions or activity history".into(),
            ));
        }
        Ok(report)
    }
}

impl Phylactery {
    pub fn reclaim_storage(
        &mut self,
        output_path: impl AsRef<Path>,
    ) -> Result<StorageReclamationReport, PhylacteryError> {
        let expected = SemanticState {
            owner_uuid: self.owner_uuid(),
            global_version: self.latest_global_version(),
            memory_version: self.memory_version(),
            entity_version: self.entity_version(),
            relationship_version: self.relationship_version(),
            graph_version: self.graph_version(),
            archive_version: None,
            vector_version: None,
        };
        let report = reclaim_container(&mut self.container, output_path.as_ref())
            .map_err(PhylacteryError::Repack)?;
        let reopened = Phylactery::open(output_path.as_ref()).map_err(|error| {
            let _ = fs::remove_file(output_path.as_ref());
            error
        })?;
        let actual = SemanticState {
            owner_uuid: reopened.owner_uuid(),
            global_version: reopened.latest_global_version(),
            memory_version: reopened.memory_version(),
            entity_version: reopened.entity_version(),
            relationship_version: reopened.relationship_version(),
            graph_version: reopened.graph_version(),
            archive_version: None,
            vector_version: None,
        };
        drop(reopened);
        if actual != expected {
            let _ = fs::remove_file(output_path.as_ref());
            return Err(PhylacteryError::Repack(
                "reclaimed PHY changed owner or semantic version state".into(),
            ));
        }
        Ok(report)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SemanticState {
    owner_uuid: Option<[u8; 16]>,
    global_version: u64,
    memory_version: u64,
    entity_version: u64,
    relationship_version: u64,
    graph_version: u64,
    archive_version: Option<u64>,
    vector_version: Option<u64>,
}

fn reclaim_container(
    container: &mut Container,
    output_path: &Path,
) -> Result<StorageReclamationReport, String> {
    container.sync().map_err(|error| error.to_string())?;
    let source_file_bytes = fs::metadata(container.path())
        .map_err(|error| error.to_string())?
        .len();
    let objects = container.chunks().map_err(|error| error.to_string())?;

    let mut scan = ReclamationScan::default();
    for object in &objects {
        let payload = container.read(*object).map_err(|error| error.to_string())?;
        scan.ingest(*object, &payload)?;
    }
    let plan = scan.finish();

    let mut output = container
        .create_empty_like(output_path)
        .map_err(|error| error.to_string())?;
    let mut relocated = BTreeMap::<ObjectRef, ObjectRef>::new();
    let mut copied_chunks = 0_usize;
    let mut reclaimed_payload_bytes = 0_u64;
    let copy_result = (|| {
        for object in objects {
            if plan.drop.contains(&object) {
                reclaimed_payload_bytes = reclaimed_payload_bytes
                    .checked_add(container.object_capacity(object))
                    .ok_or_else(|| "reclaimed payload byte overflow".to_owned())?;
                continue;
            }
            let payload = container.read(object).map_err(|error| error.to_string())?;
            let payload = relocate_payload(&payload, &relocated)?;
            let destination = output.append(&payload).map_err(|error| error.to_string())?;
            relocated.insert(object, destination);
            copied_chunks += 1;
        }
        output.sync().map_err(|error| error.to_string())
    })();
    if let Err(error) = copy_result {
        drop(output);
        let _ = fs::remove_file(output_path);
        return Err(error);
    }
    drop(output);

    let output_file_bytes = fs::metadata(output_path)
        .map_err(|error| error.to_string())?
        .len();
    if plan.drop.is_empty() && output_file_bytes != source_file_bytes {
        let _ = fs::remove_file(output_path);
        return Err(format!(
            "reclamation changed file size without reclaimable chunks: source={source_file_bytes} output={output_file_bytes}"
        ));
    }
    if !plan.drop.is_empty() && output_file_bytes >= source_file_bytes {
        let _ = fs::remove_file(output_path);
        return Err(format!(
            "reclamation did not shrink file: source={source_file_bytes} output={output_file_bytes}"
        ));
    }

    Ok(StorageReclamationReport {
        copied_chunks,
        reclaimed_chunks: plan.drop.len(),
        unpublished_backing_chunks: plan.unpublished_backing_chunks,
        orphan_content_chunks: plan.orphan_content_chunks,
        orphan_memory_body_chunks: plan.orphan_memory_body_chunks,
        unactivated_archive_vector_chunks: plan.unactivated_archive_vector_chunks,
        unreferenced_packed_vector_chunks: plan.unreferenced_packed_vector_chunks,
        free_compaction_chunks: plan.free_compaction_chunks,
        superseded_external_index_chunks: plan.superseded_external_index_chunks,
        reclaimed_payload_bytes,
        source_file_bytes,
        output_file_bytes,
    })
}

pub(crate) fn relocate_payload(
    payload: &[u8],
    relocated: &BTreeMap<ObjectRef, ObjectRef>,
) -> Result<Vec<u8>, String> {
    if let Some(mut version) = crate::vector_generation_codec::decode_version(payload)
        .map_err(|error| error.to_string())?
    {
        version.record = relocated_ref(version.record, relocated, "Vector generation record")?;
        return Ok(crate::vector_generation_codec::encode_version(version).to_vec());
    }
    if let Some(envelope) = reliquary_container::publication_envelope::decode(payload)
        .map_err(|error| error.to_string())?
    {
        // These owner bodies contain stable IDs or Graph ordinals, never
        // physical ObjectRefs. Validate the supported body and preserve the
        // complete envelope byte-for-byte; direct Graph mappings are retained.
        if crate::memory_compact_codec::publication::decode(envelope.body, envelope.global_version)
            .map_err(|error| error.to_string())?
            .is_none()
            && crate::entity_compact_codec::publication::decode(
                envelope.body,
                envelope.global_version,
            )
            .map_err(|error| error.to_string())?
            .is_none()
            && reliquary_perception_store::relationship_compact_codec::publication::decode(
                envelope.body,
                envelope.global_version,
            )
            .map_err(|error| error.to_string())?
            .is_none()
            && reliquary_archive::archive_codec::publication::decode(
                envelope.body,
                envelope.global_version,
            )
            .map_err(|error| error.to_string())?
            .is_none()
            && crate::graph_compact_codec::publication::decode(
                envelope.body,
                envelope.global_version,
            )
            .map_err(|error| error.to_string())?
            .is_none()
        {
            return Err("unsupported publication envelope body during relocation".to_owned());
        }
        return Ok(payload.to_vec());
    }
    Ok(payload.to_vec())
}

fn relocated_ref(
    source: ObjectRef,
    relocated: &BTreeMap<ObjectRef, ObjectRef>,
    kind: &str,
) -> Result<ObjectRef, String> {
    relocated
        .get(&source)
        .copied()
        .ok_or_else(|| format!("{kind} points to a missing or not-yet-copied physical object"))
}
#[cfg(test)]
#[path = "storage_reclamation_entity_compact_tests.rs"]
mod entity_compact_tests;

#[cfg(test)]
#[path = "storage_reclamation_relationship_compact_tests.rs"]
mod relationship_compact_tests;

#[cfg(test)]
#[path = "storage_reclamation_memory_publication_tests.rs"]
mod memory_publication_tests;

#[cfg(test)]
#[path = "storage_reclamation_entity_publication_tests.rs"]
mod entity_publication_tests;

#[cfg(test)]
#[path = "storage_reclamation_relationship_publication_tests.rs"]
mod relationship_publication_tests;

#[cfg(test)]
#[path = "graph_compact_maintenance_tests.rs"]
mod compact_graph_tests;

#[cfg(test)]
#[path = "storage_reclamation_graph_publication_tests.rs"]
mod graph_publication_tests;
