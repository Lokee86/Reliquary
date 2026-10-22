//! Concrete REL/PHY reclamation capabilities over a held physical source.
//! Liveness policy stays in Maintenance; this module only freezes and copies frames.
use super::HistorySource;
use crate::{Container, ObjectRef};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::Path,
};

struct ReclamationPhysical<'a> {
    container: &'a mut Container,
    objects: Vec<ObjectRef>,
    source_file_bytes: u64,
    physical_end: u64,
}

impl<'a> ReclamationPhysical<'a> {
    fn prepare(container: &'a mut Container) -> Result<Self, String> {
        container.sync().map_err(|error| error.to_string())?;
        let source_file_bytes = fs::metadata(container.path())
            .map_err(|error| error.to_string())?
            .len();
        let objects = container.chunks().map_err(|error| error.to_string())?;
        let physical_end = container
            .physical_length()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            container,
            objects,
            source_file_bytes,
            physical_end,
        })
    }
    fn history_source(&self) -> Result<HistorySource<'_>, String> {
        let source = HistorySource::new(self.container).map_err(|error| error.to_string())?;
        if source.prefix_end() != self.physical_end {
            return Err(crate::ContainerError::UnstableSnapshot.to_string());
        }
        Ok(source)
    }
    fn copy(
        self,
        discard: &HashSet<ObjectRef>,
        output_path: &Path,
    ) -> Result<(ReclamationCopyStats, Container), String> {
        self.history_source()?
            .validate()
            .map_err(|error| error.to_string())?;
        copy_selected(
            self.container,
            self.objects,
            self.source_file_bytes,
            discard,
            output_path,
        )
    }
}

pub(crate) struct ReliquaryReclamationSource<'a> {
    physical: ReclamationPhysical<'a>,
}
pub(crate) struct PhylacteryReclamationSource<'a> {
    physical: ReclamationPhysical<'a>,
}
pub(crate) struct ReliquaryReclamationCopy {
    stats: ReclamationCopyStats,
    container: Container,
}
pub(crate) struct PhylacteryReclamationCopy {
    stats: ReclamationCopyStats,
    container: Container,
}

#[derive(Clone, Copy)]
pub(crate) struct ReclamationCopyStats {
    pub(crate) copied_chunks: usize,
    pub(crate) reclaimed_payload_bytes: u64,
    pub(crate) source_file_bytes: u64,
    pub(crate) output_file_bytes: u64,
}

impl<'a> ReliquaryReclamationSource<'a> {
    pub(crate) fn prepare(container: &'a mut Container) -> Result<Self, String> {
        Ok(Self {
            physical: ReclamationPhysical::prepare(container)?,
        })
    }
    pub(crate) fn history_source(&self) -> Result<HistorySource<'_>, String> {
        self.physical.history_source()
    }
    pub(crate) fn objects(&self) -> &[ObjectRef] {
        &self.physical.objects
    }
    pub(crate) fn copy(
        self,
        discard: &HashSet<ObjectRef>,
        output: &Path,
    ) -> Result<ReliquaryReclamationCopy, String> {
        let (stats, container) = self.physical.copy(discard, output)?;
        Ok(ReliquaryReclamationCopy { stats, container })
    }
}
impl<'a> PhylacteryReclamationSource<'a> {
    pub(crate) fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.physical.container.owner_uuid()
    }
    pub(crate) fn latest_version(&self) -> u64 {
        self.physical.container.latest_version()
    }
    pub(crate) fn prepare(container: &'a mut Container) -> Result<Self, String> {
        Ok(Self {
            physical: ReclamationPhysical::prepare(container)?,
        })
    }
    pub(crate) fn history_source(&self) -> Result<HistorySource<'_>, String> {
        self.physical.history_source()
    }
    pub(crate) fn objects(&self) -> &[ObjectRef] {
        &self.physical.objects
    }
    pub(crate) fn copy(
        self,
        discard: &HashSet<ObjectRef>,
        output: &Path,
    ) -> Result<PhylacteryReclamationCopy, String> {
        let (stats, container) = self.physical.copy(discard, output)?;
        Ok(PhylacteryReclamationCopy { stats, container })
    }
}
impl ReliquaryReclamationCopy {
    pub(crate) fn stats(&self) -> ReclamationCopyStats {
        self.stats
    }
    // Temporary upper REL adapter consumes the held output until lower REL finalization converges.
    pub(crate) fn into_reliquary_container(self) -> Container {
        self.container
    }
}
impl PhylacteryReclamationCopy {
    pub(crate) fn stats(&self) -> ReclamationCopyStats {
        self.stats
    }
    pub(crate) fn read_source(
        &self,
    ) -> Result<crate::container::PhysicalReadSource, crate::ContainerError> {
        self.container.read_source()
    }
    pub(crate) fn open_owner(self) -> Result<super::PhylacteryOwner, super::OwnerPhylacteryError> {
        super::open_phylactery_from_container(self.container)
    }
}

fn copy_selected(
    container: &mut Container,
    objects: Vec<ObjectRef>,
    source_file_bytes: u64,
    discard: &HashSet<ObjectRef>,
    output_path: &Path,
) -> Result<(ReclamationCopyStats, Container), String> {
    let mut output = container
        .create_empty_like(output_path)
        .map_err(|error| error.to_string())?;
    let output_guard = output.read_source().map_err(|error| error.to_string())?;
    let mut relocated = BTreeMap::<ObjectRef, ObjectRef>::new();
    let mut copied_chunks = 0_usize;
    let mut reclaimed_payload_bytes = 0_u64;
    let copy_result = (|| {
        for object in objects {
            if discard.contains(&object) {
                reclaimed_payload_bytes = reclaimed_payload_bytes
                    .checked_add(container.object_capacity(object))
                    .ok_or_else(|| "reclaimed payload byte overflow".to_owned())?;
                continue;
            }
            let payload = container.read(object).map_err(|error| error.to_string())?;
            let payload = super::relocation::relocate_payload(&payload, &relocated)?;
            let destination = output.append(&payload).map_err(|error| error.to_string())?;
            relocated.insert(object, destination);
            copied_chunks += 1;
        }
        output.sync().map_err(|error| error.to_string())
    })();
    if let Err(error) = copy_result {
        drop(output);
        if output_guard.validate_path_identity().is_ok() {
            let _ = fs::remove_file(output_path);
        }
        return Err(error);
    }
    let output_file_bytes = fs::metadata(output_path)
        .map_err(|error| error.to_string())?
        .len();
    if discard.is_empty() && output_file_bytes != source_file_bytes {
        if output_guard.validate_path_identity().is_ok() {
            let _ = fs::remove_file(output_path);
        }
        return Err(format!(
            "reclamation changed file size without reclaimable chunks: source={source_file_bytes} output={output_file_bytes}"
        ));
    }
    if !discard.is_empty() && output_file_bytes >= source_file_bytes {
        if output_guard.validate_path_identity().is_ok() {
            let _ = fs::remove_file(output_path);
        }
        return Err(format!(
            "reclamation did not shrink file: source={source_file_bytes} output={output_file_bytes}"
        ));
    }

    Ok((
        ReclamationCopyStats {
            copied_chunks,
            reclaimed_payload_bytes,
            source_file_bytes,
            output_file_bytes,
        },
        output,
    ))
}

#[cfg(test)]
#[path = "reclamation_source_tests.rs"]
mod tests;
