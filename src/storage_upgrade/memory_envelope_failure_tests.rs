//! Rejected V1 authority leaves both source and staging destination untouched.
use super::*;
use std::io::{Seek, SeekFrom, Write};

fn append_frame(bytes: &mut Vec<u8>, payload: &[u8]) -> crate::ObjectRef {
    let mut reference = [0; 16];
    reference[..8].copy_from_slice(&(bytes.len() as u64).to_le_bytes());
    reference[8..].copy_from_slice(&(payload.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    bytes.extend_from_slice(payload);
    crate::ObjectRef::from_legacy_bytes(reference)
}

#[test]
fn invalid_legacy_memory_authority_never_returns_a_stage_or_changes_the_source() {
    for kind in [FileKind::Reliquary, FileKind::Phylactery] {
        for resurrection in [false, true] {
            let scratch = Scratch::new();
            let request = scratch.request(kind);
            build_v1_source(&request.source, kind);
            if resurrection {
                let mut bytes = fs::read(&request.source).unwrap();
                let mut ticket = b"CVAVERS2".to_vec();
                ticket.extend_from_slice(&5_u64.to_le_bytes());
                ticket.extend_from_slice(&(-99_i64).to_le_bytes());
                append_frame(&mut bytes, &ticket);
                let body = memory_body_id(
                    "Legacy body",
                    "The payload and its history survive envelope conversion.",
                );
                let carrier = append_frame(
                    &mut bytes,
                    &encode_record(&record(4, "resurrection", "invalid", body)).unwrap(),
                );
                append_frame(
                    &mut bytes,
                    &encode_version(MemoryVersion {
                        global_version: 5,
                        memory_version: 4,
                        record: carrier,
                    }),
                );
                fs::write(&request.source, bytes).unwrap();
            } else {
                let mut source = Container::open_v1_read_only(&request.source).unwrap();
                let mut marker = None;
                for object in source.chunks().unwrap() {
                    if let Some(mut version) =
                        crate::memory_codec::decode_version(&source.read(object).unwrap()).unwrap()
                    {
                        version.record = crate::ObjectRef::from_legacy_bytes([0; 16]);
                        marker = Some((object.offset() + 8, encode_version(version)));
                        break;
                    }
                }
                drop(source);
                let (offset, payload) = marker.unwrap();
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .open(&request.source)
                    .unwrap();
                file.seek(SeekFrom::Start(offset)).unwrap();
                file.write_all(&payload).unwrap();
                file.sync_all().unwrap();
            }
            let before = fs::read(&request.source).unwrap();
            let modified = fs::metadata(&request.source).unwrap().modified().unwrap();
            let plan = plan_storage_upgrade(request.clone()).unwrap();
            assert!(stage_memory_envelope_storage_upgrade(&plan).is_err());
            assert_eq!(fs::read(&request.source).unwrap(), before);
            assert_eq!(
                fs::metadata(&request.source).unwrap().modified().unwrap(),
                modified
            );
            assert!(!request.destination.exists());
            assert!(fs::read_dir(&scratch.0).unwrap().all(|entry| {
                !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".storage-upgrade-")
            }));
        }
    }
}
