//! Archive-owned conversion at accepted V1 marker slots, including repeats.
use super::StorageUpgradeError;
use crate::archive_codec::publication::{self, ArchivePublication};
use crate::container::publication_envelope::{self, PublicationClock, PublicationEnvelope};
use crate::{Archive, Container, ObjectRef};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Default)]
pub(super) struct ArchiveEnvelopePlan {
    pub(super) markers: BTreeMap<ObjectRef, Vec<u8>>,
    pub(super) carriers: HashSet<ObjectRef>,
}

pub(super) fn prepare(
    source: &mut Container,
    archive: &Archive,
) -> Result<ArchiveEnvelopePlan, StorageUpgradeError> {
    // The enclosing V1 Cva open admitted these exact facts and source proofs.
    let mut plan = ArchiveEnvelopePlan::default();
    let mut first = HashMap::new();
    for object in source.chunks()? {
        let payload = source.read(object)?;
        let Some(version) = crate::archive_history_codec::decode_record_version(&payload)? else {
            continue;
        };
        if archive.record_version(version.archive_version) != Some(&version) {
            return Err(crate::ArchiveError::InvalidArchiveRecordVersion.into());
        }
        let publication = if let Some(first_applying_archive_version) = first.get(&version.record) {
            ArchivePublication::Repeat {
                archive_version: version.archive_version,
                first_applying_archive_version: *first_applying_archive_version,
            }
        } else {
            let (record, applying) = archive.read_publication(source, version)?;
            if applying != version {
                return Err(crate::ArchiveError::InvalidArchiveRecordVersion.into());
            }
            first.insert(version.record, version.archive_version);
            ArchivePublication::Record {
                archive_version: version.archive_version,
                record,
            }
        };
        plan.carriers.insert(version.record);
        let body = publication::encode(&publication)?;
        plan.markers.insert(
            object,
            publication_envelope::encode(PublicationEnvelope {
                global_version: version.global_version,
                clock: PublicationClock::BindReserved,
                body: &body,
            })?,
        );
    }
    Ok(plan)
}
