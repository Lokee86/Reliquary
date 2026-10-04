//! Fixed-size negative membership hint for validated owner-local receipt identities.
//! Positive hints always fall through to exact receipts; saturation only loses speed.
//! This is rebuildable acceleration state, never identity or journal authority.
use sha2::{Digest, Sha256};

const WORDS: usize = 1024;

#[derive(Clone, Debug)]
pub(super) struct ReceiptMembership {
    bits: [u64; WORDS],
    complete: bool,
}

impl Default for ReceiptMembership {
    fn default() -> Self {
        Self {
            bits: [0; WORDS],
            complete: false,
        }
    }
}

impl ReceiptMembership {
    pub(super) fn empty_complete() -> Self {
        Self {
            complete: true,
            ..Self::default()
        }
    }
    pub(super) fn invalidate(&mut self) {
        self.complete = false;
    }
    pub(super) fn finish_rebuild(&mut self) {
        self.complete = true;
    }
    fn positions(kind: u8, id: &str) -> [usize; 4] {
        let mut hash = Sha256::new();
        hash.update([kind]);
        hash.update(id.as_bytes());
        let bytes = hash.finalize();
        std::array::from_fn(|n| {
            (u64::from_le_bytes(bytes[n * 8..n * 8 + 8].try_into().unwrap()) % (WORDS as u64 * 64))
                as usize
        })
    }
    pub(super) fn insert(&mut self, kind: u8, id: &str) {
        for position in Self::positions(kind, id) {
            self.bits[position / 64] |= 1 << (position % 64);
        }
    }
    pub(super) fn may_contain(&self, kind: u8, id: &str) -> bool {
        !self.complete
            || Self::positions(kind, id)
                .iter()
                .all(|position| self.bits[position / 64] & (1 << (position % 64)) != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incomplete_and_saturated_hints_never_suppress_exact_lookup() {
        let mut hint = ReceiptMembership::default();
        assert!(hint.may_contain(0, "unknown"));
        hint.finish_rebuild();
        assert!(!hint.may_contain(0, "unknown"));
        for n in 0..10000 {
            hint.insert(0, &format!("use-{n}"));
            hint.insert(1, &format!("event-{n}"));
        }
        for n in 0..10000 {
            assert!(hint.may_contain(0, &format!("use-{n}")));
            assert!(hint.may_contain(1, &format!("event-{n}")));
        }
        hint.bits.fill(u64::MAX);
        assert!(hint.may_contain(0, "any-false-positive"));
        hint.invalidate();
        assert!(hint.may_contain(1, "any-unknown-prefix"));
        assert_eq!(std::mem::size_of_val(&hint.bits), 8192);
    }
}
