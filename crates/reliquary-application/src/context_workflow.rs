//! Owner- and source-bound composition around the pure Context Engine.

use reliquary_context_engine::{
    CompactionBudget, CompactionCheckpoint, CompactionPlan, ContextRequest, ContextRole,
    ContextTokenCounter, ContextTurn, ContextView, finalize_compaction, plan_session_compaction,
    request_context,
};
use reliquary_owner_session::model::{ProjectFileRef, validate_project_file_reference};
use sha2::{Digest, Sha256};
use std::fmt;

pub const MAX_CONTEXT_SOURCE_TURNS: usize = 64;
pub const MAX_CONTEXT_SOURCE_BYTES: usize = 128 << 10;
pub const MAX_CONTEXT_PROJECT_SOURCES: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextRepositoryEvidence {
    pub attachment_id: [u8; 32],
    pub reference: ProjectFileRef,
    pub byte_length: u64,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextMemoryScope {
    Rel,
    Phy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextMemoryEvidence {
    pub scope: ContextMemoryScope,
    pub owner_id: [u8; 16],
    pub memory_id: [u8; 32],
    pub revision: u64,
    pub body_id: [u8; 32],
    pub title: String,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContextWorkflowError {
    InvalidNativeContext,
    InvalidRepositoryEvidence,
    TooManyTurns,
    TooManyProjectSources,
    SourceByteLimit,
    Compaction(String),
}

impl fmt::Display for ContextWorkflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNativeContext => f.write_str("native context transcript is invalid"),
            Self::InvalidRepositoryEvidence => f.write_str("repository evidence is invalid"),
            Self::TooManyTurns => f.write_str("context source turn limit exceeded"),
            Self::TooManyProjectSources => f.write_str("context project source limit exceeded"),
            Self::SourceByteLimit => f.write_str("context source byte limit exceeded"),
            Self::Compaction(error) => write!(f, "context compaction failed: {error}"),
        }
    }
}
impl std::error::Error for ContextWorkflowError {}

/// Immutable prepared inputs. Native turns come only from the host's active
/// conversation transcript; repository evidence is separately source-bound.
pub struct PreparedContextWorkflow {
    view: ContextView,
    request: ContextRequest,
    plan: Option<CompactionPlan>,
    repository_sources: Vec<ContextRepositoryEvidence>,
    memory_sources: Vec<ContextMemoryEvidence>,
}
impl PreparedContextWorkflow {
    pub fn request(&self) -> &ContextRequest {
        &self.request
    }
    pub fn compaction_plan(&self) -> Option<&CompactionPlan> {
        self.plan.as_ref()
    }
    pub fn repository_sources(&self) -> &[ContextRepositoryEvidence] {
        &self.repository_sources
    }
    pub fn memory_sources(&self) -> &[ContextMemoryEvidence] {
        &self.memory_sources
    }
    pub fn source_view(&self) -> &ContextView {
        &self.view
    }
}

/// Build a bounded model request while preserving host transcript identity and
/// attaching only bytes whose exact repository identity can be checked.
pub fn prepare_context_workflow(
    mut native: ContextView,
    repository_sources: Vec<ContextRepositoryEvidence>,
    memory_sources: Vec<ContextMemoryEvidence>,
    checkpoint: Option<&CompactionCheckpoint>,
    current_tokens: u64,
    budget: CompactionBudget,
    counter: &impl ContextTokenCounter,
) -> Result<PreparedContextWorkflow, ContextWorkflowError> {
    if native.turns.len() > MAX_CONTEXT_SOURCE_TURNS {
        return Err(ContextWorkflowError::TooManyTurns);
    }
    if repository_sources.len() > MAX_CONTEXT_PROJECT_SOURCES {
        return Err(ContextWorkflowError::TooManyProjectSources);
    }
    if memory_sources.len() > MAX_CONTEXT_SOURCE_TURNS {
        return Err(ContextWorkflowError::TooManyTurns);
    }
    if native
        .turns
        .len()
        .checked_add(repository_sources.len())
        .and_then(|n| n.checked_add(memory_sources.len()))
        .is_none_or(|count| count > MAX_CONTEXT_SOURCE_TURNS)
    {
        return Err(ContextWorkflowError::TooManyTurns);
    }
    let mut bytes = 0usize;
    let mut ids = std::collections::HashSet::new();
    for turn in &native.turns {
        if turn.message_id.trim().is_empty()
            || turn.message_id.len() > 512
            || turn.message_id.chars().any(char::is_control)
            || !ids.insert(turn.message_id.clone())
        {
            return Err(ContextWorkflowError::InvalidNativeContext);
        }
        bytes = bytes
            .checked_add(turn.content.len())
            .ok_or(ContextWorkflowError::SourceByteLimit)?;
    }
    for source in &repository_sources {
        if validate_project_file_reference(&source.reference).is_err()
            || source.reference.content_hash != Some(Sha256::digest(&source.bytes).into())
            || source.byte_length != source.bytes.len() as u64
        {
            return Err(ContextWorkflowError::InvalidRepositoryEvidence);
        }
        let message_id = format!("project-evidence-{}", hex(&source.attachment_id));
        if !ids.insert(message_id.clone()) {
            return Err(ContextWorkflowError::InvalidRepositoryEvidence);
        }
        let body = String::from_utf8(source.bytes.clone())
            .map_err(|_| ContextWorkflowError::InvalidRepositoryEvidence)?;
        let reference = &source.reference;
        let labeled = format!(
            "[Authorized project evidence: {} {} {} @ {}]\n{}",
            reference.revision.repository.repository_id,
            reference.revision.repository.project_path,
            reference.path,
            reference.revision.revision,
            body
        );
        bytes = bytes
            .checked_add(labeled.len())
            .ok_or(ContextWorkflowError::SourceByteLimit)?;
        native.turns.push(ContextTurn {
            message_id,
            role: ContextRole::User,
            content: labeled,
        });
    }
    for source in &memory_sources {
        let message_id = format!(
            "memory-evidence-{}-{}-{}",
            match source.scope {
                ContextMemoryScope::Rel => "rel",
                ContextMemoryScope::Phy => "phy",
            },
            hex(&source.owner_id),
            hex(&source.memory_id)
        );
        if source.revision == 0
            || source.title.trim().is_empty()
            || source.content.trim().is_empty()
            || !ids.insert(message_id.clone())
        {
            return Err(ContextWorkflowError::InvalidNativeContext);
        }
        let content = format!(
            "[Selected {} Memory {} revision {}]\n{}\n{}",
            match source.scope {
                ContextMemoryScope::Rel => "REL",
                ContextMemoryScope::Phy => "PHY",
            },
            hex(&source.memory_id),
            source.revision,
            source.title,
            source.content
        );
        bytes = bytes
            .checked_add(content.len())
            .ok_or(ContextWorkflowError::SourceByteLimit)?;
        native.turns.push(ContextTurn {
            message_id,
            role: ContextRole::User,
            content,
        });
    }
    if bytes > MAX_CONTEXT_SOURCE_BYTES {
        return Err(ContextWorkflowError::SourceByteLimit);
    }
    let request = request_context(&native, checkpoint);
    let composed_tokens = request.messages.iter().fold(
        counter.count_text(&request.instructions),
        |total, message| total.saturating_add(counter.count_message(message)),
    );
    let effective_tokens = current_tokens.max(composed_tokens);
    let plan = plan_session_compaction(&native, checkpoint, effective_tokens, budget, counter)
        .map_err(|error| ContextWorkflowError::Compaction(error.to_string()))?;
    Ok(PreparedContextWorkflow {
        view: native,
        request,
        plan,
        repository_sources,
        memory_sources,
    })
}

/// Validate and normalize provider output against the exact private source cut.
pub fn finalize_context_workflow(
    prepared: &PreparedContextWorkflow,
    summary: String,
    counter: &impl ContextTokenCounter,
) -> Result<Option<CompactionCheckpoint>, ContextWorkflowError> {
    let Some(plan) = prepared.plan.as_ref() else {
        return Ok(None);
    };
    finalize_compaction(&prepared.view, plan, summary, counter)
        .map(Some)
        .map_err(ContextWorkflowError::Compaction)
}

/// Read a repository file at its exact retained Git/Lore revision.
pub fn read_context_repository_source(
    project_root: &std::path::Path,
    reference: &ProjectFileRef,
) -> Result<Vec<u8>, ContextWorkflowError> {
    validate_project_file_reference(reference)
        .map_err(|_| ContextWorkflowError::InvalidRepositoryEvidence)?;
    let bytes = match reference.revision.repository.kind {
        reliquary_owner_session::model::ProjectRepositoryKind::Git => {
            reliquary_project_environment::read_project_file_reference(project_root, reference)
        }
        reliquary_owner_session::model::ProjectRepositoryKind::Lore => {
            reliquary_project_environment::read_lore_project_file(project_root, reference)
        }
    }
    .map_err(|_| ContextWorkflowError::InvalidRepositoryEvidence)?;
    if reference.content_hash != Some(Sha256::digest(&bytes).into()) {
        return Err(ContextWorkflowError::InvalidRepositoryEvidence);
    }
    Ok(bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_context_engine::{ContextMessage, compaction_budget};
    use reliquary_owner_session::model::{
        ProjectRepositoryKind, ProjectRepositoryRef, ProjectRevisionRef,
    };

    struct Words;
    impl ContextTokenCounter for Words {
        fn count_text(&self, text: &str) -> u64 {
            text.split_whitespace().count() as u64
        }
        fn count_message(&self, message: &ContextMessage) -> u64 {
            self.count_text(&message.content).saturating_add(1)
        }
    }

    fn project_evidence() -> ContextRepositoryEvidence {
        let bytes = b"repository source text".to_vec();
        ContextRepositoryEvidence {
            attachment_id: [7; 32],
            reference: ProjectFileRef {
                revision: ProjectRevisionRef {
                    repository: ProjectRepositoryRef {
                        kind: ProjectRepositoryKind::Git,
                        repository_id: "repo".into(),
                        project_path: ".".into(),
                    },
                    revision: "a".repeat(40),
                },
                path: "docs/source.md".into(),
                content_hash: Some(Sha256::digest(&bytes).into()),
            },
            byte_length: bytes.len() as u64,
            bytes,
        }
    }

    #[test]
    fn context_composition_keeps_native_turn_once_and_binds_project_evidence() {
        let native = ContextView { turns: vec![
            ContextTurn { message_id: "host-1".into(), role: ContextRole::User, content: "first native turn with enough retained detail to make the protected raw tail measurable and bounded for this compaction test".into() },
            ContextTurn { message_id: "host-2".into(), role: ContextRole::Assistant, content: "second native turn with enough retained detail to make the protected raw tail measurable and bounded for this compaction test".into() },
        ]};
        let prepared = prepare_context_workflow(
            native,
            vec![project_evidence()],
            vec![],
            None,
            100,
            compaction_budget(200, 50).unwrap(),
            &Words,
        )
        .unwrap();
        assert_eq!(prepared.request().messages.len(), 3);
        assert!(
            prepared.request().messages[0]
                .content
                .starts_with("first native turn")
        );
        assert!(
            prepared.request().messages[1]
                .content
                .starts_with("second native turn")
        );
        assert!(
            prepared.request().messages[2]
                .content
                .contains("repository source text")
        );
        let plan = prepared.compaction_plan().unwrap();
        let checkpoint = finalize_context_workflow(
            &prepared,
            format!(
                "WORKING STATE\nsource: {}..{}",
                plan.through_message_id, plan.through_message_id
            ),
            &Words,
        )
        .unwrap()
        .unwrap();
        assert!(reliquary_context_engine::validate_summary_sources(
            &prepared.view,
            &checkpoint.through_message_id,
            &checkpoint.summary
        ));
    }

    #[test]
    fn context_rejects_forged_repository_bytes_and_unbounded_native_sources() {
        let one = ContextView {
            turns: vec![ContextTurn {
                message_id: "host".into(),
                role: ContextRole::User,
                content: "native".into(),
            }],
        };
        let mut forged = project_evidence();
        forged.bytes = b"changed source".to_vec();
        assert!(matches!(
            prepare_context_workflow(
                one,
                vec![forged],
                vec![],
                None,
                0,
                compaction_budget(1000, 50).unwrap(),
                &Words
            ),
            Err(ContextWorkflowError::InvalidRepositoryEvidence)
        ));
        let oversized = ContextView {
            turns: (0..=MAX_CONTEXT_SOURCE_TURNS)
                .map(|i| ContextTurn {
                    message_id: format!("m{i}"),
                    role: ContextRole::User,
                    content: "x".into(),
                })
                .collect(),
        };
        assert!(matches!(
            prepare_context_workflow(
                oversized,
                vec![],
                vec![],
                None,
                0,
                compaction_budget(1000, 50).unwrap(),
                &Words
            ),
            Err(ContextWorkflowError::TooManyTurns)
        ));
    }

    #[test]
    fn selected_rel_and_phy_memories_keep_separate_scope_provenance() {
        let native = ContextView {
            turns: vec![ContextTurn {
                message_id: "host".into(),
                role: ContextRole::User,
                content: "native transcript".into(),
            }],
        };
        let source = |scope, owner_id| ContextMemoryEvidence {
            scope,
            owner_id,
            memory_id: [9; 32],
            revision: 2,
            body_id: [8; 32],
            title: "selected title".into(),
            content: "selected body".into(),
        };
        let prepared = prepare_context_workflow(
            native,
            vec![],
            vec![
                source(ContextMemoryScope::Rel, [1; 16]),
                source(ContextMemoryScope::Phy, [2; 16]),
            ],
            None,
            0,
            compaction_budget(1000, 50).unwrap(),
            &Words,
        )
        .unwrap();
        assert_eq!(prepared.request().messages.len(), 3);
        assert!(
            prepared.request().messages[1]
                .content
                .contains("Selected REL Memory")
        );
        assert!(
            prepared.request().messages[2]
                .content
                .contains("Selected PHY Memory")
        );
        assert_eq!(prepared.memory_sources().len(), 2);
    }
}
