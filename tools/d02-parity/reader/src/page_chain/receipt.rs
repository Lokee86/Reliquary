use crate::evidence_encoding::{hex, sha256};
use reliquary_owner_session::{CurrentOwnerKind, CurrentOwnerObservation, MemoryId};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(crate) const PAGE_CHAIN_SCHEMA_ID: &str = "reliquary-d02-memory-id-page-chain-v1";
const PAGE_CHAIN_SCHEMA_DEFINITION: &[u8] = b"reliquary-d02-memory-id-page-chain-v1\npage_index:u64;history_generation_sha256:32;memory_version:u64;family:memory_ids;order:ascending_memory_id;after_id_exclusive:optional-id;ids:ordered-id-list;last_id:optional-id;returned_count:u64;raw_payload_bytes:u64;previous_page_digest:optional-sha256;page_digest:sha256;terminal_empty:bool";
const PAGE_CHAIN_DOMAIN: &[u8] = b"reliquary:d02-memory-id-page-chain:v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PageSnapshot {
    pub kind: String,
    pub scope: String,
    pub owner_id: [u8; 16],
    pub file_sha256: [u8; 32],
    pub history_generation: [u8; 32],
    pub memory_version: u64,
}

impl PageSnapshot {
    pub(crate) fn from_observation(observation: &CurrentOwnerObservation) -> Self {
        Self {
            kind: match observation.kind {
                CurrentOwnerKind::Rel => "REL".into(),
                CurrentOwnerKind::Phy => "PHY".into(),
            },
            scope: format!("{:?}", observation.scope).to_ascii_lowercase(),
            owner_id: observation.owner_id,
            file_sha256: observation.file_sha256_before,
            history_generation: observation.history_generation,
            memory_version: observation.memory_version,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PageReceipt {
    pub snapshot: PageSnapshot,
    pub page_index: u64,
    pub after_id: Option<MemoryId>,
    pub ids: Vec<MemoryId>,
    pub raw_payload_bytes: usize,
    pub previous_page_digest: Option<[u8; 32]>,
    pub page_digest: [u8; 32],
    pub terminal_empty: bool,
}

impl PageReceipt {
    pub(crate) fn new(
        snapshot: &PageSnapshot,
        page_index: u64,
        after_id: Option<MemoryId>,
        previous_page_digest: Option<[u8; 32]>,
        ids: &[MemoryId],
    ) -> Self {
        let raw_payload_bytes = ids.len() * 32;
        let terminal_empty = ids.is_empty();
        let page_digest = digest_page(
            snapshot,
            page_index,
            after_id,
            previous_page_digest,
            ids,
            terminal_empty,
            raw_payload_bytes,
        );
        Self {
            snapshot: snapshot.clone(),
            page_index,
            after_id,
            ids: ids.to_vec(),
            raw_payload_bytes,
            previous_page_digest,
            page_digest,
            terminal_empty,
        }
    }

    pub(crate) fn to_json(&self) -> Value {
        json!({
            "schema_id": PAGE_CHAIN_SCHEMA_ID,
            "schema_sha256": hex(&page_chain_schema_sha256()),
            "page_index": self.page_index.to_string(),
            "history_generation_sha256": hex(&self.snapshot.history_generation),
            "memory_version": self.snapshot.memory_version.to_string(),
            "family": "memory_ids",
            "order": "ascending_memory_id",
            "after_id_exclusive": self.after_id.map(|id| hex(&id.0)),
            "last_id": self.ids.last().map(|id| hex(&id.0)),
            "returned_count": self.ids.len().to_string(),
            "raw_payload_bytes": self.raw_payload_bytes.to_string(),
            "previous_page_digest": self.previous_page_digest.map(|digest| hex(&digest)),
            "page_digest": hex(&self.page_digest),
            "terminal_empty": self.terminal_empty
        })
    }
}

pub(crate) fn page_chain_schema_sha256() -> [u8; 32] {
    sha256(PAGE_CHAIN_SCHEMA_DEFINITION)
}

fn digest_page(
    snapshot: &PageSnapshot,
    page_index: u64,
    after_id: Option<MemoryId>,
    previous_page_digest: Option<[u8; 32]>,
    ids: &[MemoryId],
    terminal_empty: bool,
    raw_payload_bytes: usize,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(PAGE_CHAIN_DOMAIN);
    update_field(&mut digest, &page_chain_schema_sha256());
    update_field(&mut digest, snapshot.kind.as_bytes());
    update_field(&mut digest, snapshot.scope.as_bytes());
    update_field(&mut digest, &snapshot.owner_id);
    update_field(&mut digest, &snapshot.file_sha256);
    update_field(&mut digest, &snapshot.history_generation);
    update_field(&mut digest, &snapshot.memory_version.to_be_bytes());
    update_field(&mut digest, &page_index.to_be_bytes());
    update_optional_field(&mut digest, after_id.as_ref().map(|id| id.0.as_slice()));
    update_optional_field(
        &mut digest,
        previous_page_digest.as_ref().map(|value| value.as_slice()),
    );
    update_field(&mut digest, b"memory_ids");
    update_field(&mut digest, b"ascending_memory_id");
    update_field(&mut digest, &(ids.len() as u64).to_be_bytes());
    update_field(&mut digest, &(raw_payload_bytes as u64).to_be_bytes());
    update_field(&mut digest, &[u8::from(terminal_empty)]);
    for id in ids {
        update_field(&mut digest, &id.0);
    }
    digest.finalize().into()
}

fn update_optional_field(digest: &mut Sha256, value: Option<&[u8]>) {
    match value {
        Some(value) => {
            update_field(digest, &[1]);
            update_field(digest, value);
        }
        None => update_field(digest, &[0]),
    }
}

fn update_field(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}
