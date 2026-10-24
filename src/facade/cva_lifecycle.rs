use crate::{Container, Cva, CvaError};
use std::path::Path;

impl Cva {
    pub fn create(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::create_rel(path, None)
    }

    pub fn create_project(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::create_rel(path, Some("Project"))
    }

    pub fn create_organization(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::create_rel(path, Some("Organization"))
    }

    pub fn create_connection(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::create_rel(path, Some("Connection"))
    }

    pub fn open_project(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::open(path)
    }

    pub fn open_organization(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::open(path)
    }

    pub fn open_connection(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::open(path)
    }

    fn create_rel(path: impl AsRef<Path>, type_label: Option<&str>) -> Result<Self, CvaError> {
        crate::rel_lifecycle_execution::create(
            path.as_ref(),
            None,
            None,
            type_label.map(str::to_owned),
            true,
        )
    }

    pub(crate) fn create_rel_with_uuid(
        path: impl AsRef<Path>,
        owner_uuid: [u8; 16],
        type_label: Option<String>,
    ) -> Result<Self, CvaError> {
        crate::rel_lifecycle_execution::create(
            path.as_ref(),
            Some(owner_uuid),
            None,
            type_label,
            false,
        )
    }

    pub(crate) fn create_legacy_scope_with_uuid(
        path: impl AsRef<Path>,
        scope: crate::ReliquaryScopeKind,
        owner_uuid: [u8; 16],
    ) -> Result<Self, CvaError> {
        crate::rel_lifecycle_execution::create(
            path.as_ref(),
            Some(owner_uuid),
            Some(scope),
            None,
            false,
        )
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        crate::rel_lifecycle_execution::open(path.as_ref(), false)
    }

    /// Replays a coherent read-only snapshot without acquiring writer authority.
    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        crate::rel_lifecycle_execution::open(path.as_ref(), true)
    }

    pub(crate) fn open_from_container(container: Container) -> Result<Self, CvaError> {
        crate::rel_lifecycle_execution::from_container(container)
    }
}
