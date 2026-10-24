//! Physical REL construction and coherent replay-session admission.
use super::{OwnerError, ReliquaryOwner, ReliquaryReplay, initialize_reliquary};
use crate::{Container, ContainerIdentity, FileKind, RelMetadata, ReliquaryScopeKind};
use std::path::Path;

pub(crate) fn create(
    path: &Path,
    owner_uuid: Option<[u8; 16]>,
    scope: Option<ReliquaryScopeKind>,
    type_label: Option<String>,
    sync_metadata: bool,
) -> Result<ReliquaryOwner, OwnerError> {
    let identity = ContainerIdentity {
        file_kind: FileKind::Reliquary,
        scope,
    };
    let container = match owner_uuid {
        Some(uuid) => Container::create_with_identity_and_uuid(path, identity, uuid)?,
        None => Container::create_with_identity(path, identity)?,
    };
    let mut owner = initialize_reliquary(container)?;
    if let Some(label) = type_label {
        let label = label.trim();
        let type_label = (!label.is_empty()).then(|| label.to_owned());
        owner
            .rel_metadata
            .put(
                &mut owner.container,
                RelMetadata {
                    type_label,
                    dependencies: Vec::new(),
                },
            )
            .map_err(OwnerError::RelMetadata)?;
        if sync_metadata {
            owner.container.sync()?;
        }
    }
    Ok(owner)
}

pub(crate) fn open(path: &Path, read_only: bool) -> Result<ReliquaryReplay, OwnerError> {
    let container = if read_only {
        Container::open_read_only(path)?
    } else {
        Container::open(path)?
    };
    ReliquaryReplay::read(container)
}
