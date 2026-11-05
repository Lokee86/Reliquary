//! Read-only verification for Archive-associated external project files.

use reliquary_owner_session::RelSession;
use reliquary_owner_session::model::{
    ContentId, ProjectFileRef, ProjectRepositoryKind, SourceAttachment,
    validate_project_file_reference,
};
use sha2::{Digest, Sha256};
use std::{fmt, path::Path};

pub const MAX_PROJECT_SOURCE_BYTES: u64 = 16 << 20;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedExternalSource {
    pub owner_id: [u8; 16],
    pub conversation_id: String,
    pub source_node_id: String,
    pub attachment_id: [u8; 32],
    pub content_id: ContentId,
    pub reference: ProjectFileRef,
    pub bytes: Vec<u8>,
}

/// Owner-authorized immutable attachment metadata. Repository reads can run
/// after the caller has released the owner session or its writer lock.
pub struct PreparedProjectAttachment {
    owner_id: [u8; 16],
    conversation_id: String,
    source_node_id: String,
    attachment_id: [u8; 32],
    content_id: ContentId,
    byte_length: u64,
    reference: ProjectFileRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExternalSourceError {
    OwnerMismatch,
    AttachmentUnavailable,
    NotProjectReference,
    InvalidReference(String),
    SourceTooLarge,
    RepositoryRead(String),
    ByteLengthMismatch,
    ContentIdentityMismatch,
}

impl fmt::Display for ExternalSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OwnerMismatch => f.write_str("project source owner does not match authorization"),
            Self::AttachmentUnavailable => f.write_str("owner Archive attachment is unavailable"),
            Self::NotProjectReference => f.write_str("Archive attachment has no project reference"),
            Self::InvalidReference(error) => {
                write!(f, "invalid Archive project reference: {error}")
            }
            Self::SourceTooLarge => f.write_str("project source exceeds the read limit"),
            Self::RepositoryRead(error) => write!(f, "external repository read failed: {error}"),
            Self::ByteLengthMismatch => f.write_str("project source byte length changed"),
            Self::ContentIdentityMismatch => {
                f.write_str("project source bytes do not match Archive identity")
            }
        }
    }
}

impl std::error::Error for ExternalSourceError {}

/// Re-read one project attachment already associated with this REL owner's
/// Archive. Repository identity, revision, path, and optional content hash are
/// checked by Project Environment. This returns a volatile verification result;
/// it does not add a transcript turn or persist a host-consumption receipt.
pub fn verify_project_attachment(
    rel: &RelSession,
    authorized_owner_id: [u8; 16],
    conversation_id: &str,
    source_node_id: &str,
    attachment_id: [u8; 32],
    project_dir: &Path,
) -> Result<VerifiedExternalSource, ExternalSourceError> {
    prepare_project_attachment(
        rel,
        authorized_owner_id,
        conversation_id,
        source_node_id,
        attachment_id,
    )?
    .verify(project_dir)
}

pub fn prepare_project_attachment(
    rel: &RelSession,
    authorized_owner_id: [u8; 16],
    conversation_id: &str,
    source_node_id: &str,
    attachment_id: [u8; 32],
) -> Result<PreparedProjectAttachment, ExternalSourceError> {
    let owner_id = rel.owner_id();
    if owner_id != authorized_owner_id {
        return Err(ExternalSourceError::OwnerMismatch);
    }
    let attachment: SourceAttachment = rel
        .archive_source_attachments(conversation_id, source_node_id)
        .into_iter()
        .find(|attachment| attachment.id == attachment_id)
        .ok_or(ExternalSourceError::AttachmentUnavailable)?;
    let reference = attachment
        .project_reference
        .ok_or(ExternalSourceError::NotProjectReference)?;
    validate_project_file_reference(&reference)
        .map_err(|error| ExternalSourceError::InvalidReference(error.to_string()))?;
    if attachment.byte_length > MAX_PROJECT_SOURCE_BYTES {
        return Err(ExternalSourceError::SourceTooLarge);
    }
    Ok(PreparedProjectAttachment {
        owner_id,
        conversation_id: conversation_id.to_owned(),
        source_node_id: source_node_id.to_owned(),
        attachment_id,
        content_id: attachment.content_id,
        byte_length: attachment.byte_length,
        reference,
    })
}

impl PreparedProjectAttachment {
    pub fn reference(&self) -> &ProjectFileRef {
        &self.reference
    }

    pub fn verify(self, project_dir: &Path) -> Result<VerifiedExternalSource, ExternalSourceError> {
        let reference = self.reference;
        let bytes = match reference.revision.repository.kind {
            ProjectRepositoryKind::Git => {
                reliquary_project_environment::read_project_file_reference(project_dir, &reference)
            }
            ProjectRepositoryKind::Lore => {
                reliquary_project_environment::read_lore_project_file(project_dir, &reference)
            }
        }
        .map_err(ExternalSourceError::RepositoryRead)?;
        if bytes.len() as u64 != self.byte_length {
            return Err(ExternalSourceError::ByteLengthMismatch);
        }
        let actual_content_id = ContentId(Sha256::digest(&bytes).into());
        if actual_content_id != self.content_id
            || reference
                .content_hash
                .is_some_and(|expected| expected != actual_content_id.0)
        {
            return Err(ExternalSourceError::ContentIdentityMismatch);
        }
        Ok(VerifiedExternalSource {
            owner_id: self.owner_id,
            conversation_id: self.conversation_id,
            source_node_id: self.source_node_id,
            attachment_id: self.attachment_id,
            content_id: actual_content_id,
            reference,
            bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_archive::{
        Archive, AttachmentInput, ProjectAttachmentInput, ProjectRepositoryRef, ProjectRevisionRef,
        TurnInput,
    };
    use reliquary_container::{Container, ContainerIdentity, ReliquaryScope};
    use reliquary_owner_session::GraphAdmission;
    use reliquary_vector_store::{
        ArchiveVectorStore, CompatibilityProfileStore, MemoryVectorStore, PackedVectorStore,
        VectorGenerationStore,
    };
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
            fs::create_dir_all(&root).unwrap();
            let path = root.join(format!(
                "external-source-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn git(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    #[test]
    fn validates_archive_associated_external_bytes_without_adding_transcript_authority() {
        let temp = Temp::new();
        let repo = temp.0.join("repo");
        fs::create_dir_all(repo.join("project/docs")).unwrap();
        git(&repo, &["init", "-q"]);
        git(&repo, &["config", "user.name", "Source Tests"]);
        git(
            &repo,
            &["config", "user.email", "source-tests@example.invalid"],
        );
        git(
            &repo,
            &[
                "config",
                "--local",
                "warlock.repositoryId",
                "external-test-owner",
            ],
        );
        let bytes = b"repository-owned bytes\0";
        fs::write(repo.join("project/docs/source.bin"), bytes).unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-qm", "source revision"]);
        let revision = git(&repo, &["rev-parse", "HEAD"]);
        let content_id = ContentId(Sha256::digest(bytes).into());
        let reference = ProjectFileRef {
            revision: ProjectRevisionRef {
                repository: ProjectRepositoryRef {
                    kind: ProjectRepositoryKind::Git,
                    repository_id: "external-test-owner".into(),
                    project_path: "project".into(),
                },
                revision,
            },
            path: "project/docs/source.bin".into(),
            content_hash: Some(content_id.0),
        };

        let owner_id = [111; 16];
        let rel_path = temp.0.join("owner.rel");
        let mut container = Container::create(
            &rel_path,
            ContainerIdentity::reliquary(ReliquaryScope::Project),
            owner_id,
        )
        .unwrap();
        PackedVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        CompatibilityProfileStore::default()
            .initialize(&mut container)
            .unwrap();
        ArchiveVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        MemoryVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        VectorGenerationStore::empty()
            .initialize(&mut container)
            .unwrap();
        let mut archive = Archive::open(&container).unwrap();
        archive
            .accept_turn(
                &mut container,
                TurnInput {
                    id: "source-turn".into(),
                    conversation_id: "conversation".into(),
                    parent_id: None,
                    role: "user".into(),
                    principal_id: None,
                    timestamp_ns: 1,
                    content: "Please inspect the linked file.".into(),
                    attachments: vec![AttachmentInput {
                        filename: "native.txt".into(),
                        mime_type: None,
                        bytes: b"native attachment".to_vec(),
                    }],
                    project_attachments: vec![ProjectAttachmentInput {
                        filename: "source.bin".into(),
                        mime_type: Some("application/octet-stream".into()),
                        content_id,
                        byte_length: bytes.len() as u64,
                        reference,
                    }],
                },
            )
            .unwrap();
        container.sync().unwrap();
        drop(archive);
        drop(container);
        let admission = GraphAdmission::for_replay(owner_id, [], [], [], [], []);
        let rel = RelSession::open_path(&rel_path, &admission, [], []).unwrap();
        let attachments = rel.archive_source_attachments("conversation", "source-turn");
        let external = attachments
            .iter()
            .find(|attachment| attachment.project_reference.is_some())
            .unwrap();
        let native = attachments
            .iter()
            .find(|attachment| attachment.project_reference.is_none())
            .unwrap();

        let prepared =
            prepare_project_attachment(&rel, owner_id, "conversation", "source-turn", external.id)
                .unwrap();
        drop(rel);
        let verified = prepared.verify(&repo.join("project")).unwrap();
        let rel = RelSession::open_path(&rel_path, &admission, [], []).unwrap();
        assert_eq!(verified.bytes, bytes);
        assert_eq!(verified.content_id, content_id);
        assert!(
            !rel.resolved_archive_turn("conversation", "source-turn")
                .unwrap()
                .unwrap()
                .content
                .contains("repository-owned bytes")
        );
        assert_eq!(
            verify_project_attachment(
                &rel,
                [112; 16],
                "conversation",
                "source-turn",
                external.id,
                &repo.join("project"),
            ),
            Err(ExternalSourceError::OwnerMismatch)
        );
        assert_eq!(
            verify_project_attachment(
                &rel,
                owner_id,
                "conversation",
                "source-turn",
                native.id,
                &repo.join("project"),
            ),
            Err(ExternalSourceError::NotProjectReference)
        );
        assert_eq!(
            verify_project_attachment(
                &rel,
                owner_id,
                "other-conversation",
                "source-turn",
                external.id,
                &repo.join("project"),
            ),
            Err(ExternalSourceError::AttachmentUnavailable)
        );
    }
}
