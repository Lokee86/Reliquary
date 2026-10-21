use crate::MemoryId;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

// The separator ends with one NUL byte.
const DOMAIN: &[u8] = b"reliquary:dream-population:v1\0";

/// Hash a canonical Dream population set. Authority coordinates such as owner,
/// source and Memory cut are bound separately and intentionally excluded here.
pub(crate) fn digest(ids: &BTreeSet<MemoryId>) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update((ids.len() as u64).to_le_bytes());
    for id in ids {
        hash.update(id.0);
    }
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::digest;
    use crate::MemoryId;
    use std::collections::BTreeSet;

    fn from_hex(value: &str) -> [u8; 32] {
        let mut bytes = [0; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap();
        }
        bytes
    }

    fn ascending_id(start: u8) -> MemoryId {
        MemoryId(std::array::from_fn(|index| start.wrapping_add(index as u8)))
    }

    #[test]
    fn digest_matches_independent_empty_single_and_two_id_vectors() {
        let empty = BTreeSet::new();
        assert_eq!(
            digest(&empty),
            from_hex("33662ef9e4a7f1c1438a0233cf2471bc3cfb6456ae942ab35d5f937aa8879c97")
        );

        let first = ascending_id(0);
        let second = ascending_id(32);
        assert_eq!(
            digest(&BTreeSet::from([first])),
            from_hex("11035bedf2554f7a000371d59ac099c587f488ce972fdb180ef895b7686ee6cc")
        );
        assert_eq!(
            digest(&BTreeSet::from([first, second])),
            from_hex("45196baabc06eb4169de31b45ee407c3983d8e4274ca3f1ea22800ce99978792")
        );
    }

    #[test]
    fn digest_is_independent_of_insertion_order_and_changes_with_membership() {
        let first = ascending_id(0);
        let second = ascending_id(32);
        let third = ascending_id(64);
        let forward = BTreeSet::from([first, second]);
        let reverse = [second, first].into_iter().collect();
        assert_eq!(digest(&forward), digest(&reverse));
        assert_eq!(
            digest(&[first, first].into_iter().collect()),
            digest(&BTreeSet::from([first]))
        );
        assert_ne!(digest(&forward), digest(&BTreeSet::from([first, third])));
    }
}
