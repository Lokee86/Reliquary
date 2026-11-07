use crate::project_file_binding_codec::{decode, encode};
use crate::{Container, ContainerError, FileId, ProjectFileRef};
use std::collections::HashMap;

#[derive(Debug)]
pub(crate) enum ProjectFileStoreError {
    Project(String),
    Container(ContainerError),
}

impl From<ContainerError> for ProjectFileStoreError {
    fn from(error: ContainerError) -> Self {
        Self::Container(error)
    }
}

impl From<String> for ProjectFileStoreError {
    fn from(error: String) -> Self {
        Self::Project(error)
    }
}

#[derive(Default)]
pub(crate) struct ProjectFileStore {
    records: HashMap<FileId, ProjectFileRef>,
}

impl ProjectFileStore {
    pub(crate) fn accepted_facts(&self) -> crate::source_identity::ProjectFileFacts<'_> {
        crate::source_identity::ProjectFileFacts::new(&self.records)
    }

    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), ProjectFileStoreError> {
        let Some((file_id, reference)) = decode(payload).map_err(ProjectFileStoreError::Project)?
        else {
            return Ok(());
        };
        validate_project_file_ref(&reference)?;
        match self.records.get(&file_id) {
            Some(existing) if existing == &reference => Ok(()),
            Some(_) => Err(ProjectFileStoreError::Project(
                "conflicting project file binding".into(),
            )),
            None => {
                self.records.insert(file_id, reference);
                Ok(())
            }
        }
    }

    pub(crate) fn put(
        &mut self,
        container: &mut Container,
        file_id: FileId,
        reference: ProjectFileRef,
    ) -> Result<bool, ProjectFileStoreError> {
        validate_project_file_ref(&reference)?;
        if let Some(existing) = self.records.get(&file_id) {
            return if existing == &reference {
                Ok(false)
            } else {
                Err(ProjectFileStoreError::Project(
                    "conflicting project file binding".into(),
                ))
            };
        }
        container.append(&encode(file_id, &reference).map_err(ProjectFileStoreError::Project)?)?;
        container.sync()?;
        self.records.insert(file_id, reference);
        Ok(true)
    }

    #[cfg(feature = "history-oracle-fixtures")]
    pub(crate) fn witness_bindings(&self) -> impl Iterator<Item = (&FileId, &ProjectFileRef)> {
        self.records.iter()
    }

    pub(crate) fn get(&self, file_id: FileId) -> Option<&ProjectFileRef> {
        self.records.get(&file_id)
    }

    pub(crate) fn contains(&self, file_id: FileId) -> bool {
        self.records.contains_key(&file_id)
    }
}

pub(crate) fn validate_project_file_ref(
    reference: &ProjectFileRef,
) -> Result<(), ProjectFileStoreError> {
    let repository = &reference.revision.repository;
    validate_relative_path(&repository.project_path, true, "project path")?;
    validate_relative_path(&reference.path, false, "file path")?;
    if repository.repository_id.trim().is_empty() {
        return Err(ProjectFileStoreError::Project(
            "project repository id must not be empty".into(),
        ));
    }
    if reference.revision.revision.trim().is_empty() {
        return Err(ProjectFileStoreError::Project(
            "project revision must not be empty".into(),
        ));
    }
    if repository.project_path != "." {
        let prefix = format!("{}/", repository.project_path);
        if !reference.path.starts_with(&prefix) {
            return Err(ProjectFileStoreError::Project(
                "project file path is outside the associated project subtree".into(),
            ));
        }
    }
    Ok(())
}

fn validate_relative_path(
    path: &str,
    allow_root: bool,
    name: &str,
) -> Result<(), ProjectFileStoreError> {
    if allow_root && path == "." {
        return Ok(());
    }
    let windows_absolute = path
        .as_bytes()
        .get(0..2)
        .is_some_and(|prefix| prefix[0].is_ascii_alphabetic() && prefix[1] == b':');
    let invalid_component = path
        .split('/')
        .any(|component| component.is_empty() || matches!(component, "." | ".."));
    if path.is_empty()
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || windows_absolute
        || invalid_component
    {
        return Err(ProjectFileStoreError::Project(format!(
            "{name} must be a normalized repository-relative path"
        )));
    }
    Ok(())
}
