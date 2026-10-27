//! Durable Dream storage primitives.

pub mod dream_begin_codec;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct MemoryId(pub [u8; 32]);

use sha2::{Digest, Sha256};

const POPULATION_DOMAIN: &[u8] = b"reliquary:dream-population:v1\0";

/// Hash the sorted, unique raw identities in a Dream population.
///
/// The caller must provide unique identities in canonical ascending order.
/// Selecting and authorizing the population belongs to the caller.
pub fn population_digest<'a>(ids: impl ExactSizeIterator<Item = &'a [u8; 32]>) -> [u8; 32] {
    let count = ids.len() as u64;
    let mut hash = Sha256::new();
    hash.update(POPULATION_DOMAIN);
    hash.update(count.to_le_bytes());
    for id in ids {
        hash.update(id);
    }
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::population_digest;

    #[test]
    fn canonical_vectors() {
        let first = std::array::from_fn(|i| i as u8);
        let second = std::array::from_fn(|i| (i + 32) as u8);
        let hex = |bytes: [u8; 32]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(
            hex(population_digest([].into_iter())),
            "33662ef9e4a7f1c1438a0233cf2471bc3cfb6456ae942ab35d5f937aa8879c97"
        );
        assert_eq!(
            hex(population_digest([&first].into_iter())),
            "11035bedf2554f7a000371d59ac099c587f488ce972fdb180ef895b7686ee6cc"
        );
        assert_eq!(
            hex(population_digest([&first, &second].into_iter())),
            "45196baabc06eb4169de31b45ee407c3983d8e4274ca3f1ea22800ce99978792"
        );
    }
}

pub mod freshness_journal;
