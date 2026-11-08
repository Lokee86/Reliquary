use super::{FixtureDir, admission};
use reliquary_memory::{
    AuthorizedProjectCorrelation, ContentId, ConversationQueue, ConversationSubmissionId,
    FinalizeCapture, Gateway, Instance, OwnerId, ProductError, ProductErrorKind,
    ProjectContextAttachmentRef, ProjectCorrelationRequest, ProjectFileRef, ProjectRepositoryKind,
    ProjectRepositoryRef, ProjectRevisionRef, TrustedProjectSource, VerifiedExternalSource,
};
use std::path::PathBuf;

#[test]
fn project_source_binding_rejects_another_gateway_for_the_same_owner() {
    let dir = FixtureDir::new();
    let rel = dir.create_rel();
    let first = Gateway::new();
    let second = Gateway::new();
    let owner = first.mount_rel(&rel, false).unwrap();
    assert_eq!(second.mount_rel(&rel, false).unwrap(), owner);
    let (_, viewer, _) = admission(&second, owner);
    let source = first
        .bind_project_source(
            "alice",
            owner,
            ProjectRepositoryRef {
                kind: ProjectRepositoryKind::Git,
                repository_id: "repo".into(),
                project_path: "project".into(),
            },
            dir.0.clone(),
        )
        .unwrap();
    let denied = viewer.verify_project_attachment(owner, "chat", "turn", [1; 32], &source);
    assert_eq!(denied.unwrap_err().kind(), ProductErrorKind::Unauthorized);
}

// Compile the full project source path without requiring a VCS checkout.
#[allow(dead_code)]
fn project_source_contract(
    gateway: &Gateway,
    instance: &Instance,
    queue: &mut ConversationQueue,
    id: &ConversationSubmissionId,
    owner: OwnerId,
    capture: FinalizeCapture,
    attachment: ProjectContextAttachmentRef,
    root: PathBuf,
) -> Result<(), ProductError> {
    let repository = ProjectRepositoryRef {
        kind: ProjectRepositoryKind::Git,
        repository_id: "repo".into(),
        project_path: "project".into(),
    };
    let source: TrustedProjectSource =
        gateway.bind_project_source(instance.principal(), owner, repository.clone(), root)?;
    let request = ProjectCorrelationRequest {
        authorized_owner_id: owner.as_bytes(),
        authorized_principal_id: instance.principal().into(),
        repository,
        revision: "revision".into(),
        path: "src/main.rs".into(),
        expected_content_hash: [1; 32],
        filename: "main.rs".into(),
        mime_type: None,
    };
    let correlation: AuthorizedProjectCorrelation =
        instance.prepare_project_correlation(owner, request, &source)?;
    let revision = instance.conversation_revision(queue, &id.conversation_id)?;
    instance.finalize_project_correlated_submission(
        queue,
        id,
        revision,
        capture,
        vec![correlation],
    )?;
    let verified: VerifiedExternalSource = instance.verify_project_attachment(
        owner,
        &attachment.conversation_id,
        &attachment.source_node_id,
        attachment.attachment_id,
        &source,
    )?;
    let _: ContentId = verified.content_id;
    let file: ProjectFileRef = verified.reference;
    let _: ProjectRevisionRef = file.revision;
    source.revoke();
    Ok(())
}
