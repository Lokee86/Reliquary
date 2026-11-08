//! Bounded, text-only source normalization for the native import workflow.

#[path = "native_import_workflow/source_ancestry.rs"]
mod ancestry;
#[path = "native_import_workflow/source_chatgpt.rs"]
mod chatgpt;
#[path = "native_import_workflow/source_graph.rs"]
mod graph;
#[path = "native_import_workflow/source_hermes.rs"]
mod hermes;
#[path = "native_import_workflow/source_normalized.rs"]
mod normalized;
#[path = "native_import_workflow/source_stage.rs"]
mod stage;

use crate::native_import_workflow::NativeImportEpisode;
use reliquary_owner_session::model::{Branch, ImportedConversation, TurnInput};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fmt,
    fs::File,
    io::{self, BufReader, Read, Take},
    path::Path,
};

pub use hermes::{
    HermesCopiedMessage, HermesCopiedRole, HermesCopiedSnapshot, HermesCopyScope,
    tombstone_hermes_copy,
};
pub use reliquary_owner_session::model::ImportedSourceAuthor as HermesCopiedAuthor;
pub use stage::{
    NativeImportStageError, NativeImportStageResult, NativeImportState, NativeImportStatus,
    native_import_status,
};

const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CONVERSATIONS: usize = 4_096;
const MAX_NODES: usize = 32_768;
const MAX_TURN_BYTES: usize = 256 * 1024;
const MAX_ACTION_BYTES: usize = 64 * 1024 * 1024;
const EPISODE_TURNS: usize = 24;
const EPISODE_BYTES: usize = 24_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeImportFormat {
    ChatGpt,
    Normalized,
}

impl NativeImportFormat {
    fn identity(self) -> (&'static str, &'static str) {
        match self {
            Self::ChatGpt => ("chatgpt-export", "chatgpt-mapping-text-v1"),
            Self::Normalized => (
                "chatgpt-export-2026-08-14/two-day-live",
                "chatgpt-visible-normalized-v2-48h",
            ),
        }
    }
}

/// Half-open UTC timestamp selection, in Unix nanoseconds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct NativeImportWindow {
    pub start_ns: i64,
    pub end_exclusive_ns: i64,
}

impl NativeImportWindow {
    pub fn from_rfc3339(start: &str, end_exclusive: &str) -> Result<Self, NativeImportSourceError> {
        let window = Self {
            start_ns: parse_instant(start)?,
            end_exclusive_ns: parse_instant(end_exclusive)?,
        };
        window.validate()?;
        Ok(window)
    }

    fn validate(self) -> Result<(), NativeImportSourceError> {
        if self.start_ns >= self.end_exclusive_ns {
            return Err(invalid(
                "import window must have a start before its exclusive end",
            ));
        }
        Ok(())
    }

    fn includes(self, timestamp: i64) -> bool {
        (self.start_ns..self.end_exclusive_ns).contains(&timestamp)
    }
}

fn parse_instant(value: &str) -> Result<i64, NativeImportSourceError> {
    use reliquary_chronos::{TemporalGranularity, analyze};
    let invalid_time = || invalid("import window endpoints must be RFC3339 timestamps in range");
    if !(20..=35).contains(&value.len()) {
        return Err(invalid_time());
    }
    analyze(value, None)
        .anchors
        .into_iter()
        .find(|anchor| {
            anchor.granularity == TemporalGranularity::Instant && anchor.evidence == value
        })
        .map(|anchor| anchor.start_ns)
        .ok_or_else(invalid_time)
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct NativeImportCoverage {
    pub window: Option<NativeImportWindow>,
    pub conversations: usize,
    pub turns: usize,
    pub branches: usize,
    pub episodes: usize,
    pub text_bytes: usize,
    pub text_only: bool,
    pub omitted_asset_references: usize,
    pub omitted_unsupported_messages: usize,
    pub omitted_hidden_messages: usize,
    pub omitted_missing_timestamps: usize,
    pub omitted_outside_window: usize,
    pub remapped_parents: usize,
}

/// Evidence of a source parent bypassed by visible-turn selection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NativeImportParentMapping {
    pub conversation_id: String,
    pub node_id: String,
    pub original_parent_id: Option<String>,
    pub retained_parent_id: Option<String>,
}

#[derive(Debug)]
pub enum NativeImportSourceError {
    Io(io::Error),
    Invalid(&'static str),
    InvalidJson { line: usize, column: usize },
    Limit(&'static str),
}

impl fmt::Display for NativeImportSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "cannot read import source: {error}"),
            Self::Invalid(reason) | Self::Limit(reason) => f.write_str(reason),
            Self::InvalidJson { line, column } => {
                write!(f, "invalid import JSON at line {line}, column {column}")
            }
        }
    }
}

impl std::error::Error for NativeImportSourceError {}

fn invalid(reason: &'static str) -> NativeImportSourceError {
    NativeImportSourceError::Invalid(reason)
}

enum NativeSource {
    File(NativeImportFormat),
    HermesCopy {
        scope: HermesCopyScope,
        predecessor: Option<[u8; 32]>,
    },
}

impl NativeSource {
    fn identity(&self, principal: &str) -> (String, &'static str) {
        match self {
            Self::File(format) => {
                let (namespace, format) = format.identity();
                (stage::principal_namespace(namespace, principal), format)
            }
            Self::HermesCopy { scope, .. } => (scope.namespace(principal), hermes::FORMAT),
        }
    }
}

/// Immutable source snapshot produced by a bounded source validator.
pub struct NativeImportPlan {
    source: NativeSource,
    source_sha256: [u8; 32],
    import_key: [u8; 32],
    coverage: NativeImportCoverage,
    parent_mappings: Vec<NativeImportParentMapping>,
    conversations: Vec<ImportedConversation>,
    turns: Vec<(TurnInput, Branch)>,
    episodes: Vec<NativeImportEpisode>,
}

impl NativeImportPlan {
    /// Decode a regular JSON file. A normalized manifest already carries its
    /// selection; an explicit window must match the manifest's declared window.
    pub fn from_path(
        path: impl AsRef<Path>,
        format: NativeImportFormat,
        window: Option<NativeImportWindow>,
    ) -> Result<Self, NativeImportSourceError> {
        if let Some(window) = window {
            window.validate()?;
        }
        let metadata = std::fs::metadata(path.as_ref()).map_err(NativeImportSourceError::Io)?;
        if !metadata.is_file() {
            return Err(invalid("import source must be a regular JSON file"));
        }
        if metadata.len() > MAX_SOURCE_BYTES {
            return Err(source_size_error());
        }
        let file = File::open(path).map_err(NativeImportSourceError::Io)?;
        let mut reader = BufReader::new(SourceReader {
            file: file.take(MAX_SOURCE_BYTES + 1),
            hash: Sha256::new(),
        });
        let mut plan = PlanBuilder::new(window);
        let parsed = match format {
            NativeImportFormat::ChatGpt => chatgpt::read(&mut reader, &mut plan),
            NativeImportFormat::Normalized => normalized::read(&mut reader, &mut plan),
        };
        let reader = reader.into_inner();
        if reader.file.limit() == 0 {
            return Err(source_size_error());
        }
        parsed?;
        if plan.turns.is_empty() {
            return Err(invalid("import selection contains no visible text turns"));
        }
        let source_sha256 = reader.hash.finalize().into();
        let import_key = match (format, window) {
            (NativeImportFormat::ChatGpt, Some(window)) => {
                let mut hash = Sha256::new();
                hash.update(b"reliquary/chatgpt-mapping-text-v1/window");
                hash.update(source_sha256);
                hash.update(window.start_ns.to_le_bytes());
                hash.update(window.end_exclusive_ns.to_le_bytes());
                hash.finalize().into()
            }
            _ => source_sha256,
        };
        Ok(Self {
            source: NativeSource::File(format),
            source_sha256,
            import_key,
            coverage: plan.coverage,
            parent_mappings: plan.parent_mappings,
            conversations: plan.conversations,
            turns: plan.turns,
            episodes: plan.episodes,
        })
    }

    pub fn coverage(&self) -> &NativeImportCoverage {
        &self.coverage
    }

    pub fn parent_mappings(&self) -> &[NativeImportParentMapping] {
        &self.parent_mappings
    }

    pub fn source_sha256(&self) -> [u8; 32] {
        self.source_sha256
    }
}

fn source_size_error() -> NativeImportSourceError {
    NativeImportSourceError::Limit("import source exceeds the 64 MiB JSON limit")
}

struct SourceReader {
    file: Take<File>,
    hash: Sha256,
}

impl Read for SourceReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.file.read(buffer)?;
        self.hash.update(&buffer[..count]);
        Ok(count)
    }
}

fn json_error(error: serde_json::Error) -> NativeImportSourceError {
    match error.io_error_kind() {
        Some(kind) => NativeImportSourceError::Io(io::Error::from(kind)),
        None => NativeImportSourceError::InvalidJson {
            line: error.line(),
            column: error.column(),
        },
    }
}

struct PlanBuilder {
    coverage: NativeImportCoverage,
    seen_conversations: BTreeSet<String>,
    source_nodes: usize,
    action_bytes: usize,
    parent_mappings: Vec<NativeImportParentMapping>,
    conversations: Vec<ImportedConversation>,
    turns: Vec<(TurnInput, Branch)>,
    episodes: Vec<NativeImportEpisode>,
}

impl PlanBuilder {
    fn new(window: Option<NativeImportWindow>) -> Self {
        Self {
            coverage: NativeImportCoverage {
                window,
                text_only: true,
                ..Default::default()
            },
            seen_conversations: BTreeSet::new(),
            source_nodes: 0,
            action_bytes: 0,
            parent_mappings: Vec::new(),
            conversations: Vec::new(),
            turns: Vec::new(),
            episodes: Vec::new(),
        }
    }

    fn register(&mut self, id: &str, nodes: usize) -> Result<(), NativeImportSourceError> {
        validate_id(id)?;
        if !self.seen_conversations.insert(id.to_owned()) {
            return Err(invalid("duplicate source conversation ID"));
        }
        self.source_nodes += nodes;
        if self.seen_conversations.len() > MAX_CONVERSATIONS || self.source_nodes > MAX_NODES {
            return Err(NativeImportSourceError::Limit(
                "import source exceeds the conversation or node limit",
            ));
        }
        Ok(())
    }

    fn reserve_action(
        &mut self,
        turn: &TurnInput,
        branch: &Branch,
    ) -> Result<(), NativeImportSourceError> {
        self.action_bytes += turn.content.len()
            + turn.id.len()
            + turn.conversation_id.len()
            + turn.parent_id.as_ref().map_or(0, String::len)
            + branch.id.len()
            + branch.conversation_id.len()
            + branch.leaf_node_id.len();
        if self.action_bytes > MAX_ACTION_BYTES {
            return Err(NativeImportSourceError::Limit(
                "expanded branch actions exceed the import byte budget",
            ));
        }
        Ok(())
    }
}

fn validate_id(id: &str) -> Result<(), NativeImportSourceError> {
    if id.trim().is_empty() || id.len() > 4096 || id.chars().any(char::is_control) {
        return Err(invalid("invalid source identifier"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "native_import_workflow/source_tests.rs"]
mod tests;
