//! Normalize one authorized copied view and bind terminal notices to its receipt.

use super::graph::{SourceConversation, SourceTurn};
use super::*;
use reliquary_owner_session::{RelSession, RelWriter};

#[path = "source_hermes_extension.rs"]
mod extension;
pub(super) use extension::attach_predecessor;

pub(super) use reliquary_owner_session::model::HERMES_COPIED_SOURCE_FORMAT as FORMAT;
const MAX_MESSAGES: usize = 128;
const MAX_MESSAGE_BYTES: usize = 32_768;
const MAX_COPY_BYTES: usize = 262_144;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HermesCopyScope {
    profile_id: String,
    session_id: String,
}

impl HermesCopyScope {
    pub fn new(profile_id: String, session_id: String) -> Result<Self, NativeImportSourceError> {
        source_id(&profile_id)?;
        source_id(&session_id)?;
        Ok(Self {
            profile_id,
            session_id,
        })
    }

    /// Stable native key. The receipt retains the separate original session ID.
    pub fn conversation_id(&self) -> String {
        let mut digest = Sha256::new();
        digest.update(b"reliquary/hermes-copied-conversation/v1\0");
        digest.update((self.profile_id.len() as u64).to_le_bytes());
        digest.update(self.profile_id.as_bytes());
        digest.update(self.session_id.as_bytes());
        format!("hermes-copy:{:x}", digest.finalize())
    }

    pub(super) fn namespace(&self, principal: &str) -> String {
        stage::principal_namespace(&format!("hermes/profile/{}", self.profile_id), principal)
    }

    fn accepted_conversation<'a>(
        &self,
        session: &'a RelSession,
        source_sha256: &[u8; 32],
        namespace: &str,
    ) -> Option<&'a ImportedConversation> {
        let receipt = session.native_import_receipt(source_sha256)?;
        if receipt.source_namespace != namespace || receipt.source_format != FORMAT {
            return None;
        }
        let id = self.conversation_id();
        receipt
            .conversations
            .iter()
            .find(|row| row.source_id == self.session_id && row.conversation_id == id)
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HermesCopiedRole {
    User,
    Assistant,
    System,
    Tool,
}

impl HermesCopiedRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::System => "system",
            Self::Tool => "tool",
        }
    }
}

/// One already privacy-filtered logical source message. Author metadata is provenance.
#[derive(Serialize)]
pub struct HermesCopiedMessage {
    pub message_uid: String,
    pub role: HermesCopiedRole,
    pub timestamp_ns: i64,
    pub content: String,
    pub author: Option<HermesCopiedAuthor>,
}

/// Ordered, text-only snapshot supplied by the trusted source adapter.
#[derive(Serialize)]
pub struct HermesCopiedSnapshot {
    pub scope: HermesCopyScope,
    pub branch_id: Option<String>,
    pub source_revision: Option<String>,
    pub messages: Vec<HermesCopiedMessage>,
}

impl NativeImportPlan {
    pub fn from_hermes_copy(
        snapshot: HermesCopiedSnapshot,
    ) -> Result<Self, NativeImportSourceError> {
        Self::hermes_copy(snapshot, None)
    }

    pub fn from_hermes_copy_extension(
        snapshot: HermesCopiedSnapshot,
        predecessor_sha256: [u8; 32],
    ) -> Result<Self, NativeImportSourceError> {
        Self::hermes_copy(snapshot, Some(predecessor_sha256))
    }

    fn hermes_copy(
        snapshot: HermesCopiedSnapshot,
        predecessor: Option<[u8; 32]>,
    ) -> Result<Self, NativeImportSourceError> {
        if snapshot.messages.is_empty() || snapshot.messages.len() > MAX_MESSAGES {
            return Err(invalid(
                "Hermes copy requires 1 through 128 source messages",
            ));
        }
        for id in snapshot.branch_id.iter().chain(&snapshot.source_revision) {
            source_id(id)?;
        }
        for message in &snapshot.messages {
            source_id(&message.message_uid)?;
            if let Some(actor) = message
                .author
                .as_ref()
                .and_then(|author| author.id.as_ref())
            {
                source_id(actor)?;
            }
            if message.timestamp_ns < 0 || message.content.len() > MAX_MESSAGE_BYTES {
                return Err(invalid(
                    "Hermes source message exceeds time or content bounds",
                ));
            }
        }
        let bytes = serde_json::to_vec(&snapshot).map_err(|_| invalid("invalid Hermes copy"))?;
        if bytes.len() > MAX_COPY_BYTES {
            return Err(NativeImportSourceError::Limit(
                "Hermes copy exceeds 256 KiB",
            ));
        }
        let digest = Sha256::digest(bytes);
        let revision = snapshot.source_revision.as_ref().map_or_else(
            || format!("authorized-copy-sha256:{digest:x}"),
            |revision| format!("hermes-snapshot-revision:{revision}"),
        );
        let conversation_id = snapshot.scope.conversation_id();
        let branch_id = copied_branch_id(snapshot.branch_id.as_deref());
        let mut parent = None;
        let turns = snapshot
            .messages
            .iter()
            .map(|message| SourceTurn {
                id: message.message_uid.clone(),
                source_id: message.message_uid.clone(),
                parent: parent.replace(message.message_uid.clone()),
                original_parent: None,
                role: message.role.as_str().into(),
                timestamp_ns: message.timestamp_ns,
                content: message.content.clone(),
                asset_references: 0,
                source_branch: None,
            })
            .collect::<Vec<_>>();
        let branch = Branch {
            id: branch_id,
            conversation_id: conversation_id.clone(),
            leaf_node_id: turns.last().expect("nonempty copy").id.clone(),
            canonical: true,
        };
        let mut plan = PlanBuilder::new(None);
        plan.register(&conversation_id, turns.len())?;
        plan.add_conversation_with_roles(
            SourceConversation {
                id: conversation_id,
                title: None,
                turns,
                branches: vec![branch],
                episodes: None,
            },
            |_| true,
        )?;
        let receipt = &mut plan.conversations[0];
        receipt.source_id = snapshot.scope.session_id.clone();
        if let Some(source) = snapshot.branch_id {
            receipt.branches[0].source_id = source;
        } else {
            receipt.branches.clear();
        }
        // The adjacency is for the local copy. The callback asserts no source parents.
        plan.parent_mappings.clear();
        plan.coverage.remapped_parents = 0;
        for (accepted, source) in receipt.messages.iter_mut().zip(snapshot.messages) {
            accepted.revision = revision.clone();
            accepted.source_parent_id = None;
            accepted.source_author = source.author;
        }
        let import_key = predecessor.map_or_else(
            || digest.into(),
            |previous| {
                let mut extension = Sha256::new();
                extension.update(b"reliquary/hermes-copied-extension/v1\0");
                extension.update(previous);
                extension.update(digest);
                extension.finalize().into()
            },
        );
        Ok(Self {
            source: NativeSource::HermesCopy {
                scope: snapshot.scope,
                predecessor,
            },
            source_sha256: import_key,
            import_key,
            coverage: plan.coverage,
            parent_mappings: plan.parent_mappings,
            conversations: plan.conversations,
            turns: plan.turns,
            episodes: plan.episodes,
        })
    }
}

/// Tombstone only the copied conversation bound by this principal's durable receipt.
pub fn tombstone_hermes_copy(
    writer: &mut RelWriter,
    scope: &HermesCopyScope,
    source_sha256: [u8; 32],
    principal: &str,
    credential_active: impl Fn() -> bool,
) -> Result<bool, NativeImportStageError> {
    let session = writer.read_session();
    let id = scope.conversation_id();
    if scope
        .accepted_conversation(session, &source_sha256, &scope.namespace(principal))
        .is_none()
    {
        return Err(NativeImportStageError::Conflict);
    }
    if !credential_active() {
        return Err(NativeImportStageError::Revoked);
    }
    if session.conversation_tombstone(&id).is_some() {
        return Ok(false);
    }
    if session.open_native_import_id().is_some() {
        return Err(NativeImportStageError::Conflict);
    }
    writer
        .tombstone_conversation(&id)
        .map_err(NativeImportStageError::Owner)
}

fn copied_branch_id(source: Option<&str>) -> String {
    source.map_or_else(
        || "hermes-copy:authorized-view".into(),
        |source| format!("hermes-copy:branch:{source}"),
    )
}

fn source_id(id: &str) -> Result<(), NativeImportSourceError> {
    if (1..=128).contains(&id.len())
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_.:-".contains(&c))
    {
        Ok(())
    } else {
        Err(invalid("invalid Hermes source identifier"))
    }
}
