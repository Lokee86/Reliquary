//! Explicit Freshness state carried into a rewritten history.
//!
//! A baseline is semantic owner state, not a synthetic access or Dream event.

use crate::{
    HistoryGenerationId, MemoryId,
    freshness::{FreshnessPolicy, FreshnessRecord},
};
use sha2::{Digest, Sha256};

pub(crate) const BASELINE_MAGIC: &[u8; 8] = b"CVAFRB01";
const CHECKSUM_LEN: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FreshnessRewriteBaseline {
    pub(crate) owner_uuid: [u8; 16],
    pub(crate) history_generation: HistoryGenerationId,
    pub(crate) memory_id: MemoryId,
    pub(crate) source_cut: u64,
    pub(crate) source_initial_decay_carry: u64,
    pub(crate) destination_cut: u64,
    /// Original materialized source record, retained as provenance evidence.
    pub(crate) source_record: FreshnessRecord,
    /// Source record settled at `source_cut`; this is the effective state preserved.
    pub(crate) effective_record: FreshnessRecord,
    /// Rebased score and clock anchors in the destination activity domain.
    pub(crate) destination_record: FreshnessRecord,
    /// Unconsumed fraction of the decay interval at the source cut.
    pub(crate) decay_remainder: u64,
    pub(crate) policy_digest: [u8; 32],
}

impl FreshnessRewriteBaseline {
    pub(crate) fn preserve_effective(
        owner_uuid: [u8; 16],
        history_generation: HistoryGenerationId,
        memory_id: MemoryId,
        source_cut: u64,
        destination_cut: u64,
        source_record: FreshnessRecord,
        policy: &FreshnessPolicy,
        policy_digest: [u8; 32],
    ) -> Result<Self, String> {
        Self::preserve_effective_with_carry(
            owner_uuid,
            history_generation,
            memory_id,
            source_cut,
            destination_cut,
            source_record,
            policy,
            policy_digest,
            0,
        )
    }

    pub(crate) fn preserve_effective_with_carry(
        owner_uuid: [u8; 16],
        history_generation: HistoryGenerationId,
        memory_id: MemoryId,
        source_cut: u64,
        destination_cut: u64,
        source_record: FreshnessRecord,
        policy: &FreshnessPolicy,
        policy_digest: [u8; 32],
        source_initial_decay_carry: u64,
    ) -> Result<Self, String> {
        policy.validate().map_err(|error| error.to_string())?;
        if owner_uuid == [0; 16] || history_generation == HistoryGenerationId::ZERO {
            return Err("Freshness rewrite baseline owner and generation must be nonzero".into());
        }
        if policy_digest == [0; 32] {
            return Err("Freshness rewrite baseline policy digest must be nonzero".into());
        }
        let mut effective_record = source_record;
        crate::freshness::settle_with_initial_decay_carry(
            &mut effective_record,
            source_cut,
            source_initial_decay_carry,
            policy,
        )
        .map_err(|error| error.to_string())?;
        let decay_remainder = remainder(&effective_record, source_cut, source_initial_decay_carry)?;
        let destination_anchor = destination_cut.saturating_sub(decay_remainder);
        let destination_record = FreshnessRecord {
            score: effective_record.score,
            admitted_at_turn: effective_record
                .admitted_at_turn
                .map(|_| destination_anchor),
            accounted_turn: effective_record.accounted_turn.map(|_| destination_anchor),
        };
        let result = Self {
            owner_uuid,
            history_generation,
            memory_id,
            source_cut,
            source_initial_decay_carry,
            destination_cut,
            source_record,
            effective_record,
            destination_record,
            decay_remainder,
            policy_digest,
        };
        result.validate(policy)?;
        Ok(result)
    }

    pub(crate) fn validate(&self, policy: &FreshnessPolicy) -> Result<(), String> {
        policy.validate().map_err(|error| error.to_string())?;
        if self.owner_uuid == [0; 16] || self.history_generation == HistoryGenerationId::ZERO {
            return Err("Freshness rewrite baseline owner and generation must be nonzero".into());
        }
        if self.policy_digest == [0; 32] {
            return Err("Freshness rewrite baseline policy digest must be nonzero".into());
        }
        let mut expected_effective = self.source_record;
        if self.source_initial_decay_carry >= policy.decay_turns_per_point {
            return Err("invalid inherited Freshness decay carry".into());
        }
        crate::freshness::settle_with_initial_decay_carry(
            &mut expected_effective,
            self.source_cut,
            self.source_initial_decay_carry,
            policy,
        )
        .map_err(|error| error.to_string())?;
        if expected_effective != self.effective_record {
            return Err("Freshness rewrite baseline effective source state is inconsistent".into());
        }
        let expected_remainder = remainder(
            &expected_effective,
            self.source_cut,
            self.source_initial_decay_carry,
        )?;
        if expected_remainder != self.decay_remainder {
            return Err("Freshness rewrite baseline decay evidence is inconsistent".into());
        }
        if self.decay_remainder >= policy.decay_turns_per_point {
            return Err("Freshness rewrite baseline decay remainder is invalid".into());
        }
        if self.effective_record.score != self.destination_record.score {
            return Err("Freshness rewrite baseline changed the effective score".into());
        }
        if self.effective_record.admitted_at_turn.is_some()
            != self.destination_record.admitted_at_turn.is_some()
            || self.effective_record.accounted_turn.is_some()
                != self.destination_record.accounted_turn.is_some()
        {
            return Err("Freshness rewrite baseline admission state is inconsistent".into());
        }
        if let Some(admitted) = self.destination_record.admitted_at_turn {
            if admitted > self.destination_cut
                || self.destination_record.accounted_turn.unwrap_or(u64::MAX) < admitted
            {
                return Err("Freshness rewrite baseline destination anchors are invalid".into());
            }
            let expected_anchor = self.destination_cut.saturating_sub(self.decay_remainder);
            if self.destination_record.accounted_turn != Some(expected_anchor)
                || self.destination_record.admitted_at_turn != Some(expected_anchor)
            {
                return Err(
                    "Freshness rewrite baseline does not preserve destination decay remainder"
                        .into(),
                );
            }
        } else if self.decay_remainder != 0 {
            return Err("unadmitted Freshness baseline has a decay remainder".into());
        }
        // Validate all source-side evidence without requiring it to be in the
        // destination clock domain.
        self.source_record
            .score_at_with_policy(
                self.source_record.accounted_turn.unwrap_or(self.source_cut),
                policy,
            )
            .map_err(|error| error.to_string())?;
        self.effective_record
            .score_at_with_policy(
                self.effective_record
                    .accounted_turn
                    .unwrap_or(self.source_cut),
                policy,
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>, String> {
        let mut out = BASELINE_MAGIC.to_vec();
        out.extend_from_slice(&self.owner_uuid);
        out.extend_from_slice(self.history_generation.as_bytes());
        out.extend_from_slice(&self.memory_id.0);
        out.extend_from_slice(&self.source_cut.to_le_bytes());
        out.extend_from_slice(&self.source_initial_decay_carry.to_le_bytes());
        out.extend_from_slice(&self.destination_cut.to_le_bytes());
        put_record(&mut out, self.source_record);
        put_record(&mut out, self.effective_record);
        put_record(&mut out, self.destination_record);
        out.extend_from_slice(&self.decay_remainder.to_le_bytes());
        out.extend_from_slice(&self.policy_digest);
        let checksum = Sha256::digest(&out);
        out.extend_from_slice(&checksum);
        Ok(out)
    }

    pub(crate) fn decode(payload: &[u8], policy: &FreshnessPolicy) -> Result<Self, String> {
        if payload.len() < 8 + 16 + 16 + 32 + 8 + 8 + 8 + 4 * 3 + 8 + 32 + CHECKSUM_LEN {
            return Err("truncated Freshness rewrite baseline".into());
        }
        if payload.get(..8) != Some(BASELINE_MAGIC.as_slice()) {
            return Err("invalid Freshness rewrite baseline magic".into());
        }
        let content_len = payload.len() - CHECKSUM_LEN;
        if Sha256::digest(&payload[..content_len]).as_slice() != &payload[content_len..] {
            return Err("Freshness rewrite baseline checksum mismatch".into());
        }
        let mut reader = Reader {
            bytes: &payload[8..content_len],
            offset: 0,
        };
        let owner_uuid = reader.array::<16>()?;
        let history_generation = HistoryGenerationId::from_bytes(reader.array::<16>()?);
        let memory_id = MemoryId(reader.array::<32>()?);
        let source_cut = reader.u64()?;
        let source_initial_decay_carry = reader.u64()?;
        let destination_cut = reader.u64()?;
        let source_record = reader.record()?;
        let effective_record = reader.record()?;
        let destination_record = reader.record()?;
        let decay_remainder = reader.u64()?;
        let policy_digest = reader.array::<32>()?;
        if reader.offset != reader.bytes.len() {
            return Err("trailing bytes in Freshness rewrite baseline".into());
        }
        let value = Self {
            owner_uuid,
            history_generation,
            memory_id,
            source_cut,
            source_initial_decay_carry,
            destination_cut,
            source_record,
            effective_record,
            destination_record,
            decay_remainder,
            policy_digest,
        };
        value.validate(policy)?;
        Ok(value)
    }
}

fn remainder(record: &FreshnessRecord, cut: u64, carry: u64) -> Result<u64, String> {
    match record.accounted_turn {
        Some(0) => cut
            .checked_add(carry)
            .ok_or_else(|| "Freshness carry overflow".into()),
        Some(accounted) => cut
            .checked_sub(accounted)
            .ok_or_else(|| "Freshness accounting exceeds source cut".into()),
        None => Ok(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> FreshnessRewriteBaseline {
        let policy = FreshnessPolicy::default_v1();
        let mut source = FreshnessRecord::created();
        source.admit(5).unwrap();
        source.score = 73;
        FreshnessRewriteBaseline::preserve_effective(
            [1; 16],
            HistoryGenerationId([2; 16]),
            MemoryId([3; 32]),
            28,
            4,
            source,
            &policy,
            [4; 32],
        )
        .unwrap()
    }

    #[test]
    fn zero_destination_cut_preserves_partial_decay_and_rewrite_chaining() {
        let policy = FreshnessPolicy::default_v1();
        let source = fixture();
        let b = FreshnessRewriteBaseline::preserve_effective(
            source.owner_uuid,
            source.history_generation,
            source.memory_id,
            source.source_cut,
            0,
            source.source_record,
            &policy,
            source.policy_digest,
        )
        .unwrap();
        assert_eq!(b.destination_record.accounted_turn, Some(0));
        assert_eq!(b.decay_remainder, 3);
        assert_eq!(
            FreshnessRewriteBaseline::decode(&b.encode().unwrap(), &policy).unwrap(),
            b
        );
        let mut r = b.destination_record;
        crate::freshness::settle_with_initial_decay_carry(&mut r, 6, 3, &policy).unwrap();
        assert_eq!(r.score, 71);
        crate::freshness::settle_with_initial_decay_carry(&mut r, 7, 3, &policy).unwrap();
        assert_eq!(r.score, 70);
        assert_eq!(r.accounted_turn, Some(7));
        crate::freshness::settle_with_initial_decay_carry(&mut r, 17, 3, &policy).unwrap();
        assert_eq!(r.score, 69);
        let again = FreshnessRewriteBaseline::preserve_effective_with_carry(
            b.owner_uuid,
            b.history_generation,
            b.memory_id,
            0,
            0,
            b.destination_record,
            &policy,
            b.policy_digest,
            3,
        )
        .unwrap();
        assert_eq!(again.decay_remainder, 3);
        assert_eq!(again.effective_record.score, 71);
    }

    #[test]
    fn round_trips_effective_state_without_rebasing_from_source_cut() {
        let baseline = fixture();
        assert_eq!(baseline.effective_record.score, 71);
        assert_eq!(baseline.decay_remainder, 3);
        assert_eq!(baseline.destination_cut, 4);
        assert_eq!(baseline.destination_record.admitted_at_turn, Some(1));
        assert_eq!(baseline.destination_record.accounted_turn, Some(1));
        assert_eq!(baseline.destination_record.score_at(10).unwrap(), 71);
        assert_eq!(baseline.destination_record.score_at(11).unwrap(), 70);
        let decoded = FreshnessRewriteBaseline::decode(
            &baseline.encode().unwrap(),
            &FreshnessPolicy::default_v1(),
        )
        .unwrap();
        assert_eq!(decoded, baseline);
    }

    #[test]
    fn rejects_truncation_corruption_and_trailing_bytes() {
        let baseline = fixture();
        let bytes = baseline.encode().unwrap();
        let policy = FreshnessPolicy::default_v1();
        assert!(FreshnessRewriteBaseline::decode(&bytes[..bytes.len() - 1], &policy).is_err());
        let mut corrupt = bytes.clone();
        corrupt[20] ^= 1;
        assert!(
            FreshnessRewriteBaseline::decode(&corrupt, &policy)
                .unwrap_err()
                .contains("checksum")
        );
        let mut trailing = bytes;
        trailing.splice(trailing.len() - 32..trailing.len() - 32, [9]);
        let checksum = Sha256::digest(&trailing[..trailing.len() - 32]);
        let n = trailing.len();
        trailing[n - 32..].copy_from_slice(&checksum);
        assert!(
            FreshnessRewriteBaseline::decode(&trailing, &policy)
                .unwrap_err()
                .contains("trailing")
        );
    }

    #[test]
    fn preserves_unadmitted_state_without_inventing_a_clock() {
        let baseline = FreshnessRewriteBaseline::preserve_effective(
            [1; 16],
            HistoryGenerationId([2; 16]),
            MemoryId([3; 32]),
            9,
            0,
            FreshnessRecord::created(),
            &FreshnessPolicy::default_v1(),
            [4; 32],
        )
        .unwrap();
        assert_eq!(baseline.destination_record, FreshnessRecord::created());
    }

    #[test]
    fn store_ingest_is_idempotent_and_conflicting_baselines_fail() {
        let baseline = fixture();
        let mut store = super::super::FreshnessStore::new(Some(baseline.owner_uuid));
        store
            .ingest(&super::super::encode_policy(
                baseline.owner_uuid,
                FreshnessPolicy::default_v1(),
            ))
            .unwrap();
        let bytes = baseline.encode().unwrap();
        store.ingest_rewrite_baseline(&bytes).unwrap();
        store.ingest_rewrite_baseline(&bytes).unwrap();
        assert_eq!(store.rewrite_baselines.len(), 1);
        assert_eq!(
            store.births[&baseline.memory_id].0,
            baseline.destination_cut
        );
        assert!(
            !store
                .recent
                .iter()
                .any(|(entry, _)| matches!(entry, super::super::JournalEntry::Initialize { .. }))
        );

        let mut conflict = baseline;
        conflict.policy_digest = [8; 32];
        assert!(
            store
                .ingest_rewrite_baseline(&conflict.encode().unwrap())
                .unwrap_err()
                .contains("conflicting")
        );
    }
}

fn put_record(out: &mut Vec<u8>, record: FreshnessRecord) {
    out.extend_from_slice(&record.score.to_le_bytes());
    for anchor in [record.admitted_at_turn, record.accounted_turn] {
        match anchor {
            Some(value) => {
                out.push(1);
                out.extend_from_slice(&value.to_le_bytes());
            }
            None => out.push(0),
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or("baseline offset overflow")?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or("truncated Freshness rewrite baseline")?;
        self.offset = end;
        Ok(bytes)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        self.take(N)?
            .try_into()
            .map_err(|_| "truncated Freshness rewrite baseline".into())
    }
    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn record(&mut self) -> Result<FreshnessRecord, String> {
        let score = i16::from_le_bytes(self.array()?);
        let admitted_at_turn = self.anchor()?;
        let accounted_turn = self.anchor()?;
        Ok(FreshnessRecord {
            score,
            admitted_at_turn,
            accounted_turn,
        })
    }
    fn anchor(&mut self) -> Result<Option<u64>, String> {
        match self.take(1)?[0] {
            0 => Ok(None),
            1 => Ok(Some(self.u64()?)),
            _ => Err("invalid Freshness rewrite baseline anchor tag".into()),
        }
    }
}
