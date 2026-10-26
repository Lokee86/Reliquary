use crate::entity_resolution_codec::*;
use std::{fs, path::PathBuf};
fn scratch() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("resolution-format-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&p).unwrap();
    p
}
#[test]
fn entity_resolution_delta_explicit_v1_owner_rejection_is_nonmutating() {
    let root = scratch();
    let payload = delta::encode(
        &DecodedEntityResolution {
            key: reliquary_perception_store::MemoryEntityMentionKey {
                memory_id: [7; 32],
                field: reliquary_perception_store::MemoryTextField::Content,
                start_byte: 0,
                end_byte: 2,
            },
            revision: 1,
            updated_at_ns: 11,
            status: Some(crate::MemoryEntityResolutionStatus::Rejected {
                reason: crate::EntityResolutionReason::GenericRole,
            }),
        },
        None,
    )
    .unwrap();
    for phy in [false, true] {
        let path = root.join(if phy { "input.phy" } else { "input.rel" });
        if phy {
            let mut owner = crate::Phylactery::create(&path).unwrap();
            owner.container.append(&payload).unwrap();
            owner.sync().unwrap();
        } else {
            let mut owner = crate::Cva::create(&path).unwrap();
            owner.container.append(&payload).unwrap();
            owner.sync().unwrap();
        }
        let mut bytes = fs::read(&path).unwrap();
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        fs::write(&path, &bytes).unwrap();
        if phy {
            assert!(crate::Phylactery::open(&path).is_err());
            assert!(crate::Phylactery::open(&path).is_err());
        } else {
            assert!(crate::Cva::open(&path).is_err());
            assert!(crate::Cva::open(&path).is_err());
        }
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    fs::remove_dir_all(root).unwrap();
}
