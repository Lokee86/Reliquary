//! Payload selection for semantic replays. The callback sees only the first
//! sixty-four physical bytes, so uncertain and malformed headers stay in the scan.

/// Archive publications have Archive-local effects. Their full payloads are
/// validated by the Archive store; owner graph and Dream prefix readers only
/// need the relative order of other frames.
pub(crate) fn without_archive_publications(prefix: &[u8]) -> bool {
    if prefix.len() < 64 {
        return true;
    }
    let Ok(Some(publication)) = reliquary_container::decode_publication(prefix) else {
        return true;
    };
    !publication
        .body
        .starts_with(reliquary_archive::ARCHIVE_PUBLICATION_MARKER)
}

/// Packed matrices are validated by the vector store and do not affect Dream
/// or import-admission clocks. Keep short frames for whole-record validation.
pub(crate) fn without_packed_vectors(prefix: &[u8]) -> bool {
    prefix.len() < 64 || !prefix.starts_with(b"RELPVEC1")
}

pub(crate) fn without_archive_or_packed(prefix: &[u8]) -> bool {
    without_archive_publications(prefix) && without_packed_vectors(prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_uncertain_headers_and_skips_known_large_families() {
        for timed in [false, true] {
            let mut publication = b"RELPUB01".to_vec();
            publication.extend_from_slice(&1u64.to_le_bytes());
            publication.push(u8::from(timed));
            if timed {
                publication.extend_from_slice(&0i64.to_le_bytes());
            }
            publication.extend_from_slice(b"RELAPB01");
            publication.extend_from_slice(&1u64.to_le_bytes());
            publication.push(0);
            publication.extend_from_slice(b"RELTURN2");
            publication.resize(64, 0);
            assert!(!without_archive_publications(&publication[..64]));
            publication[16] = 9;
            assert!(without_archive_publications(&publication[..64]));
        }

        assert!(without_archive_publications(b"RELAPB01"));
        assert!(without_packed_vectors(b"RELPVEC1short"));
        let mut packed = b"RELPVEC1".to_vec();
        packed.resize(64, 0);
        assert!(!without_packed_vectors(&packed[..64]));
    }
}
