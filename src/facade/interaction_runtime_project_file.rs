use crate::{InteractionError, InteractionRuntime, StoredFile};

impl InteractionRuntime {
    pub fn attach_project_file(
        &mut self,
        session_id: &str,
        message_id: &str,
        file: StoredFile,
    ) -> Result<(), InteractionError> {
        if self.cva.runtime_project_file_ref(file.id).is_none() {
            return Err(crate::CvaError::ProjectFile(
                "project attachment is not registered".into(),
            )
            .into());
        }
        self.message_mut(session_id, message_id)?
            .project_attachments
            .push(file);
        Ok(())
    }
}
