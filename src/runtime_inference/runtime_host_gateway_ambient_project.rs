use super::*;
use crate::{ProjectRevisionRef, StoredFile};
use std::path::{Path, PathBuf};

impl GatewayRuntime {
    pub fn latest_project_revision_correlation_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<Option<crate::ProjectRevisionCorrelation>, ReliquaryRuntimeHostError> {
        let token = self.explicit_rel_token(instance, owner_id)?;
        let correlation = self.with_rel(&token, |runtime| {
            Ok(runtime.cva().latest_project_revision_correlation())
        })?;
        self.deliver_rel(instance, &token, correlation)
    }

    // Instance-local test instrumentation after real external I/O and before
    // commit/delivery. This is absent from the production gateway.
    #[cfg(test)]
    fn project_io_completed(&self) {
        let hook = self.project_io_test_hook.lock().unwrap().clone();
        if let Some(hook) = hook {
            hook();
        }
    }

    pub fn project_repository_root_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        project_dir: &Path,
    ) -> Result<PathBuf, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ManageProject)?;
        let correlation = self.with_rel(&token, |runtime| {
            runtime
                .cva
                .latest_project_revision_correlation()
                .ok_or_else(|| operation("Project REL has no repository correlation"))
        })?;
        let root = crate::project_environment::repository_root(project_dir, &correlation)
            .map_err(operation)?;
        #[cfg(test)]
        self.project_io_completed();
        self.deliver_rel(instance, &token, root)
    }

    pub fn validate_project_environment_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        project_dir: &Path,
    ) -> Result<ProjectRevisionRef, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ManageProject)?;
        let correlation = self.with_rel(&token, |runtime| {
            runtime
                .cva
                .latest_project_revision_correlation()
                .ok_or_else(|| operation("Project REL has no repository correlation"))
        })?;
        let current = crate::project_environment::current_revision_for(project_dir, &correlation)
            .map_err(operation)?;
        #[cfg(test)]
        self.project_io_completed();
        if current.repository.project_path != correlation.project_revision.repository.project_path {
            self.with_rel_mutation(&token, RelMutationFamily::Project, |runtime| {
                runtime
                    .cva
                    .correlate_project_revision_with_management(
                        current.clone(),
                        correlation.repository_management,
                    )
                    .map_err(operation)?;
                Ok(((), false, vec![]))
            })?;
        }
        self.deliver_rel(instance, &token, current)
    }

    pub fn checkpoint_manual_project_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        project_dir: &Path,
    ) -> Result<ProjectRevisionRef, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ManageProject)?;
        let correlation = self.with_rel(&token, |runtime| {
            runtime
                .cva
                .latest_project_revision_correlation()
                .ok_or_else(|| operation("Project REL has no repository correlation"))
        })?;
        // Filesystem/git work is outside the owner runtime lock.
        let revision =
            crate::project_environment::checkpoint_manual_lore(project_dir, &correlation)
                .map_err(operation)?;
        #[cfg(test)]
        self.project_io_completed();
        self.with_rel_mutation(&token, RelMutationFamily::Project, |runtime| {
            runtime
                .cva
                .correlate_project_revision_with_management(
                    revision.clone(),
                    correlation.repository_management,
                )
                .map_err(operation)?;
            Ok(((), false, vec![]))
        })?;
        self.deliver_rel(instance, &token, revision)
    }

    pub fn ingest_project_attachment_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        project_dir: &Path,
        source: &Path,
    ) -> Result<StoredFile, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ManageProject)?;
        let correlation = self.with_rel(&token, |runtime| {
            runtime
                .cva
                .latest_project_revision_correlation()
                .ok_or_else(|| operation("Project REL has no repository correlation"))
        })?;
        // Read/hash the attachment outside the owner lock.
        let prepared =
            crate::project_environment::prepare_upload(&correlation, project_dir, source)
                .map_err(operation)?;
        #[cfg(test)]
        self.project_io_completed();
        let file = self.with_rel_mutation(&token, RelMutationFamily::Project, |runtime| {
            runtime
                .cva
                .correlate_project_revision_with_management(
                    prepared.reference.revision.clone(),
                    correlation.repository_management,
                )
                .map_err(operation)?;
            let file = runtime
                .cva
                .register_project_file(
                    prepared.filename,
                    prepared.mime_type,
                    prepared.byte_length,
                    prepared.reference,
                )
                .map_err(operation)?;
            let id = file.id;
            Ok((file, false, vec![RelInvalidation::File(id)]))
        })?;
        self.deliver_rel(instance, &token, file)
    }

    pub fn resolved_file_bytes_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        project_dir: Option<&Path>,
        file_id: crate::FileId,
    ) -> Result<Vec<u8>, ReliquaryRuntimeHostError> {
        let token = self.explicit_rel_token(instance, owner_id)?;
        let reference =
            self.with_rel(&token, |runtime| Ok(runtime.cva.project_file_ref(file_id)))?;
        let bytes = match reference {
            Some(reference) => {
                let project_dir = project_dir
                    .ok_or_else(|| operation("Project file has no local repository association"))?;
                let bytes = crate::project_environment::read_project_file(project_dir, &reference)
                    .map_err(operation)?;
                #[cfg(test)]
                self.project_io_completed();
                bytes
            }
            None => self.with_rel(&token, |runtime| {
                runtime.cva.file_bytes(file_id).map_err(operation)
            })?,
        };
        self.deliver_rel(instance, &token, bytes)
    }
}
