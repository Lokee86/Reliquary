use super::*;
use reliquary_application::{VerifiedExternalSource, prepare_project_attachment};

impl Instance {
    /// Verify one owner-admitted project attachment against its repository
    /// without holding the owner lock during repository I/O.
    pub fn verify_project_attachment(
        &self,
        owner: OwnerId,
        conversation_id: &str,
        node_id: &str,
        attachment_id: [u8; 32],
        source: &crate::TrustedProjectSource,
    ) -> Result<VerifiedExternalSource, GatewayError> {
        let prepare = |rel: &RelSession| {
            prepare_project_attachment(
                rel,
                owner.as_bytes(),
                conversation_id,
                node_id,
                attachment_id,
            )
        };
        let prepared = match self.reliquary(owner) {
            Ok(session) => prepare(&session),
            Err(GatewayError::WriterUnavailable) => self.with_writable_reliquary(owner, prepare)?,
            Err(error) => return Err(error),
        }
        .map_err(|error| OwnerSessionError::Record(error.to_string()))?;
        let repository = prepared.reference().revision.repository.clone();
        let project_dir = source.authorize(self, owner, &repository)?;
        let verified = prepared.verify(project_dir);
        self.ensure_credential_active()?;
        source.authorize(self, owner, &repository)?;
        Ok(verified.map_err(|error| OwnerSessionError::Record(error.to_string()))?)
    }
}
