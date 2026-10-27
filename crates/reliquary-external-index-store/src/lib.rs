//! Disposable external index snapshots and atomic installation.
pub mod compact_index;
pub mod model;
pub mod store;
#[cfg(test)]
mod tests {
    use super::{compact_index, store::*};
    fn unhex(s: &str) -> Vec<u8> {
        s.as_bytes()
            .chunks_exact(2)
            .map(|x| u8::from_str_radix(std::str::from_utf8(x).unwrap(), 16).unwrap())
            .collect()
    }
    fn state() -> SourceState {
        SourceState {
            namespace: "x".into(),
            contract: 1,
            profile: [1; 32],
            dimensions: 2,
            generation: 9,
            revision: 17,
            cursor: 4,
            messages: vec![],
            last_base: 3,
            last_digest: [2; 32],
            rebuild: false,
        }
    }
    #[test]
    fn golden_wire_and_damage_recovery() {
        let snapshot = unhex(
            "43564158494458320072000000000000000100000078010000000101010101010101010101010101010101010101010101010101010101010101020000000900000000000000110000000000000004000000000000000300000000000000020202020202020202020202020202020202020202020202020202020202020200000000001a546f58a3df17c177e4e084a7b9ac08ebc3b0d432a803e053733d78e59f2b94",
        );
        let discard = unhex(
            "4356415849445832010000000000000000930a6c0503be5d544ab6e49a171fb9480890ea373254d82125401c022cb575a8",
        );
        assert_eq!(encode(&state()).unwrap(), snapshot);
        assert_eq!(discard_payload(), discard);
        for n in 0..snapshot.len() {
            assert!(compact_index::decode(&snapshot[..n]).is_err());
        }
        let mut broken = snapshot.clone();
        broken[20] ^= 1;
        assert!(compact_index::decode(&broken).is_err());
        let mut store = ExternalIndexStore::default();
        store.ingest(&snapshot);
        assert_eq!(store.sources["x"], state());
        store.ingest(&broken);
        assert!(store.damaged && store.sources.is_empty());
        store.ingest(&snapshot);
        store.ingest(&discard);
        assert!(store.damaged && store.sources.is_empty());
    }
    #[test]
    fn cas_conflict_does_not_write_and_storage_error_invalidates() {
        use crate::model::ExternalSourceError;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("external-index-cas-{}.rel", std::process::id()));
        let mut container = reliquary_container::Container::create(&path).unwrap();
        let mut store = ExternalIndexStore::default();
        store.install(&mut container, state(), None).unwrap();
        let length = std::fs::metadata(&path).unwrap().len();
        assert_eq!(
            store.install(&mut container, state(), None),
            Err(ExternalSourceError::Conflict)
        );
        assert_eq!(std::fs::metadata(&path).unwrap().len(), length);
        assert!(!store.damaged && store.sources.contains_key("x"));
        drop(container);
        let mut read_only = reliquary_container::Container::open_read_only(&path).unwrap();
        assert_eq!(
            store.install(&mut read_only, state(), Some(17)),
            Err(ExternalSourceError::Storage)
        );
        assert!(store.damaged && store.sources.is_empty());
        drop(read_only);
        std::fs::remove_file(path).unwrap();
    }
}
