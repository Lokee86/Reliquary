//! Current retention of one authenticated imported Freshness settlement.
use crate::{FreshnessPolicy, HistoricalFreshnessSnapshot};
use sha2::{Digest, Sha256};

pub const RETAINED_FRESHNESS_AUTHORITY_MARKER: &[u8; 8] = b"RELHFA01";
const COORDINATE_BYTES: usize = 4 * size_of::<u64>();
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
    let (header, body) = bytes[RETAINED_FRESHNESS_AUTHORITY_MARKER.len()..]
        .split_at_checked(COORDINATE_BYTES + size_of::<u32>())
        .ok_or("truncated retained Freshness authority")?;
    let clock =
        |start| u64::from_le_bytes(header[start..start + size_of::<u64>()].try_into().unwrap());
    let (global, archive, graph, memory_graph) = (clock(0), clock(8), clock(16), clock(24));
    if archive > global || graph > global || memory_graph > graph {
        return Err("invalid retained Freshness authority coordinates".into());
    }
    let size = u32::from_le_bytes(header[COORDINATE_BYTES..].try_into().unwrap()) as usize;
    let (snapshot, proof) = body
        .split_at_checked(size)
        .ok_or("truncated retained Freshness snapshot")?;
    let snapshot = HistoricalFreshnessSnapshot::decode(snapshot, policy)?
        .ok_or("missing retained Freshness snapshot")?;
    if !(1..=MAX_PROOF_BYTES).contains(&proof.len()) {
        return Err("invalid retained Freshness evidence length".into());
    }
    let actual: [u8; 32] = Sha256::digest(proof).into();
    if actual != snapshot.evidence_digest {
        return Err("retained Freshness evidence digest differs".into());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_coordinates_bind_an_opaque_evidence_digest() {
        let policy = FreshnessPolicy::default_v1();
        let proof = b"opaque authority bytes";
        let snapshot = HistoricalFreshnessSnapshot {
            owner_uuid: [1; 16],
            import_id: "retained-import".into(),
            base_cut: 1,
            settlement_cut: 2,
            journal_entries_included: 3,
            evidence_digest: Sha256::digest(proof).into(),
            records: Default::default(),
        };
        let encoded = snapshot.encode(&policy).unwrap();
        let mut frame = RETAINED_FRESHNESS_AUTHORITY_MARKER.to_vec();
        for value in [11u64, 9, 8, 7] {
            frame.extend_from_slice(&value.to_le_bytes());
        }
        frame.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
        frame.extend_from_slice(&encoded);
        frame.extend_from_slice(proof);
        let (reopened, evidence) = decode(&frame, &policy).unwrap().unwrap();
        assert_eq!(reopened, snapshot);
        assert_eq!(evidence.source_proof(), proof);
        assert_eq!(
            (
                evidence.global_version,
                evidence.archive_version,
                evidence.graph_version,
                evidence.memory_graph_version
            ),
            (11, 9, 8, 7)
        );

        let mut wrong_clock = frame.clone();
        wrong_clock[32..40].copy_from_slice(&9u64.to_le_bytes());
        assert!(decode(&wrong_clock, &policy).is_err());
        *frame.last_mut().unwrap() ^= 1;
        assert!(decode(&frame, &policy).is_err());
    }
}
