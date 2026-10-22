use crate::project_history_codec::{decode, encode};
use crate::{
    Container, CvaError, ProjectRepositoryManagement, ProjectRevisionCorrelation,
    ProjectRevisionRef, RelSemanticCut,
};

#[derive(Default)]
pub(crate) struct ProjectHistoryStore {
    records: Vec<ProjectRevisionCorrelation>,
}

impl ProjectHistoryStore {
    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), CvaError> {
        let Some(record) = decode(payload).map_err(CvaError::ProjectHistory)? else {
            return Ok(());
        };
        self.validate_record(&record)?;
        if let Some(previous) = self.records.last() {
            validate_repository_identity(&previous.project_revision, &record.project_revision)?;
        }
        let expected_sequence = match self.records.last() {
            Some(previous) => previous.sequence.checked_add(1).ok_or_else(|| {
                CvaError::ProjectHistory("project correlation sequence exhausted".into())
            })?,
            None => 1,
        };
        if record.sequence != expected_sequence {
            return Err(CvaError::ProjectHistory(
                "project correlation sequence is not contiguous".into(),
            ));
        }
        self.records.push(record);
        Ok(())
    }

    pub(crate) fn put(
        &mut self,
        container: &mut Container,
        rel_cut: RelSemanticCut,
        project_revision: ProjectRevisionRef,
        repository_management: ProjectRepositoryManagement,
    ) -> Result<(ProjectRevisionCorrelation, bool), CvaError> {
        validate_revision(&project_revision)?;
        if let Some(previous) = self.records.last() {
            validate_repository_identity(&previous.project_revision, &project_revision)?;
        }
        if let Some(existing) = self.records.last()
            && existing.rel_cut == rel_cut
            && existing.project_revision == project_revision
            && existing.repository_management == repository_management
        {
            return Ok((existing.clone(), false));
        }
        let sequence = match self.records.last() {
            Some(record) => record.sequence.checked_add(1).ok_or_else(|| {
                CvaError::ProjectHistory("project correlation sequence exhausted".into())
            })?,
            None => 1,
        };
        let record = ProjectRevisionCorrelation {
            sequence,
            rel_cut,
            project_revision,
            repository_management,
        };
        let payload = encode(&record).map_err(CvaError::ProjectHistory)?;
        container.append(&payload)?;
        container.sync()?;
        self.records.push(record.clone());
        Ok((record, true))
    }

    pub(crate) fn records(&self) -> Vec<ProjectRevisionCorrelation> {
        self.records.clone()
    }

    pub(crate) fn latest(&self) -> Option<ProjectRevisionCorrelation> {
        self.records.last().cloned()
    }

    fn validate_record(&self, record: &ProjectRevisionCorrelation) -> Result<(), CvaError> {
        if record.sequence == 0 {
            return Err(CvaError::ProjectHistory(
                "project correlation sequence must be non-zero".into(),
            ));
        }
        validate_revision(&record.project_revision)
    }
}

fn validate_repository_identity(
    previous: &ProjectRevisionRef,
    next: &ProjectRevisionRef,
) -> Result<(), CvaError> {
    if previous.repository.kind != next.repository.kind
        || previous.repository.repository_id != next.repository.repository_id
    {
        return Err(CvaError::ProjectHistory(
            "project repository identity must remain stable across correlation history".into(),
        ));
    }
    Ok(())
}

fn validate_revision(project_revision: &ProjectRevisionRef) -> Result<(), CvaError> {
    let repository = &project_revision.repository;
    if repository.repository_id.trim().is_empty() {
        return Err(CvaError::ProjectHistory(
            "project repository id must not be empty".into(),
        ));
    }
    if project_revision.revision.trim().is_empty() {
        return Err(CvaError::ProjectHistory(
            "project revision must not be empty".into(),
        ));
    }
    let path = repository.project_path.as_str();
    let windows_absolute = path
        .as_bytes()
        .get(0..2)
        .is_some_and(|prefix| prefix[0].is_ascii_alphabetic() && prefix[1] == b':');
    let invalid_component = path != "."
        && path
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."));
    if path.is_empty()
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || windows_absolute
        || invalid_component
    {
        return Err(CvaError::ProjectHistory(
            "project path must be a normalized repository-relative path".into(),
        ));
    }
    Ok(())
}
