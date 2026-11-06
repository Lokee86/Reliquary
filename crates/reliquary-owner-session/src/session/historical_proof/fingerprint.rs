use super::*;
use sha2::{Digest, Sha256};

struct Fingerprint(Sha256);

impl Fingerprint {
    fn new(kind: &str) -> Self {
        let mut digest = Sha256::new();
        digest.update(b"reliquary-owner-history-proof-v1\0");
        digest.update(kind.as_bytes());
        Self(digest)
    }

    fn debug(&mut self, label: &str, value: &impl fmt::Debug) {
        let value = format!("{value:?}");
        self.bytes(label, value.as_bytes());
    }

    fn bytes(&mut self, label: &str, value: &[u8]) {
        self.0.update((label.len() as u64).to_le_bytes());
        self.0.update(label.as_bytes());
        self.0.update((value.len() as u64).to_le_bytes());
        self.0.update(value);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

fn common(fingerprint: &mut Fingerprint, history: &CommonHistory) {
    fingerprint.debug("memories", &history.memories);
    fingerprint.debug("perception", &history.perception);
    fingerprint.debug("graph", &history.graph);
    fingerprint.debug("community", &history.community);
    fingerprint.debug("ego", &history.ego);
    fingerprint.debug("generations", &history.generations);
}

pub(super) fn rel(
    owner_id: [u8; 16],
    history: &CommonHistory,
    archive: &reliquary_archive::ArchiveHistorySnapshot,
    freshness: &BTreeMap<FreshnessMemoryId, FreshnessRecord>,
    cut: &RelHistoricalCut,
) -> [u8; 32] {
    let mut fingerprint = Fingerprint::new("REL");
    fingerprint.bytes("owner", &owner_id);
    fingerprint.debug("cut", cut);
    common(&mut fingerprint, history);
    fingerprint.debug("archive", archive);
    fingerprint.debug("freshness", freshness);
    fingerprint.finish()
}

pub(super) fn phy(
    owner_id: [u8; 16],
    history: &CommonHistory,
    cut: HistoricalOwnerCut,
) -> [u8; 32] {
    let mut fingerprint = Fingerprint::new("PHY");
    fingerprint.bytes("owner", &owner_id);
    fingerprint.debug("cut", &cut);
    common(&mut fingerprint, history);
    fingerprint.finish()
}
