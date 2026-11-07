//! Current retention of one authenticated imported Freshness settlement.
use crate::{FreshnessPolicy, HistoricalFreshnessSnapshot};
use sha2::{Digest, Sha256};

pub const RETAINED_FRESHNESS_AUTHORITY_MARKER: &[u8; 8] = b"RELHFA01";
const SOURCE_CHECKSUM_DOMAIN: &[u8] = b"reliquary-semantic-import-settlement-v7";
const MAX_PROOF_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedFreshnessAuthority {
    pub import_id: String,
    pub source_sha256: [u8; 32],
    pub global_version: u64,
    pub archive_version: u64,
    pub graph_version: u64,
    pub memory_graph_version: u64,
    source_proof: Vec<u8>,
}

impl RetainedFreshnessAuthority {
    pub fn source_proof(&self) -> &[u8] {
        &self.source_proof
    }
}

pub(crate) fn decode(
    bytes: &[u8],
    policy: &FreshnessPolicy,
) -> Result<Option<(HistoricalFreshnessSnapshot, RetainedFreshnessAuthority)>, String> {
    if !bytes.starts_with(RETAINED_FRESHNESS_AUTHORITY_MARKER) {
        return Ok(None);
    }
    let size = bytes
        .get(8..12)
        .ok_or("truncated retained Freshness authority")?;
    let snapshot_end = 12 + u32::from_le_bytes(size.try_into().unwrap()) as usize;
    let snapshot = bytes
        .get(12..snapshot_end)
        .ok_or("truncated retained Freshness snapshot")?;
    let snapshot = HistoricalFreshnessSnapshot::decode(snapshot, policy)?
        .ok_or("missing retained Freshness snapshot")?;
    let proof = bytes
        .get(snapshot_end..)
        .ok_or("missing Freshness source proof")?;
    if !(329..=MAX_PROOF_BYTES).contains(&proof.len())
        || &proof[..8] != b"CVAFRS07"
        || proof[8..24] != snapshot.owner_uuid
        || crate::store::hex(&proof[24..56]) != snapshot.import_id
    {
        return Err("retained Freshness authority has a different source identity".into());
    }
    let actual: [u8; 32] = Sha256::digest(proof).into();
    let checksum_at = proof.len() - 32;
    let mut hasher = Sha256::new();
    hasher.update(SOURCE_CHECKSUM_DOMAIN);
    hasher.update(&proof[..checksum_at]);
    if actual != snapshot.evidence_digest || hasher.finalize().as_slice() != &proof[checksum_at..] {
        return Err("retained Freshness source authority digest differs".into());
    }
    let number = |start| u64::from_le_bytes(proof[start..start + 8].try_into().unwrap());
    let (base, settlement) = (number(248), number(256));
    let (global, archive, graph, memory_graph) =
        (number(264), number(272), number(280), number(288));
    if base != snapshot.base_cut
        || settlement != snapshot.settlement_cut
        || archive > global
        || memory_graph > graph
        || graph > global
    {
        return Err("retained Freshness source authority coordinates differ".into());
    }
    let evidence = RetainedFreshnessAuthority {
        import_id: snapshot.import_id.clone(),
        source_sha256: actual,
        global_version: global,
        archive_version: archive,
        graph_version: graph,
        memory_graph_version: memory_graph,
        source_proof: proof.to_vec(),
    };
    Ok(Some((snapshot, evidence)))
}
