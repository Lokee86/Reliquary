//! Admit VCS-proven project file references on an accepted native turn.
//!
//! Repository reads happen while preparing the opaque correlation, before the
//! caller obtains the REL writer lease. The durable Archive record keeps only
//! the exact repository reference, content identity, and length; project bytes
//! remain owned by Git or Lore.

use reliquary_owner_session::{
    OwnerSessionError, RelWriter,
    model::{
        Branch, ContentId, ProjectAttachmentInput, ProjectFileRef, ProjectRepositoryKind,
        ProjectRepositoryRef, ProjectRevisionRef, TurnInput, validate_project_file_reference,
    },
};
use sha2::{Digest, Sha256};
use std::{fmt, path::Path};

pub const MAX_CORRELATED_PROJECT_FILE_BYTES: u64 = 16 << 20;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCorrelationRequest {
    /// REL owner authorized by the caller's project grant.
    pub authorized_owner_id: [u8; 16],
    /// Principal whose project grant selected this exact source.
    pub authorized_principal_id: String,
    pub repository: ProjectRepositoryRef,
    pub revision: String,
    /// Repository-root-relative, exact VCS path.
    pub path: String,
    /// Hash already bound by the authorized project file selection.
    pub expected_content_hash: [u8; 32],
    pub filename: String,
    pub mime_type: Option<String>,
}

/// A VCS-verified, owner-bound correlation. Fields stay private so a caller
/// cannot substitute repository metadata after source verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedProjectCorrelation {
    owner_id: [u8; 16],
    principal_id: String,
    attachment: ProjectAttachmentInput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectCorrelationError {
    OwnerMismatch,
    InvalidReference(String),
    RepositoryRead(String),
    SourceTooLarge,
    ContentHashMismatch,
}

impl fmt::Display for ProjectCorrelationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OwnerMismatch => f.write_str("project correlation owner is not authorized"),
            Self::InvalidReference(error) => write!(f, "invalid project reference: {error}"),
            Self::RepositoryRead(error) => write!(f, "project repository read failed: {error}"),
            Self::SourceTooLarge => f.write_str("project source exceeds the read limit"),
            Self::ContentHashMismatch => {
                f.write_str("project source hash does not match selection")
            }
        }
    }
}

impl std::error::Error for ProjectCorrelationError {}

/// Read an exact Git/Lore revision through Project Environment and bind its
/// verified bytes to an owner-qualified Archive attachment descriptor.
///
/// This function does not open or mutate REL state. Its caller must recheck
/// principal and project grants after this repository I/O and before commit.
pub fn prepare_project_correlation(
    target_owner_id: [u8; 16],
    request: ProjectCorrelationRequest,
    project_dir: &Path,
) -> Result<PreparedProjectCorrelation, ProjectCorrelationError> {
    if target_owner_id != request.authorized_owner_id {
        return Err(ProjectCorrelationError::OwnerMismatch);
    }
    let reference = ProjectFileRef {
        revision: ProjectRevisionRef {
            repository: request.repository,
            revision: request.revision,
        },
        path: request.path,
        content_hash: Some(request.expected_content_hash),
    };
    validate_project_file_reference(&reference)
        .map_err(|error| ProjectCorrelationError::InvalidReference(error.to_string()))?;

    let bytes = match reference.revision.repository.kind {
        ProjectRepositoryKind::Git => {
            reliquary_project_environment::read_project_file_reference(project_dir, &reference)
        }
        ProjectRepositoryKind::Lore => {
            reliquary_project_environment::read_lore_project_file(project_dir, &reference)
        }
    }
    .map_err(ProjectCorrelationError::RepositoryRead)?;
    let byte_length =
        u64::try_from(bytes.len()).map_err(|_| ProjectCorrelationError::SourceTooLarge)?;
    if byte_length > MAX_CORRELATED_PROJECT_FILE_BYTES {
        return Err(ProjectCorrelationError::SourceTooLarge);
    }
    let content_id = ContentId(Sha256::digest(&bytes).into());
    if content_id.0 != request.expected_content_hash {
        return Err(ProjectCorrelationError::ContentHashMismatch);
    }
    // Rebuild the final correlation from the bytes just read; the request hash
    // is checked as authority and the stored hash comes from source content.
    let reference = ProjectFileRef {
        content_hash: Some(content_id.0),
        ..reference
    };
    validate_project_file_reference(&reference)
        .map_err(|error| ProjectCorrelationError::InvalidReference(error.to_string()))?;
    Ok(PreparedProjectCorrelation {
        owner_id: target_owner_id,
        principal_id: request.authorized_principal_id,
        attachment: ProjectAttachmentInput {
            filename: request.filename,
            mime_type: request.mime_type,
            content_id,
            byte_length,
            reference,
        },
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCorrelatedNativeCommit {
    pub native: super::conversation_workflow::NativeConversationCommit,
    pub project_sources: Vec<PreparedProjectCorrelation>,
}

/// Validate owner binding and the native branch cut before queue finalization.
/// The caller must retain the same owner writer lock through commit.
pub fn validate_project_correlated_native_commit(
    writer: &RelWriter,
    authorized_owner_id: [u8; 16],
    command: &ProjectCorrelatedNativeCommit,
) -> Result<(), OwnerSessionError> {
    let (turn, branch) = into_archive_parts(command)?;
    if writer.owner_id() != authorized_owner_id
        || command.project_sources.iter().any(|source| {
            source.owner_id != authorized_owner_id
                || source.principal_id != command.native.principal_id
        })
    {
        return Err(OwnerSessionError::Record(
            "project correlation does not match the authorized owner and principal".into(),
        ));
    }
    writer.validate_turn_branch(&turn, &branch)
}

/// Publish the native user turn and its source correlations in the same
/// Archive turn write. Exact retries reuse the stable VCS reference and the
/// Archive's existing idempotent turn/branch contract.
pub fn commit_project_correlated_native_turn(
    writer: &mut RelWriter,
    authorized_owner_id: [u8; 16],
    command: ProjectCorrelatedNativeCommit,
) -> Result<super::conversation_workflow::PublishedNativeTurn, OwnerSessionError> {
    let (turn, branch) = into_archive_parts(&command)?;
    validate_project_correlated_native_commit(writer, authorized_owner_id, &command)?;
    let node = writer.accept_turn_and_advance_branch(turn, branch)?;
    Ok(super::conversation_workflow::PublishedNativeTurn {
        node_id: node.id,
        parent_node_id: node.parent_id,
        principal_id: node.principal_id,
        committed_timestamp_ns: node.timestamp_ns,
    })
}

/// Check an idempotent queue retry against the exact project references
/// already accepted on the native turn.
pub fn validate_project_correlation_retry(
    rel: &reliquary_owner_session::RelSession,
    authorized_owner_id: [u8; 16],
    authorized_principal_id: &str,
    conversation_id: &str,
    node_id: &str,
    sources: &[PreparedProjectCorrelation],
) -> Result<(), OwnerSessionError> {
    if rel.owner_id() != authorized_owner_id
        || sources.iter().any(|source| {
            source.owner_id != authorized_owner_id || source.principal_id != authorized_principal_id
        })
    {
        return Err(OwnerSessionError::Record(
            "project correlation retry authority does not match".into(),
        ));
    }
    let existing = rel
        .archive_source_attachments(conversation_id, node_id)
        .into_iter()
        .filter(|attachment| attachment.project_reference.is_some())
        .collect::<Vec<_>>();
    let expected = sources
        .iter()
        .map(|source| &source.attachment)
        .collect::<Vec<_>>();
    if existing.len() != expected.len()
        || expected.iter().any(|expected| {
            !existing.iter().any(|existing| {
                existing.filename == expected.filename
                    && existing.mime_type == expected.mime_type
                    && existing.content_id == expected.content_id
                    && existing.byte_length == expected.byte_length
                    && existing.project_reference.as_ref() == Some(&expected.reference)
            })
        })
    {
        return Err(OwnerSessionError::Record(
            "project correlation retry conflicts with accepted source references".into(),
        ));
    }
    Ok(())
}

fn into_archive_parts(
    command: &ProjectCorrelatedNativeCommit,
) -> Result<(TurnInput, Branch), OwnerSessionError> {
    if command.project_sources.is_empty() {
        return Err(OwnerSessionError::Record(
            "project-correlated native turn requires a verified source".into(),
        ));
    }
    let (mut turn, branch) = command.native.clone().into_archive_parts();
    turn.project_attachments = command
        .project_sources
        .iter()
        .map(|source| source.attachment.clone())
        .collect();
    Ok((turn, branch))
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_container::ReliquaryScope;
    use reliquary_owner_session::{
        GraphAdmission, RelSession,
        model::{ProjectRepositoryKind, ProjectRepositoryRef},
    };
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static NEXT: AtomicU64 = AtomicU64::new(1);

    struct Temp(PathBuf);

    impl Temp {
        fn new() -> Self {
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(".test-tmp")
                .join(format!(
                    "pc-{}-{}-{}",
                    std::process::id(),
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos(),
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

    fn request(
        repository_id: &str,
        revision: &str,
        hash: [u8; 32],
        path: &str,
    ) -> ProjectCorrelationRequest {
        ProjectCorrelationRequest {
            authorized_owner_id: [0x41; 16],
            authorized_principal_id: "principal".into(),
            repository: ProjectRepositoryRef {
                kind: ProjectRepositoryKind::Git,
                repository_id: repository_id.into(),
                project_path: "project".into(),
            },
            revision: revision.into(),
            path: path.into(),
            expected_content_hash: hash,
            filename: "source.txt".into(),
            mime_type: Some("text/plain".into()),
        }
    }

    fn native_commit() -> super::super::conversation_workflow::NativeConversationCommit {
        super::super::conversation_workflow::NativeConversationCommit {
            node_id: "source-turn".into(),
            conversation_id: "conversation".into(),
            parent_node_id: None,
            branch_id: "main".into(),
            principal_id: "principal".into(),
            committed_timestamp_ns: 17,
            content: "Please inspect this project file.".into(),
        }
    }

    #[test]
    fn accepted_turn_keeps_only_verified_reference_and_retries_after_reopen() {
        let temp = Temp::new();
        let repo = temp.0.join("repo");
        fs::create_dir_all(repo.join("project")).unwrap();
        git(&repo, &["init", "-q"]);
        git(&repo, &["config", "user.name", "Correlation test"]);
        git(&repo, &["config", "user.email", "test@example.invalid"]);
        git(
            &repo,
            &["config", "--local", "warlock.repositoryId", "repo-owner"],
        );
        let bytes = b"authoritative VCS bytes\n";
        fs::write(repo.join("project/source.txt"), bytes).unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-qm", "source"]);
        let revision = git(&repo, &["rev-parse", "HEAD"]);
        let hash: [u8; 32] = Sha256::digest(bytes).into();
        let prepared = prepare_project_correlation(
            [0x41; 16],
            request("repo-owner", &revision, hash, "project/source.txt"),
            &repo,
        )
        .unwrap();
        let command = ProjectCorrelatedNativeCommit {
            native: native_commit(),
            project_sources: vec![prepared],
        };

        let owner_path = temp.0.join("owner.rel");
        let mut writer =
            RelWriter::create_path(&owner_path, ReliquaryScope::Project, [0x41; 16]).unwrap();
        let first = commit_project_correlated_native_turn(&mut writer, [0x41; 16], command.clone())
            .unwrap();
        assert_eq!(
            commit_project_correlated_native_turn(&mut writer, [0x41; 16], command).unwrap(),
            first
        );
        drop(writer);

        let admission = GraphAdmission::for_replay([0x41; 16], [], [], [], [], []);
        let writer = RelWriter::open_path(&owner_path, admission, [], []).unwrap();
        let attachment = writer
            .read_session()
            .archive_source_attachments("conversation", "source-turn")
            .into_iter()
            .next()
            .unwrap();
        assert!(attachment.bytes.is_empty());
        assert_eq!(attachment.byte_length, bytes.len() as u64);
        let reference = attachment.project_reference.unwrap();
        assert_eq!(reference.path, "project/source.txt");
        assert_eq!(reference.revision.revision, revision);
        assert_eq!(reference.content_hash, Some(hash));
        assert_eq!(
            reliquary_project_environment::read_project_file_reference(&repo, &reference).unwrap(),
            bytes
        );
        assert!(
            !writer
                .read_session()
                .resolved_archive_turn("conversation", "source-turn")
                .unwrap()
                .unwrap()
                .content
                .contains("authoritative VCS bytes")
        );
    }

    #[test]
    fn wrong_owner_repository_hash_and_path_fail_before_publication() {
        let temp = Temp::new();
        let repo = temp.0.join("repo");
        fs::create_dir_all(repo.join("project")).unwrap();
        git(&repo, &["init", "-q"]);
        git(&repo, &["config", "user.name", "Correlation test"]);
        git(&repo, &["config", "user.email", "test@example.invalid"]);
        git(
            &repo,
            &["config", "--local", "warlock.repositoryId", "repo-owner"],
        );
        fs::write(repo.join("project/source.txt"), b"authoritative bytes").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-qm", "source"]);
        let revision = git(&repo, &["rev-parse", "HEAD"]);
        let hash: [u8; 32] = Sha256::digest(b"authoritative bytes").into();

        assert_eq!(
            prepare_project_correlation(
                [0x42; 16],
                request("repo-owner", &revision, hash, "project/source.txt"),
                &repo,
            ),
            Err(ProjectCorrelationError::OwnerMismatch)
        );
        assert!(
            prepare_project_correlation(
                [0x41; 16],
                request("different-repo", &revision, hash, "project/source.txt"),
                &repo,
            )
            .is_err()
        );
        assert!(
            prepare_project_correlation(
                [0x41; 16],
                request("repo-owner", &revision, [0x99; 32], "project/source.txt"),
                &repo,
            )
            .is_err()
        );
        assert!(matches!(
            prepare_project_correlation(
                [0x41; 16],
                request("repo-owner", &revision, hash, "other/source.txt"),
                &repo,
            ),
            Err(ProjectCorrelationError::InvalidReference(_))
        ));

        let owner_path = temp.0.join("owner.rel");
        let writer =
            RelWriter::create_path(&owner_path, ReliquaryScope::Project, [0x41; 16]).unwrap();
        let source = prepare_project_correlation(
            [0x41; 16],
            request("repo-owner", &revision, hash, "project/source.txt"),
            &repo,
        )
        .unwrap();
        let command = ProjectCorrelatedNativeCommit {
            native: native_commit(),
            project_sources: vec![source],
        };
        assert!(validate_project_correlated_native_commit(&writer, [0x42; 16], &command).is_err());
        let mut substituted_principal = command.clone();
        substituted_principal.native.principal_id = "other-principal".into();
        assert!(
            validate_project_correlated_native_commit(&writer, [0x41; 16], &substituted_principal)
                .is_err()
        );
        assert_eq!(writer.read_session().accepted_turn_sources().len(), 0);
    }

    #[test]
    fn lore_correlations_preserve_exact_historical_bytes() {
        use lore::interface::{LoreGlobalArgs, LoreSharedStoreMode, LoreString};
        use lore::repository::LoreRepositoryCreateArgs;
        let temp = Temp::new();
        let root = temp.0.join("lore-project");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("source.txt"), b"original lore source").unwrap();
        let status = lore::runtime().block_on(lore::repository::create(
            LoreGlobalArgs {
                repository_path: root.to_string_lossy().as_ref().into(),
                offline: 1,
                local: 1,
                ..Default::default()
            },
            LoreRepositoryCreateArgs {
                repository_url: "correlation-test".into(),
                id: LoreString::default(),
                description: "fixture".into(),
                use_shared_store: LoreSharedStoreMode::Disabled,
                shared_store_path: LoreString::default(),
            },
            None,
        ));
        assert_eq!(status, 0);
        let revision = reliquary_project_environment::checkpoint_lore_project(&root).unwrap();
        let hash: [u8; 32] = Sha256::digest(b"original lore source").into();
        let request = ProjectCorrelationRequest {
            authorized_owner_id: [0x41; 16],
            authorized_principal_id: "principal".into(),
            repository: revision.repository.clone(),
            revision: revision.revision.clone(),
            path: "source.txt".into(),
            expected_content_hash: hash,
            filename: "source.txt".into(),
            mime_type: None,
        };
        let source = prepare_project_correlation([0x41; 16], request, &root).unwrap();
        fs::write(root.join("source.txt"), b"later changed source").unwrap();
        reliquary_project_environment::checkpoint_lore_project(&root).unwrap();
        let path = temp.0.join("lore.rel");
        let mut writer =
            RelWriter::create_path(&path, ReliquaryScope::Project, [0x41; 16]).unwrap();
        commit_project_correlated_native_turn(
            &mut writer,
            [0x41; 16],
            ProjectCorrelatedNativeCommit {
                native: native_commit(),
                project_sources: vec![source],
            },
        )
        .unwrap();
        drop(writer);
        let rel = RelSession::open_path(
            &path,
            &GraphAdmission::for_replay([0x41; 16], [], [], [], [], []),
            [],
            [],
        )
        .unwrap();
        let attachment = rel
            .archive_source_attachments("conversation", "source-turn")
            .remove(0);
        assert!(attachment.bytes.is_empty());
        assert_eq!(
            reliquary_project_environment::read_lore_project_file(
                &root,
                &attachment.project_reference.unwrap()
            )
            .unwrap(),
            b"original lore source"
        );
    }
}
