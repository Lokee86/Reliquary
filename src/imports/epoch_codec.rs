use super::{ImportComplete, ImportError, ImportStart};

const START_MAGIC: &[u8; 8] = b"CVAIEPS1";
const COMPLETE_MAGIC: &[u8; 8] = b"CVAIEPC1";

pub(crate) enum ImportEpochRecord {
    Start(ImportStart),
    Complete(ImportComplete),
}

pub(crate) fn encode_start(value: &ImportStart) -> Result<Vec<u8>, ImportError> {
    encode(START_MAGIC, value)
}

pub(crate) fn encode_complete(value: &ImportComplete) -> Result<Vec<u8>, ImportError> {
    encode(COMPLETE_MAGIC, value)
}

fn encode<T: serde::Serialize>(magic: &[u8; 8], value: &T) -> Result<Vec<u8>, ImportError> {
    let body = serde_json::to_vec(value)?;
    let mut bytes = Vec::with_capacity(12 + body.len());
    bytes.extend_from_slice(magic);
    bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&body);
    Ok(bytes)
}

pub(crate) fn decode(payload: &[u8]) -> Result<Option<ImportEpochRecord>, ImportError> {
    let kind = match payload.get(..8) {
        Some(magic) if magic == START_MAGIC => 0,
        Some(magic) if magic == COMPLETE_MAGIC => 1,
        _ => return Ok(None),
    };
    let length = payload
        .get(8..12)
        .ok_or(ImportError::Epoch("truncated import epoch record"))?;
    let length = u32::from_le_bytes(length.try_into().unwrap()) as usize;
    let body = payload
        .get(12..)
        .filter(|body| body.len() == length)
        .ok_or(ImportError::Epoch("invalid import epoch record length"))?;
    Ok(Some(if kind == 0 {
        ImportEpochRecord::Start(serde_json::from_slice(body)?)
    } else {
        ImportEpochRecord::Complete(serde_json::from_slice(body)?)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imports::epoch_store::import_id;
    use crate::imports::{ImportComplete, ImportStart, ImportWatermarks};

    fn start() -> ImportStart {
        let owner_uuid = [7; 16];
        let import_key = "source/import-1".to_owned();
        ImportStart {
            owner_uuid,
            import_id: import_id(owner_uuid, &import_key),
            import_key,
            source_namespace: "provider/account".into(),
            source_format: "json-v1".into(),
            started_at_ns: 100,
            pre: ImportWatermarks::default(),
        }
    }

    fn complete(start: &ImportStart) -> ImportComplete {
        ImportComplete {
            owner_uuid: start.owner_uuid,
            import_id: start.import_id.clone(),
            start_digest: crate::imports::prepare::digest(start).unwrap(),
            receipt_digest: "receipt-digest".into(),
            completed_at_ns: start.started_at_ns,
            post: start.pre,
            historical_freshness_settlement_digest: "settlement-digest".into(),
        }
    }

    #[test]
    fn start_and_complete_records_roundtrip_and_reject_bad_framing() {
        let start = start();
        let start_bytes = encode_start(&start).unwrap();
        assert!(matches!(
            decode(&start_bytes).unwrap(),
            Some(ImportEpochRecord::Start(decoded)) if decoded == start
        ));

        let complete = complete(&start);
        let complete_bytes = encode_complete(&complete).unwrap();
        assert!(matches!(
            decode(&complete_bytes).unwrap(),
            Some(ImportEpochRecord::Complete(decoded)) if decoded == complete
        ));

        for bytes in [&start_bytes, &complete_bytes] {
            for end in 8..bytes.len() {
                assert!(
                    decode(&bytes[..end]).is_err(),
                    "accepted truncation at {end}"
                );
            }
            let mut trailing = bytes.to_vec();
            trailing.push(0);
            assert!(decode(&trailing).is_err());

            let mut bad_length = bytes.to_vec();
            bad_length[8] ^= 1;
            assert!(decode(&bad_length).is_err());
        }
    }
}
