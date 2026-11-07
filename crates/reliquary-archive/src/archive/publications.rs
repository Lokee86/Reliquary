use super::*;
use crate::publication::{self, Body, Publication};
use crate::{ArchivePublicationBinding, ArchivePublicationCoordinate};
use reliquary_container::ObjectRef;

impl Archive {
    pub fn publication_bindings(&self) -> &[ArchivePublicationBinding] {
        &self.publication_bindings
    }

    pub(super) fn replay_publication(
        &mut self,
        container: &Container,
        object: ObjectRef,
        publication: Publication<'_>,
    ) -> Result<(), ArchiveError> {
        let archive_version = publication.archive_version;
        if self.next_archive_version()? != archive_version {
            return Err(invalid_coordinate());
        }
        let coordinate = ArchivePublicationCoordinate {
            archive_version,
            global_version: publication.envelope.version,
            transaction_time_ns: container.transaction_time_ns(publication.envelope.version),
            bound_reservation: publication.envelope.bound_reservation,
        };
        let (first_local, first_global) = match publication.body {
            Body::Applying(payload) => {
                let record = decode_record(payload)?.ok_or_else(invalid_coordinate)?;
                if matches!(record, Record::FileBlob(..)) {
                    return Err(invalid_coordinate());
                }
                self.apply_record(record)?;
                if self.archive_version != archive_version {
                    return Err(invalid_coordinate());
                }
                (archive_version, coordinate.global_version)
            }
            Body::Repeat(origin) => {
                let global_version = self.first_applying(origin)?.global_version;
                self.archive_version = archive_version;
                (origin, global_version)
            }
        };
        self.record_binding(coordinate, object, first_local, first_global);
        Ok(())
    }

    pub(super) fn check_publication(
        &self,
        container: &Container,
        archive_version: u64,
        coordinate: Option<ArchivePublicationCoordinate>,
    ) -> Result<(), ArchiveError> {
        let Some(coordinate) = coordinate else {
            return Ok(());
        };
        let valid_global = if coordinate.bound_reservation {
            container.has_unbound_version_reservation(coordinate.global_version)
                && container.transaction_time_ns(coordinate.global_version)
                    == coordinate.transaction_time_ns
        } else {
            container.latest_version().checked_add(1) == Some(coordinate.global_version)
                && coordinate.global_version.checked_add(1).is_some()
        };
        if coordinate.archive_version != archive_version || !valid_global {
            return Err(invalid_coordinate());
        }
        Ok(())
    }

    pub(super) fn persist_applying(
        &mut self,
        container: &mut Container,
        coordinate: Option<ArchivePublicationCoordinate>,
        bytes: &[u8],
    ) -> Result<(), ArchiveError> {
        if let Some(coordinate) = coordinate {
            self.persist_publication(
                container,
                coordinate,
                Body::Applying(bytes),
                coordinate.archive_version,
                coordinate.global_version,
            )
        } else {
            container.append(bytes)?;
            container.sync()?;
            Ok(())
        }
    }

    /// Retain an accepted repeat at its own local/global cut without reapplying its origin.
    pub fn append_publication_repeat(
        &mut self,
        container: &mut Container,
        coordinate: ArchivePublicationCoordinate,
        first_applying_archive_version: u64,
    ) -> Result<(), ArchiveError> {
        self.bind_container(container)?;
        let archive_version = self.next_archive_version()?;
        self.check_publication(container, archive_version, Some(coordinate))?;
        let first_global = self
            .first_applying(first_applying_archive_version)?
            .global_version;
        self.persist_publication(
            container,
            coordinate,
            Body::Repeat(first_applying_archive_version),
            first_applying_archive_version,
            first_global,
        )?;
        self.archive_version = archive_version;
        Ok(())
    }

    fn first_applying(&self, version: u64) -> Result<&ArchivePublicationBinding, ArchiveError> {
        self.publication_bindings
            .binary_search_by_key(&version, |binding| binding.archive_version)
            .ok()
            .map(|position| &self.publication_bindings[position])
            .filter(|binding| binding.first_applying_archive_version == version)
            .ok_or_else(invalid_coordinate)
    }

    fn persist_publication(
        &mut self,
        container: &mut Container,
        coordinate: ArchivePublicationCoordinate,
        body: Body<'_>,
        first_local: u64,
        first_global: u64,
    ) -> Result<(), ArchiveError> {
        let bytes = publication::encode(coordinate.archive_version, body);
        let object = if coordinate.bound_reservation {
            container.append_bound_publication_envelope(coordinate.global_version, &bytes)?
        } else {
            let (_, object) =
                container.append_publication_envelope(&bytes, coordinate.transaction_time_ns)?;
            object
        };
        container.sync()?;
        self.record_binding(coordinate, object, first_local, first_global);
        Ok(())
    }

    fn record_binding(
        &mut self,
        coordinate: ArchivePublicationCoordinate,
        object: ObjectRef,
        first_local: u64,
        first_global: u64,
    ) {
        self.publication_bindings.push(ArchivePublicationBinding {
            archive_version: coordinate.archive_version,
            global_version: coordinate.global_version,
            transaction_time_ns: coordinate.transaction_time_ns,
            bound_reservation: coordinate.bound_reservation,
            first_applying_archive_version: first_local,
            first_applying_global_version: first_global,
            object,
        });
    }
}

fn invalid_coordinate() -> ArchiveError {
    ArchiveError::CorruptRecord("invalid Archive publication coordinate")
}
