use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportWatermarks {
    pub global_version: u64,
    pub archive_version: Option<u64>,
    pub memory_version: u64,
    pub entity_version: Option<u64>,
    pub relationship_version: Option<u64>,
    pub graph_version: u64,
    pub memory_graph_version: Option<u64>,
    pub vector_version: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportStart {
    /// 1 is a retained legacy marker whose recorded cuts are readable for
    /// fencing only; 2 uses the current explicit optional-clock contract.
    #[serde(
        default = "legacy_format_version",
        skip_serializing_if = "is_legacy_format_version"
    )]
    pub format_version: u32,
    pub owner_uuid: [u8; 16],
    pub import_id: String,
    pub import_key: String,
    pub source_namespace: String,
    pub source_format: String,
    pub started_at_ns: i64,
    pub pre: ImportWatermarks,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportComplete {
    #[serde(
        default = "legacy_format_version",
        skip_serializing_if = "is_legacy_format_version"
    )]
    pub format_version: u32,
    pub owner_uuid: [u8; 16],
    pub import_id: String,
    pub start_digest: String,
    pub receipt_digest: String,
    pub completed_at_ns: i64,
    pub post: ImportWatermarks,
    pub historical_freshness_settlement_digest: String,
}

fn legacy_format_version() -> u32 {
    1
}

fn is_legacy_format_version(version: &u32) -> bool {
    *version == 1
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportEpochStatus {
    Open(ImportStart),
    Completed {
        start: ImportStart,
        complete: ImportComplete,
    },
}
